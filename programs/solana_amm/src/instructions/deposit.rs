
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Mint, MintTo, Token, TokenAccount, Transfer},
};

use crate::{
    constants::POOL_SEED,
    error::AmmError,
    state::Pool,
};

#[derive(Accounts)]
pub struct DepositLiquidity<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [
            POOL_SEED,
            mint_x.key().as_ref(),
            mint_y.key().as_ref()
        ],
        bump = pool.bump,
        has_one = mint_x,
        has_one = mint_y,
        has_one = lp_mint,
        has_one = vault_x,
        has_one = vault_y
    )]
    pub pool: Box<Account<'info, Pool>>,
    pub mint_x: Box<Account<'info, Mint>>,
    pub mint_y: Box<Account<'info, Mint>>,

    #[account(
        mut,
        address = pool.lp_mint
    )]
    pub lp_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        address = pool.vault_x,
        token::mint = mint_x,
        token::authority = pool
    )]
    pub vault_x: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        address = pool.vault_y,
        token::mint = mint_y,
        token::authority = pool
    )]
    pub vault_y: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = user
    )]
    pub user_x: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = user
    )]
    pub user_y: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = lp_mint,
        associated_token::authority = user
    )]
    pub user_lp: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handle_deposit(
    ctx: Context<DepositLiquidity>,
    max_x: u64,
    max_y: u64,
    min_lp: u64,
) -> Result<()> {
    require!(max_x > 0 && max_y > 0, AmmError::InvalidAmount);

    let reserve_x = ctx.accounts.vault_x.amount;
    let reserve_y = ctx.accounts.vault_y.amount;
    let lp_supply = ctx.accounts.lp_mint.supply;

    let (amount_x, amount_y, lp_amount) =
        if lp_supply == 0 {
            require!(
                reserve_x == 0 && reserve_y == 0,
                AmmError::ZeroReserves
            );

            let product = (max_x as u128)
                .checked_mul(max_y as u128)
                .ok_or(AmmError::Overflow)?;

            let liquidity = integer_sqrt(product);

            (
                max_x,
                max_y,
                u64::try_from(liquidity)
                    .map_err(|_| AmmError::Overflow)?,
            )
        } else {
            require!(
                reserve_x > 0 && reserve_y > 0,
                AmmError::ZeroReserves
            );

            let lp_x = (max_x as u128)
                .checked_mul(lp_supply as u128)
                .ok_or(AmmError::Overflow)?
                .checked_div(reserve_x as u128)
                .ok_or(AmmError::ZeroReserves)?;

            let lp_y = (max_y as u128)
                .checked_mul(lp_supply as u128)
                .ok_or(AmmError::Overflow)?
                .checked_div(reserve_y as u128)
                .ok_or(AmmError::ZeroReserves)?;

            let liquidity = lp_x.min(lp_y);

            let amount_x = div_ceil(
                liquidity
                    .checked_mul(reserve_x as u128)
                    .ok_or(AmmError::Overflow)?,
                lp_supply as u128,
            )?;

            let amount_y = div_ceil(
                liquidity
                    .checked_mul(reserve_y as u128)
                    .ok_or(AmmError::Overflow)?,
                lp_supply as u128,
            )?;

            (
                u64::try_from(amount_x).map_err(|_| AmmError::Overflow)?,
                u64::try_from(amount_y).map_err(|_| AmmError::Overflow)?,
                u64::try_from(liquidity).map_err(|_| AmmError::Overflow)?,
            )
        };

    require!(lp_amount > 0, AmmError::InvalidAmount);
    require!(lp_amount >= min_lp, AmmError::SlippageExceeded);
    require!(amount_x <= max_x, AmmError::SlippageExceeded);
    require!(amount_y <= max_y, AmmError::SlippageExceeded);

    // Transfer Token X from the user to the pool.
    let transfer_x = Transfer {
        from: ctx.accounts.user_x.to_account_info(),
        to: ctx.accounts.vault_x.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            transfer_x,
        ),
        amount_x,
    )?;

    // Transfer Token Y from the user to the pool.
    let transfer_y = Transfer {
        from: ctx.accounts.user_y.to_account_info(),
        to: ctx.accounts.vault_y.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            transfer_y,
        ),
        amount_y,
    )?;

    // Mint LP tokens using the pool PDA as mint authority.
    let mint_x_key = ctx.accounts.mint_x.key();
    let mint_y_key = ctx.accounts.mint_y.key();
    let bump = [ctx.accounts.pool.bump];

    let signer_seeds: &[&[u8]] = &[
        POOL_SEED,
        mint_x_key.as_ref(),
        mint_y_key.as_ref(),
        &bump,
    ];

    let signer = &[signer_seeds];

    let mint_accounts = MintTo {
        mint: ctx.accounts.lp_mint.to_account_info(),
        to: ctx.accounts.user_lp.to_account_info(),
        authority: ctx.accounts.pool.to_account_info(),
    };

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            mint_accounts,
            signer,
        ),
        lp_amount,
    )?;

    msg!("Liquidity deposited successfully");
    msg!("Token X deposited: {}", amount_x);
    msg!("Token Y deposited: {}", amount_y);
    msg!("LP tokens minted: {}", lp_amount);

    Ok(())
}

fn integer_sqrt(value: u128) -> u128 {
    if value < 2 {
        return value;
    }

    let mut x = value;
    let mut y = x
        .checked_add(1)
        .unwrap_or(x)
        / 2;

    while y < x {
        x = y;
        y = (x + value / x) / 2;
    }

    x
}

fn div_ceil(numerator: u128, denominator: u128) -> Result<u128> {
    require!(denominator > 0, AmmError::ZeroReserves);

    let adjusted = numerator
        .checked_add(denominator - 1)
        .ok_or(AmmError::Overflow)?;

    Ok(adjusted / denominator)
}
