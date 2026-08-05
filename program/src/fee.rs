use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};

use crate::{
    constants::FEE_BPS_DENOMINATOR,
    error::ErrorCode,
    utils::{is_token_account, transfer_checked},
};

/// Move a native-SOL buffer from the payer to the SA, covering native fees or
/// account rent a venue charges on top of the swap. No-op when `topup == 0`.
pub fn fund_sa_native_buffer<'info>(
    system_program: AccountInfo<'info>,
    payer: AccountInfo<'info>,
    sa_authority: AccountInfo<'info>,
    topup: u64,
) -> Result<()> {
    if topup == 0 {
        return Ok(());
    }
    system_program::transfer(
        CpiContext::new(system_program, Transfer { from: payer, to: sa_authority }),
        topup,
    )
}

/// Return the SA's whole residual native balance to the payer at the end of a
/// route. The SA is a data-less PDA with no standing baseline, so it drains to
/// zero. No-op when the SA holds nothing.
pub fn sweep_sa_native_residual<'info>(
    system_program: AccountInfo<'info>,
    sa_authority: AccountInfo<'info>,
    payer: AccountInfo<'info>,
    sa_seeds: &[&[&[u8]]],
) -> Result<()> {
    let residual = sa_authority.lamports();
    if residual == 0 {
        return Ok(());
    }
    system_program::transfer(
        CpiContext::new_with_signer(
            system_program,
            Transfer { from: sa_authority, to: payer },
            sa_seeds,
        ),
        residual,
    )
}

/// `floor(gross * fee_bps / 10_000)`, computed over u128.
pub fn compute_fee(gross: u64, fee_bps: u64) -> Result<u64> {
    let fee = (gross as u128)
        .checked_mul(fee_bps as u128)
        .and_then(|v| v.checked_div(FEE_BPS_DENOMINATOR as u128))
        .ok_or(ErrorCode::CalculationError)?;
    u64::try_from(fee).map_err(|_| ErrorCode::CalculationError.into())
}

/// Move `fee` of `mint` from an SA-owned token account to the fee ATA.
#[allow(clippy::too_many_arguments)]
pub fn charge_token_fee<'info>(
    fee: u64,
    token_program: AccountInfo<'info>,
    sa_account: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    fee_account: AccountInfo<'info>,
    sa_authority: AccountInfo<'info>,
    decimals: u8,
    sa_seeds: &[&[&[u8]]],
) -> Result<()> {
    if fee == 0 {
        return Ok(());
    }
    require!(is_token_account(&fee_account), ErrorCode::InvalidFeeAccount);
    require_keys_neq!(fee_account.key(), sa_account.key(), ErrorCode::InvalidFeeAccount);

    transfer_checked(
        token_program,
        sa_account,
        mint,
        fee_account,
        sa_authority,
        fee,
        decimals,
        Some(sa_seeds),
    )
}

/// Move `fee` native lamports from the SA PDA to the fee wallet.
pub fn charge_native_fee<'info>(
    fee: u64,
    system_program: AccountInfo<'info>,
    sa_authority: AccountInfo<'info>,
    fee_account: AccountInfo<'info>,
    sa_seeds: &[&[&[u8]]],
) -> Result<()> {
    if fee == 0 {
        return Ok(());
    }
    require_keys_neq!(fee_account.key(), sa_authority.key(), ErrorCode::InvalidFeeAccount);
    system_program::transfer(
        CpiContext::new_with_signer(
            system_program,
            Transfer { from: sa_authority, to: fee_account },
            sa_seeds,
        ),
        fee,
    )
}

/// Token fee taken from the user's own account under the payer's signature.
#[allow(clippy::too_many_arguments)]
pub fn charge_user_token_fee<'info>(
    fee: u64,
    token_program: AccountInfo<'info>,
    user_account: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    fee_account: AccountInfo<'info>,
    payer: AccountInfo<'info>,
    decimals: u8,
) -> Result<()> {
    if fee == 0 {
        return Ok(());
    }
    require!(is_token_account(&fee_account), ErrorCode::InvalidFeeAccount);
    require_keys_neq!(fee_account.key(), user_account.key(), ErrorCode::InvalidFeeAccount);
    transfer_checked(
        token_program,
        user_account,
        mint,
        fee_account,
        payer,
        fee,
        decimals,
        None,
    )
}

