
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Mint, Token, TokenAccount, Transfer},
};

use crate::{
    constants::POOL_SEED,
    error::AmmError,
    math::cpmm::calculate_swap,
    state::Pool,
};

#[derive(Accounts)]
pub struct SwapTokens<'info> {
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
        has_one = vault_x,
        has_one = vault_y,
        has_one = treasury
    )]
    pub pool: Box<Account<'info, Pool>>,
    pub mint_x: Box<Account<'info, Mint>>,
    pub mint_y: Box<Account<'info, Mint>>,

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

    pub treasury: SystemAccount<'info>,

    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = treasury
    )]
    pub treasury_x: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = treasury
    )]
    pub treasury_y: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handle_swap(
    ctx: Context<SwapTokens>,
    amount_in: u64,
    min_amount_out: u64,
    input_is_x: bool,
) -> Result<()> {
    require!(amount_in > 0, AmmError::InvalidAmount);

    let (reserve_in, reserve_out) = if input_is_x {
        (ctx.accounts.vault_x.amount, ctx.accounts.vault_y.amount)
    } else {
        (ctx.accounts.vault_y.amount, ctx.accounts.vault_x.amount)
    };

    let (fee, amount_out) = calculate_swap(
        amount_in,
        reserve_in,
        reserve_out,
        ctx.accounts.pool.fee_bps,
    )?;

    require!(
        amount_out >= min_amount_out,
        AmmError::SlippageExceeded
    );

    let net_input = amount_in
        .checked_sub(fee)
        .ok_or(AmmError::Underflow)?;

    // Transfer the net input to the pool and the fee to the treasury.
    if input_is_x {
        let pool_transfer = Transfer {
            from: ctx.accounts.user_x.to_account_info(),
            to: ctx.accounts.vault_x.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                pool_transfer,
            ),
            net_input,
        )?;

        if fee > 0 {
            let fee_transfer = Transfer {
                from: ctx.accounts.user_x.to_account_info(),
                to: ctx.accounts.treasury_x.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            };

            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.key(),
                    fee_transfer,
                ),
                fee,
            )?;
        }
    } else {
        let pool_transfer = Transfer {
            from: ctx.accounts.user_y.to_account_info(),
            to: ctx.accounts.vault_y.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                pool_transfer,
            ),
            net_input,
        )?;

        if fee > 0 {
            let fee_transfer = Transfer {
                from: ctx.accounts.user_y.to_account_info(),
                to: ctx.accounts.treasury_y.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            };

            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.key(),
                    fee_transfer,
                ),
                fee,
            )?;
        }
    }

    // The pool PDA signs the transfer of output tokens to the user.
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

    let output_transfer = if input_is_x {
        Transfer {
            from: ctx.accounts.vault_y.to_account_info(),
            to: ctx.accounts.user_y.to_account_info(),
            authority: ctx.accounts.pool.to_account_info(),
        }
    } else {
        Transfer {
            from: ctx.accounts.vault_x.to_account_info(),
            to: ctx.accounts.user_x.to_account_info(),
            authority: ctx.accounts.pool.to_account_info(),
        }
    };

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            output_transfer,
            signer,
        ),
        amount_out,
    )?;

    msg!("Swap completed");
    msg!("Input amount: {}", amount_in);
    msg!("Fee sent to treasury: {}", fee);
    msg!("Output amount: {}", amount_out);

    Ok(())
}
