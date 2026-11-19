use anchor_lang::prelude::*;

use crate::constants::FEE_DENOMINATOR;
use crate::error::AmmError;

/// Calculates the swap fee and output amount.
///
/// Returns:
/// (fee_amount, amount_out)
pub fn calculate_swap(
    amount_in: u64,
    reserve_in: u64,
    reserve_out: u64,
    fee_bps: u16,
) -> Result<(u64, u64)> {
    require!(amount_in > 0, AmmError::InvalidAmount);
    require!(
        reserve_in > 0 && reserve_out > 0,
        AmmError::ZeroReserves
    );
    require!(fee_bps <= 1_000, AmmError::InvalidFee);

    let fee = (amount_in as u128)
        .checked_mul(fee_bps as u128)
        .ok_or(AmmError::Overflow)?
        .checked_div(FEE_DENOMINATOR as u128)
        .ok_or(AmmError::Overflow)?;

    let fee = u64::try_from(fee).map_err(|_| AmmError::Overflow)?;

    let net_input = amount_in
        .checked_sub(fee)
        .ok_or(AmmError::Underflow)?;

    let numerator = (reserve_out as u128)
        .checked_mul(net_input as u128)
        .ok_or(AmmError::Overflow)?;

    let denominator = (reserve_in as u128)
        .checked_add(net_input as u128)
        .ok_or(AmmError::Overflow)?;

    let amount_out = numerator
        .checked_div(denominator)
        .ok_or(AmmError::Overflow)?;

    let amount_out = u64::try_from(amount_out)
        .map_err(|_| AmmError::Overflow)?;

    require!(amount_out > 0, AmmError::InsufficientLiquidity);
    require!(amount_out < reserve_out, AmmError::InsufficientLiquidity);

    Ok((fee, amount_out))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_swap_with_fee() {
        let (fee, amount_out) =
            calculate_swap(1_000, 10_000, 20_000, 30).unwrap();

        assert_eq!(fee, 3);
        assert_eq!(amount_out, 1_813);
    }

    #[test]
    fn test_calculate_swap_zero_input_fails() {
        let result = calculate_swap(0, 10_000, 20_000, 30);

        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_swap_zero_reserves_fails() {
        let result = calculate_swap(1_000, 0, 20_000, 30);

        assert!(result.is_err());
    }
}