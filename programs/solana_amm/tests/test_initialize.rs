
use anchor_lang::{
    prelude::*,
    solana_program::{
        instruction::Instruction,
        system_program,
    },
    AccountDeserialize,
    InstructionData,
    ToAccountMetas,
};

use anchor_spl::{
    associated_token::get_associated_token_address,
    token::spl_token,
};

use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use std::path::PathBuf;

fn send_transaction(
    svm: &mut LiteSVM,
    payer: &Keypair,
    instruction: Instruction,
) {
    let blockhash = svm.latest_blockhash();

    let message = Message::new_with_blockhash(
        &[instruction],
        Some(&payer.pubkey()),
        &blockhash,
    );

    let transaction = VersionedTransaction::try_new(
        VersionedMessage::Legacy(message),
        &[payer],
    )
    .unwrap();

    if let Err(error) = svm.send_transaction(transaction) {
        panic!(
            "Transaction failed: {:?}\nLogs:\n{}",
            error.err,
            error.meta.pretty_logs()
        );
    }
}

fn create_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    decimals: u8,
) {
    // Classic SPL Token Mint accounts have a fixed size of 82 bytes.
    let mint_len = 82;

    let rent = svm.minimum_balance_for_rent_exemption(mint_len);

    let create_account =
        anchor_lang::solana_program::system_instruction::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            rent,
            mint_len as u64,
            &spl_token::ID,
        );

    let initialize_mint = spl_token::instruction::initialize_mint(
        &spl_token::ID,
        &mint.pubkey(),
        &payer.pubkey(),
        None,
        decimals,
    )
    .unwrap();

    let blockhash = svm.latest_blockhash();

    let message = Message::new_with_blockhash(
        &[create_account, initialize_mint],
        Some(&payer.pubkey()),
        &blockhash,
    );

    let transaction = VersionedTransaction::try_new(
        VersionedMessage::Legacy(message),
        &[payer, mint],
    )
    .unwrap();

    if let Err(error) = svm.send_transaction(transaction) {
        panic!(
            "Mint creation failed: {:?}\nLogs:\n{}",
            error.err,
            error.meta.pretty_logs()
        );
    }
}

