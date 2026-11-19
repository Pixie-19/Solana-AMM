use anchor_lang::prelude::*;

#[error_code]
pub enum AmmError {
    #[msg("Unauthorized access")]
    Unauthorized,

    #[msg("Invalid token mint")]
    InvalidMint,

    #[msg("Invalid fee configuration")]
    InvalidFee,

    #[msg("Pool has insufficient liquidity")]
    InsufficientLiquidity,

    #[msg("Amount must be greater than zero")]
    InvalidAmount,

    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,

    #[msg("Arithmetic overflow")]
    Overflow,

    #[msg("Arithmetic underflow")]
    Underflow,

    #[msg("Insufficient LP tokens")]
    InsufficientLpTokens,

    #[msg("Pool reserves cannot be zero")]
    ZeroReserves,

    #[msg("Invalid treasury account")]
    InvalidTreasury,
}