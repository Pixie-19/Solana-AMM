use anchor_lang::prelude::*;

pub const POOL_SEED: &[u8] = b"pool";
pub const LP_MINT_SEED: &[u8] = b"lp_mint";
pub const VAULT_X_SEED: &[u8] = b"vault_x";
pub const VAULT_Y_SEED: &[u8] = b"vault_y";

pub const FEE_DENOMINATOR: u64 = 10_000;
pub const MAX_FEE_BPS: u16 = 1_000;