#[test]
fn test_initialize_amm_pool() {
    let program_id = solana_amm::id();

    let payer = Keypair::new();
    let treasury = Keypair::new();

    let mint_x = Keypair::new();
    let mint_y = Keypair::new();

    let mut svm = LiteSVM::new().with_default_programs();

    // Load the compiled AMM program into LiteSVM.
    let program_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/deploy/solana_amm.so");

    svm.add_program_from_file(
        program_id,
        program_path,
    )
    .unwrap();

    // Fund the payer.
    svm.airdrop(&payer.pubkey(), 5_000_000_000)
        .unwrap();

    // Create two classic SPL token mints.
    create_mint(&mut svm, &payer, &mint_x, 6);
    create_mint(&mut svm, &payer, &mint_y, 6);

    // Derive the pool PDA.
    let (pool, _) = Pubkey::find_program_address(
        &[
            solana_amm::constants::POOL_SEED,
            mint_x.pubkey().as_ref(),
            mint_y.pubkey().as_ref(),
        ],
        &program_id,
    );

    // Derive the LP mint PDA.
    let (lp_mint, _) = Pubkey::find_program_address(
        &[
            solana_amm::constants::LP_MINT_SEED,
            pool.as_ref(),
        ],
        &program_id,
    );

    // Derive pool vault addresses.
    let vault_x = get_associated_token_address(
        &pool,
        &mint_x.pubkey(),
    );

    let vault_y = get_associated_token_address(
        &pool,
        &mint_y.pubkey(),
    );

    // Derive treasury token account addresses.
    let treasury_x = get_associated_token_address(
        &treasury.pubkey(),
        &mint_x.pubkey(),
    );

    let treasury_y = get_associated_token_address(
        &treasury.pubkey(),
        &mint_y.pubkey(),
    );

    // Build the initialize instruction.
    let instruction = Instruction {
        program_id,
        accounts: solana_amm::accounts::Initialize {
            payer: payer.pubkey(),
            treasury: treasury.pubkey(),
            mint_x: mint_x.pubkey(),
            mint_y: mint_y.pubkey(),
            pool,
            lp_mint,
            vault_x,
            vault_y,
            treasury_x,
            treasury_y,
            token_program: spl_token::ID,
            associated_token_program:
                anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: solana_amm::instruction::Initialize {
            fee_bps: 30,
        }
        .data(),
    };

    // First, verify that an invalid fee is rejected.
    let invalid_instruction = Instruction {
        data: solana_amm::instruction::Initialize {
            fee_bps: 1_001,
        }
        .data(),
        ..instruction.clone()
    };

    let blockhash = svm.latest_blockhash();

    let message = Message::new_with_blockhash(
        &[invalid_instruction],
        Some(&payer.pubkey()),
        &blockhash,
    );

    let invalid_transaction = VersionedTransaction::try_new(
        VersionedMessage::Legacy(message),
        &[&payer],
    )
    .unwrap();

    let invalid_result = svm.send_transaction(invalid_transaction);

    assert!(
        invalid_result.is_err(),
        "Initialization should reject a fee above 1,000 basis points"
    );

    println!("Invalid fee correctly rejected");

    // Now execute initialization with a valid 30 bps fee.
    send_transaction(&mut svm, &payer, instruction);

    // Read and deserialize the created pool account.
    let pool_account = svm.get_account(&pool).unwrap();

    let mut data: &[u8] = &pool_account.data;

    let pool_state =
        solana_amm::state::Pool::try_deserialize(&mut data)
            .unwrap();

    // Verify stored pool information.
    assert_eq!(pool_state.authority, payer.pubkey());
    assert_eq!(pool_state.mint_x, mint_x.pubkey());
    assert_eq!(pool_state.mint_y, mint_y.pubkey());
    assert_eq!(pool_state.lp_mint, lp_mint);
    assert_eq!(pool_state.vault_x, vault_x);
    assert_eq!(pool_state.vault_y, vault_y);
    assert_eq!(pool_state.treasury, treasury.pubkey());
    assert_eq!(pool_state.fee_bps, 30);

    // Verify that all expected token accounts were created.
    assert!(svm.get_account(&lp_mint).is_some());
    assert!(svm.get_account(&vault_x).is_some());
    assert!(svm.get_account(&vault_y).is_some());
    assert!(svm.get_account(&treasury_x).is_some());
    assert!(svm.get_account(&treasury_y).is_some());
}
fn create_token_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    owner: &Pubkey,
    mint: &Pubkey,
) -> Pubkey {
    let ata = get_associated_token_address(owner, mint);

    let instruction =
        anchor_spl::associated_token::spl_associated_token_account
            ::instruction::create_associated_token_account(
                &payer.pubkey(),
                owner,
                mint,
                &spl_token::ID,
            );

    send_transaction(svm, payer, instruction);

    ata
}

fn mint_tokens(
    svm: &mut LiteSVM,
    authority: &Keypair,
    mint: &Pubkey,
    destination: &Pubkey,
    amount: u64,
) {
    let instruction = spl_token::instruction::mint_to(
        &spl_token::ID,
        mint,
        destination,
        &authority.pubkey(),
        &[],
        amount,
    )
    .unwrap();

    send_transaction(svm, authority, instruction);
}

fn get_token_balance(
    svm: &LiteSVM,
    token_account: &Pubkey,
) -> u64 {
    let account = svm.get_account(token_account).unwrap();

    let mut data: &[u8] = &account.data;

    let token_account_state =
        anchor_spl::token::TokenAccount::try_deserialize(&mut data)
            .unwrap();

    token_account_state.amount
}