/// Native fee taken from the payer's own account.
pub fn charge_user_native_fee<'info>(
    fee: u64,
    system_program: AccountInfo<'info>,
    payer: AccountInfo<'info>,
    fee_account: AccountInfo<'info>,
) -> Result<()> {
    if fee == 0 {
        return Ok(());
    }
    require_keys_neq!(fee_account.key(), payer.key(), ErrorCode::InvalidFeeAccount);
    system_program::transfer(
        CpiContext::new(system_program, Transfer { from: payer, to: fee_account }),
        fee,
    )
}

/// Boundary currency and lean/proxy mode; selects the `charge_*` leaf.
pub enum BoundaryFeeSource<'info> {
    Native { lean: bool },
    Token {
        lean: bool,
        token_program: AccountInfo<'info>,
        token_account: AccountInfo<'info>,
        mint: AccountInfo<'info>,
        decimals: u8,
    },
}

/// Dispatch to the `charge_*` leaf for `(native|token) × (lean|proxy)`.
#[allow(clippy::too_many_arguments)]
pub fn charge_boundary_fee<'info>(
    fee: u64,
    source: BoundaryFeeSource<'info>,
    system_program: AccountInfo<'info>,
    fee_account: AccountInfo<'info>,
    payer: AccountInfo<'info>,
    sa_authority: AccountInfo<'info>,
    sa_seeds: &[&[&[u8]]],
) -> Result<()> {
    match source {
        BoundaryFeeSource::Native { lean: true } => {
            charge_user_native_fee(fee, system_program, payer, fee_account)
        }
        BoundaryFeeSource::Native { lean: false } => {
            charge_native_fee(fee, system_program, sa_authority, fee_account, sa_seeds)
        }
        BoundaryFeeSource::Token {
            lean: true,
            token_program,
            token_account,
            mint,
            decimals,
        } => charge_user_token_fee(
            fee,
            token_program,
            token_account,
            mint,
            fee_account,
            payer,
            decimals,
        ),
        BoundaryFeeSource::Token {
            lean: false,
            token_program,
            token_account,
            mint,
            decimals,
        } => charge_token_fee(
            fee,
            token_program,
            token_account,
            mint,
            fee_account,
            sa_authority,
            decimals,
            sa_seeds,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fee floors and stays exact near `u64::MAX`.
    #[test]
    fn fee_floor() {
        assert_eq!(compute_fee(1_000_000, 25).unwrap(), 2_500);
        assert_eq!(compute_fee(399, 25).unwrap(), 0);
        assert_eq!(compute_fee(0, 100).unwrap(), 0);
        assert_eq!(compute_fee(u64::MAX, 100).unwrap(), (u64::MAX as u128 * 100 / 10_000) as u64);
    }

    /// Zero bps is a zero fee at any gross.
    #[test]
    fn fee_zero_bps_is_zero() {
        assert_eq!(compute_fee(1_000_000_000, 0).unwrap(), 0);
        assert_eq!(compute_fee(u64::MAX, 0).unwrap(), 0);
    }

    /// At the cap the fee is exactly a tenth of gross.
    #[test]
    fn fee_at_platform_cap() {
        use crate::constants::MAX_PLATFORM_FEE_BPS;
        assert_eq!(MAX_PLATFORM_FEE_BPS, 1_000);
        assert_eq!(
            compute_fee(1_000_000_000, MAX_PLATFORM_FEE_BPS as u64).unwrap(),
            100_000_000
        );
        let expected = (u64::MAX as u128 * MAX_PLATFORM_FEE_BPS as u128 / 10_000) as u64;
        assert_eq!(
            compute_fee(u64::MAX, MAX_PLATFORM_FEE_BPS as u64).unwrap(),
            expected
        );
    }

    /// Exact division and the smallest non-zero fee.
    #[test]
    fn fee_exact_and_min_nonzero() {
        assert_eq!(compute_fee(7_777, 10_000).unwrap(), 7_777);
        assert_eq!(compute_fee(9_999, 1).unwrap(), 0);
        assert_eq!(compute_fee(10_000, 1).unwrap(), 1);
        assert_eq!(compute_fee(20_001, 1).unwrap(), 2);
    }
}
