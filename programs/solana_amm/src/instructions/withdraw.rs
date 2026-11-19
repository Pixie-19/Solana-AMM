
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Burn, Mint, Token, TokenAccount, Transfer},
};

use crate::{
    constants::POOL_SEED,
    error::AmmError,
    state::Pool,
};

#[derive(Accounts)]
pub struct WithdrawLiquidity<'info> {
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

    #[account(mut, address = pool.lp_mint)]
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
        associated_token::mint = lp_mint,
        associated_token::authority = user
    )]
    pub user_lp: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint_x,
        associated_token::authority = user
    )]
    pub user_x: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint_y,
        associated_token::authority = user
    )]
    pub user_y: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handle_withdraw(
    ctx: Context<WithdrawLiquidity>,
    lp_amount: u64,
    min_x: u64,
    min_y: u64,
) -> Result<()> {
    require!(lp_amount > 0, AmmError::InvalidAmount);

    let reserve_x = ctx.accounts.vault_x.amount;
    let reserve_y = ctx.accounts.vault_y.amount;
    let lp_supply = ctx.accounts.lp_mint.supply;

    require!(
        lp_amount <= ctx.accounts.user_lp.amount,
        AmmError::InsufficientLpTokens
    );

    require!(
        reserve_x > 0 && reserve_y > 0 && lp_supply > 0,
        AmmError::ZeroReserves
    );

    // Calculate the user's proportional share of both reserves.
    let amount_x = (lp_amount as u128)
        .checked_mul(reserve_x as u128)
        .ok_or(AmmError::Overflow)?
        .checked_div(lp_supply as u128)
        .ok_or(AmmError::ZeroReserves)?;

    let amount_y = (lp_amount as u128)
        .checked_mul(reserve_y as u128)
        .ok_or(AmmError::Overflow)?
        .checked_div(lp_supply as u128)
        .ok_or(AmmError::ZeroReserves)?;

    let amount_x = u64::try_from(amount_x)
        .map_err(|_| AmmError::Overflow)?;

    let amount_y = u64::try_from(amount_y)
        .map_err(|_| AmmError::Overflow)?;

    require!(amount_x > 0 && amount_y > 0, AmmError::InvalidAmount);
    require!(amount_x >= min_x, AmmError::SlippageExceeded);
    require!(amount_y >= min_y, AmmError::SlippageExceeded);

    // Burn the user's LP tokens.
    let burn_accounts = Burn {
        mint: ctx.accounts.lp_mint.to_account_info(),
        from: ctx.accounts.user_lp.to_account_info(),
        authority: ctx.accounts.user.to_account_info(),
    };

    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            burn_accounts,
        ),
        lp_amount,
    )?;

    // The pool PDA signs the transfers from its vaults.
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

    let transfer_x = Transfer {
        from: ctx.accounts.vault_x.to_account_info(),
        to: ctx.accounts.user_x.to_account_info(),
        authority: ctx.accounts.pool.to_account_info(),
    };

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            transfer_x,
            signer,
        ),
        amount_x,
    )?;

    let transfer_y = Transfer {
        from: ctx.accounts.vault_y.to_account_info(),
        to: ctx.accounts.user_y.to_account_info(),
        authority: ctx.accounts.pool.to_account_info(),
    };

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            transfer_y,
            signer,
        ),
        amount_y,
    )?;

    msg!("Liquidity withdrawn successfully");
    msg!("LP tokens burned: {}", lp_amount);
    msg!("Token X returned: {}", amount_x);
    msg!("Token Y returned: {}", amount_y);

    Ok(())
}
