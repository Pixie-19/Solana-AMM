
pub mod constants;
pub mod error;
pub mod instructions;
pub mod math;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("FhvbCcDV98D8pVothJhnVFSCjvAnZMX6zjQBZdowuypF");

#[program]
pub mod solana_amm {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        fee_bps: u16,
    ) -> Result<()> {
        instructions::initialize::handle_initialize(ctx, fee_bps)
    }

    pub fn deposit(
        ctx: Context<DepositLiquidity>,
        max_x: u64,
        max_y: u64,
        min_lp: u64,
    ) -> Result<()> {
        instructions::deposit::handle_deposit(
            ctx,
            max_x,
            max_y,
            min_lp,
        )
    }

    pub fn withdraw(
        ctx: Context<WithdrawLiquidity>,
        lp_amount: u64,
        min_x: u64,
        min_y: u64,
    ) -> Result<()> {
        instructions::withdraw::handle_withdraw(
            ctx,
            lp_amount,
            min_x,
            min_y,
        )
    }

    pub fn swap(
        ctx: Context<SwapTokens>,
        amount_in: u64,
        min_amount_out: u64,
        input_is_x: bool,
    ) -> Result<()> {
        instructions::swap::handle_swap(
            ctx,
            amount_in,
            min_amount_out,
            input_is_x,
        )
    }
}