#[test]
fn test_deposit_liquidity() {
    let program_id = solana_amm::id();

    let payer = Keypair::new();
    let treasury = Keypair::new();
    let user = Keypair::new();

    let mint_x = Keypair::new();
    let mint_y = Keypair::new();

    let mut svm = LiteSVM::new().with_default_programs();

    let program_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/deploy/solana_amm.so");

    svm.add_program_from_file(program_id, program_path).unwrap();

    svm.airdrop(&payer.pubkey(), 5_000_000_000).unwrap();
    svm.airdrop(&user.pubkey(), 5_000_000_000).unwrap();

    create_mint(&mut svm, &payer, &mint_x, 6);
    create_mint(&mut svm, &payer, &mint_y, 6);

    let (pool, _) = Pubkey::find_program_address(
        &[
            solana_amm::constants::POOL_SEED,
            mint_x.pubkey().as_ref(),
            mint_y.pubkey().as_ref(),
        ],
        &program_id,
    );

    let (lp_mint, _) = Pubkey::find_program_address(
        &[
            solana_amm::constants::LP_MINT_SEED,
            pool.as_ref(),
        ],
        &program_id,
    );

    let vault_x = get_associated_token_address(&pool, &mint_x.pubkey());
    let vault_y = get_associated_token_address(&pool, &mint_y.pubkey());

    let treasury_x =
        get_associated_token_address(&treasury.pubkey(), &mint_x.pubkey());
    let treasury_y =
        get_associated_token_address(&treasury.pubkey(), &mint_y.pubkey());

    // Initialize the pool.
    let initialize_ix = Instruction {
        program_id,
        accounts: solana_amm::accounts::Initialize {
            payer: payer.pubkey(),
            treasury: treasury.pubkey(),
            mint_x: mint_x.pubkey(),
            mint_y: mint_y.pubkey(),
            pool,
            lp_mint,
            vault_x,
            vault_y,
            treasury_x,
            treasury_y,
            token_program: spl_token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: solana_amm::instruction::Initialize { fee_bps: 30 }.data(),
    };

    send_transaction(&mut svm, &payer, initialize_ix);

    // Create the user's X and Y token accounts.
    let user_x = create_token_account(
        &mut svm,
        &user,
        &user.pubkey(),
        &mint_x.pubkey(),
    );

    let user_y = create_token_account(
        &mut svm,
        &user,
        &user.pubkey(),
        &mint_y.pubkey(),
    );

    // Create the user's LP token account.
    let user_lp = get_associated_token_address(&user.pubkey(), &lp_mint);

    // Give the user 10,000 X and 40,000 Y.
    mint_tokens(
        &mut svm,
        &payer,
        &mint_x.pubkey(),
        &user_x,
        10_000,
    );

    mint_tokens(
        &mut svm,
        &payer,
        &mint_y.pubkey(),
        &user_y,
        40_000,
    );

    // Build the deposit instruction.
    let deposit_ix = Instruction {
        program_id,
        accounts: solana_amm::accounts::DepositLiquidity {
            user: user.pubkey(),
            pool,
            mint_x: mint_x.pubkey(),
            mint_y: mint_y.pubkey(),
            lp_mint,
            vault_x,
            vault_y,
            user_x,
            user_y,
            user_lp,
            token_program: spl_token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: solana_amm::instruction::Deposit {
            max_x: 10_000,
            max_y: 40_000,
            min_lp: 20_000,
        }
        .data(),
    };

    send_transaction(&mut svm, &user, deposit_ix);

    // Verify pool reserves.
    assert_eq!(get_token_balance(&svm, &vault_x), 10_000);
    assert_eq!(get_token_balance(&svm, &vault_y), 40_000);

    // Verify LP tokens minted.
    assert_eq!(get_token_balance(&svm, &user_lp), 20_000);

    println!("Deposit test passed!");
}

#[test]
fn test_withdraw_liquidity() {
    let program_id = solana_amm::id();

    let payer = Keypair::new();
    let treasury = Keypair::new();
    let user = Keypair::new();

    let mint_x = Keypair::new();
    let mint_y = Keypair::new();

    let mut svm = LiteSVM::new().with_default_programs();

    let program_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/deploy/solana_amm.so");

    svm.add_program_from_file(program_id, program_path).unwrap();

    svm.airdrop(&payer.pubkey(), 5_000_000_000).unwrap();
    svm.airdrop(&user.pubkey(), 5_000_000_000).unwrap();

    create_mint(&mut svm, &payer, &mint_x, 6);
    create_mint(&mut svm, &payer, &mint_y, 6);

    let (pool, _) = Pubkey::find_program_address(
        &[
            solana_amm::constants::POOL_SEED,
            mint_x.pubkey().as_ref(),
            mint_y.pubkey().as_ref(),
        ],
        &program_id,
    );

    let (lp_mint, _) = Pubkey::find_program_address(
        &[
            solana_amm::constants::LP_MINT_SEED,
            pool.as_ref(),
        ],
        &program_id,
    );

    let vault_x = get_associated_token_address(&pool, &mint_x.pubkey());
    let vault_y = get_associated_token_address(&pool, &mint_y.pubkey());

    let treasury_x =
        get_associated_token_address(&treasury.pubkey(), &mint_x.pubkey());
    let treasury_y =
        get_associated_token_address(&treasury.pubkey(), &mint_y.pubkey());

    // Initialize the pool.
    let initialize_ix = Instruction {
        program_id,
        accounts: solana_amm::accounts::Initialize {
            payer: payer.pubkey(),
            treasury: treasury.pubkey(),
            mint_x: mint_x.pubkey(),
            mint_y: mint_y.pubkey(),
            pool,
            lp_mint,
            vault_x,
            vault_y,
            treasury_x,
            treasury_y,
            token_program: spl_token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: solana_amm::instruction::Initialize { fee_bps: 30 }.data(),
    };

    send_transaction(&mut svm, &payer, initialize_ix);

    // Create user token accounts.
    let user_x = create_token_account(
        &mut svm,
        &user,
        &user.pubkey(),
        &mint_x.pubkey(),
    );

    let user_y = create_token_account(
        &mut svm,
        &user,
        &user.pubkey(),
        &mint_y.pubkey(),
    );

    let user_lp = get_associated_token_address(&user.pubkey(), &lp_mint);

    // Fund the user's token accounts.
    mint_tokens(&mut svm, &payer, &mint_x.pubkey(), &user_x, 10_000);
    mint_tokens(&mut svm, &payer, &mint_y.pubkey(), &user_y, 40_000);

    // Deposit liquidity.
    let deposit_ix = Instruction {
        program_id,
        accounts: solana_amm::accounts::DepositLiquidity {
            user: user.pubkey(),
            pool,
            mint_x: mint_x.pubkey(),
            mint_y: mint_y.pubkey(),
            lp_mint,
            vault_x,
            vault_y,
            user_x,
            user_y,
            user_lp,
            token_program: spl_token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: solana_amm::instruction::Deposit {
            max_x: 10_000,
            max_y: 40_000,
            min_lp: 20_000,
        }
        .data(),
    };

    send_transaction(&mut svm, &user, deposit_ix);

    // Withdraw half of the LP tokens.
    let withdraw_ix = Instruction {
        program_id,
        accounts: solana_amm::accounts::WithdrawLiquidity {
            user: user.pubkey(),
            pool,
            mint_x: mint_x.pubkey(),
            mint_y: mint_y.pubkey(),
            lp_mint,
            vault_x,
            vault_y,
            user_lp,
            user_x,
            user_y,
            token_program: spl_token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: solana_amm::instruction::Withdraw {
            lp_amount: 10_000,
            min_x: 5_000,
            min_y: 20_000,
        }
        .data(),
    };

    send_transaction(&mut svm, &user, withdraw_ix);

    // Verify remaining pool reserves.
    assert_eq!(get_token_balance(&svm, &vault_x), 5_000);
    assert_eq!(get_token_balance(&svm, &vault_y), 20_000);

    // Verify tokens returned to the user.
    assert_eq!(get_token_balance(&svm, &user_x), 5_000);
    assert_eq!(get_token_balance(&svm, &user_y), 20_000);

    // Verify remaining LP tokens.
    assert_eq!(get_token_balance(&svm, &user_lp), 10_000);

    println!("Withdrawal test passed!");
}

#[test]
fn test_swap_and_treasury_fee() {
    let program_id = solana_amm::id();

    let payer = Keypair::new();
    let treasury = Keypair::new();
    let user = Keypair::new();

    let mint_x = Keypair::new();
    let mint_y = Keypair::new();

    let mut svm = LiteSVM::new().with_default_programs();

    let program_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/deploy/solana_amm.so");

    svm.add_program_from_file(program_id, program_path).unwrap();

    svm.airdrop(&payer.pubkey(), 5_000_000_000).unwrap();
    svm.airdrop(&user.pubkey(), 5_000_000_000).unwrap();

    create_mint(&mut svm, &payer, &mint_x, 6);
    create_mint(&mut svm, &payer, &mint_y, 6);

    let (pool, _) = Pubkey::find_program_address(
        &[
            solana_amm::constants::POOL_SEED,
            mint_x.pubkey().as_ref(),
            mint_y.pubkey().as_ref(),
        ],
        &program_id,
    );

    let (lp_mint, _) = Pubkey::find_program_address(
        &[
            solana_amm::constants::LP_MINT_SEED,
            pool.as_ref(),
        ],
        &program_id,
    );

    let vault_x = get_associated_token_address(&pool, &mint_x.pubkey());
    let vault_y = get_associated_token_address(&pool, &mint_y.pubkey());

    let treasury_x =
        get_associated_token_address(&treasury.pubkey(), &mint_x.pubkey());
    let treasury_y =
        get_associated_token_address(&treasury.pubkey(), &mint_y.pubkey());

    // Initialize the pool with a 0.3% swap fee.
    let initialize_ix = Instruction {
        program_id,
        accounts: solana_amm::accounts::Initialize {
            payer: payer.pubkey(),
            treasury: treasury.pubkey(),
            mint_x: mint_x.pubkey(),
            mint_y: mint_y.pubkey(),
            pool,
            lp_mint,
            vault_x,
            vault_y,
            treasury_x,
            treasury_y,
            token_program: spl_token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: solana_amm::instruction::Initialize { fee_bps: 30 }.data(),
    };

    send_transaction(&mut svm, &payer, initialize_ix);

    // Create user token accounts.
    let user_x = create_token_account(
        &mut svm,
        &user,
        &user.pubkey(),
        &mint_x.pubkey(),
    );

    let user_y = create_token_account(
        &mut svm,
        &user,
        &user.pubkey(),
        &mint_y.pubkey(),
    );

    let user_lp = get_associated_token_address(&user.pubkey(), &lp_mint);

    // Give the user 11,000 X and 40,000 Y.
    // 10,000 X and 40,000 Y will be deposited.
    // The remaining 1,000 X will be used for the swap.
    mint_tokens(&mut svm, &payer, &mint_x.pubkey(), &user_x, 11_000);
    mint_tokens(&mut svm, &payer, &mint_y.pubkey(), &user_y, 40_000);

    // Deposit initial liquidity.
    let deposit_ix = Instruction {
        program_id,
        accounts: solana_amm::accounts::DepositLiquidity {
            user: user.pubkey(),
            pool,
            mint_x: mint_x.pubkey(),
            mint_y: mint_y.pubkey(),
            lp_mint,
            vault_x,
            vault_y,
            user_x,
            user_y,
            user_lp,
            token_program: spl_token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: solana_amm::instruction::Deposit {
            max_x: 10_000,
            max_y: 40_000,
            min_lp: 20_000,
        }
        .data(),
    };

    send_transaction(&mut svm, &user, deposit_ix);

    // Swap 1,000 X for Y.
    let swap_ix = Instruction {
        program_id,
        accounts: solana_amm::accounts::SwapTokens {
            user: user.pubkey(),
            pool,
            mint_x: mint_x.pubkey(),
            mint_y: mint_y.pubkey(),
            vault_x,
            vault_y,
            user_x,
            user_y,
            treasury: treasury.pubkey(),
            treasury_x,
            treasury_y,
            token_program: spl_token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
        }
        .to_account_metas(None),
        data: solana_amm::instruction::Swap {
            amount_in: 1_000,
            min_amount_out: 3_626,
            input_is_x: true,
        }
        .data(),
    };

    send_transaction(&mut svm, &user, swap_ix);

    // Verify pool reserves.
    assert_eq!(get_token_balance(&svm, &vault_x), 10_997);
    assert_eq!(get_token_balance(&svm, &vault_y), 36_374);

    // Verify the treasury received the 3-token fee.
    assert_eq!(get_token_balance(&svm, &treasury_x), 3);
    assert_eq!(get_token_balance(&svm, &treasury_y), 0);

    // Verify the user received output tokens.
    assert_eq!(get_token_balance(&svm, &user_x), 0);
    assert_eq!(get_token_balance(&svm, &user_y), 3_626);

    println!("Swap and treasury fee test passed!");
}

