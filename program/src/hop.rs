use anchor_lang::{
    prelude::*,
    solana_program::{
        instruction::{AccountMeta, Instruction},
        program::invoke_signed_unchecked,
    },
};
use crate::shared::HopMeta;

use crate::{
    constants::{
        ACTUAL_IN_LOWER_BOUND_DEN, ACTUAL_IN_LOWER_BOUND_NUM, MAX_ACCOUNTS_PER_HOP, ZERO_ADDRESS,
    },
    error::ErrorCode,
    state::RouterConfig,
    utils::{address_eq, is_token_account, token_balance, token_owner, token_owner_prechecked},
};

/// One side of a hop: an SPL/Token-2022 account, or native lamports on the PDA.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LegKind {
    Token,
    Native,
}

/// Input/output delta bounds for one hop, by direction.
#[derive(Clone, Copy)]
pub enum HopBounds {
    ExactIn { amount_in: u64 },
    ExactOut { max_in: u64, amount_out: u64 },
}

/// Run one hop: resolve accounts, splice the amount, CPI the venue, measure the
/// balance deltas. Returns `(actual_in, actual_out, destination_key)`.
#[allow(clippy::too_many_arguments)]
pub fn execute_hop<'info>(
    hop: &mut HopMeta,
    remaining: &'info [AccountInfo<'info>],
    bounds: HopBounds,
    sa_authority: &Pubkey,
    config: &RouterConfig,
    hop_index: usize,
    prev_to: Pubkey,
    sa_seeds: &[&[&[u8]]],
    source_kind: LegKind,
    dest_kind: LegKind,
    extra_allowed: &[Pubkey],
    splice_xor_mask: &[u8; 8],
    native_tolerance: u64,
    allowed_user_dest: Option<Pubkey>,
    lean_payer: Option<Pubkey>,
) -> Result<(u64, u64, Pubkey)> {
    let program_ai = remaining
        .get(hop.program_id_index as usize)
        .ok_or(ErrorCode::InvalidAccountsLength)?;
    require!(config.is_allowed(program_ai.key), ErrorCode::VenueNotAllowed);

    let source_ai = remaining
        .get(hop.source_token_index as usize)
        .ok_or(ErrorCode::InvalidAccountsLength)?;
    let destination_ai = remaining
        .get(hop.destination_token_index as usize)
        .ok_or(ErrorCode::InvalidAccountsLength)?;
    require_keys_neq!(source_ai.key(), destination_ai.key(), ErrorCode::InvalidHopAccounts);

    let owned_by = |o: Pubkey| o == *sa_authority || lean_payer == Some(o);
    match source_kind {
        LegKind::Token => require!(owned_by(token_owner(source_ai)?), ErrorCode::InvalidSourceTokenAccount),
        LegKind::Native => require!(owned_by(source_ai.key()), ErrorCode::InvalidSourceTokenAccount),
    }
    match dest_kind {
        LegKind::Token => {
            let dest_ok = owned_by(token_owner(destination_ai)?)
                || allowed_user_dest == Some(destination_ai.key());
            require!(dest_ok, ErrorCode::InvalidDestinationTokenAccount);
        }
        LegKind::Native => {
            let dest_ok = owned_by(destination_ai.key())
                || allowed_user_dest == Some(destination_ai.key());
            require!(dest_ok, ErrorCode::InvalidDestinationTokenAccount);
        }
    }

    // Hop i > 0 consumes hop i-1's output.
    if hop_index > 0 {
        require!(prev_to != ZERO_ADDRESS, ErrorCode::InvalidHopChain);
        require_keys_eq!(source_ai.key(), prev_to, ErrorCode::InvalidHopChain);
    }

    // Splice the running amount into the venue data (exact-in only; exact-out
    // carries its own amount pair and is passed verbatim).
    match bounds {
        HopBounds::ExactIn { amount_in } => {
            if hop.amount_in_offset >= 0 {
                let off = hop.amount_in_offset as usize;
                let end = off.checked_add(8).ok_or(ErrorCode::CalculationError)?;
                require!(end <= hop.data.len(), ErrorCode::InvalidInstructionData);
                let amount_le = amount_in.to_le_bytes();
                for (i, b) in hop.data[off..end].iter_mut().enumerate() {
                    *b = amount_le[i] ^ splice_xor_mask[i];
                }
            }
        }
        HopBounds::ExactOut { .. } => require!(
            hop.amount_in_offset < 0,
            ErrorCode::InvalidInstructionData
        ),
    }

    require!(
        !hop.accounts.is_empty() && hop.accounts.len() <= MAX_ACCOUNTS_PER_HOP,
        ErrorCode::InvalidAccountsLength
    );
    let mut metas: Vec<AccountMeta> = Vec::with_capacity(hop.accounts.len());
    let mut infos: Vec<AccountInfo<'info>> = Vec::with_capacity(hop.accounts.len());
    for a in hop.accounts.iter() {
        let ai = remaining
            .get(a.index as usize)
            .ok_or(ErrorCode::InvalidAccountsLength)?;
        metas.push(AccountMeta {
            pubkey: ai.key(),
            is_signer: a.is_signer(),
            is_writable: a.is_writable(),
        });
        infos.push(ai.clone());
    }

    enforce_token_allowlist(
        &infos,
        sa_authority,
        lean_payer,
        source_ai.key(),
        destination_ai.key(),
        extra_allowed,
    )?;

    let leg_balance = |ai: &AccountInfo, kind: LegKind| -> Result<u64> {
        match kind {
            LegKind::Token => token_balance(ai),
            LegKind::Native => Ok(ai.lamports()),
        }
    };

    let before_source = leg_balance(source_ai, source_kind)?;
    let before_destination = leg_balance(destination_ai, dest_kind)?;

    let ix = Instruction {
        program_id: program_ai.key(),
        accounts: metas,
        // `hop.data` is dead after the invoke — take it instead of cloning.
        data: std::mem::take(&mut hop.data),
    };
    // Unchecked form: skips the safe wrapper's per-meta scan and borrow probes,
    // worth ~2-3k CU per CPI. No data borrow is held across this call.
    invoke_signed_unchecked(&ix, &infos, sa_seeds)?;

    let after_source = leg_balance(source_ai, source_kind)?;
    let after_destination = leg_balance(destination_ai, dest_kind)?;

    let actual_in = before_source
        .checked_sub(after_source)
        .ok_or(ErrorCode::CalculationError)?;
    let actual_out = after_destination
        .checked_sub(before_destination)
        .ok_or(ErrorCode::CalculationError)?;

    check_hop_bounds(bounds, source_kind, native_tolerance, actual_in, actual_out)?;

    Ok((actual_in, actual_out, destination_ai.key()))
}

/// Delta bounds for one hop: exact-in floors the spend at 90% and the output at
/// 1; exact-out caps the spend and floors the output at `amount_out`.
#[inline]
fn check_hop_bounds(
    bounds: HopBounds,
    source_kind: LegKind,
    native_tolerance: u64,
    actual_in: u64,
    actual_out: u64,
) -> Result<()> {
    // A native source rides the off-chain-computed rent/fee slack; a token
    // source moves exactly what the venue asks for.
    let cap = |base: u64| match source_kind {
        LegKind::Token => base,
        LegKind::Native => base.saturating_add(native_tolerance),
    };
    match bounds {
        HopBounds::ExactIn { amount_in } => {
            let min_in = u64::try_from(
                (amount_in as u128)
                    .checked_mul(ACTUAL_IN_LOWER_BOUND_NUM)
                    .and_then(|v| v.checked_div(ACTUAL_IN_LOWER_BOUND_DEN))
                    .ok_or(ErrorCode::CalculationError)?,
            )
            .map_err(|_| ErrorCode::CalculationError)?;
            require!(
                actual_in <= cap(amount_in) && actual_in >= min_in,
                ErrorCode::InvalidActualAmountIn
            );
            require!(actual_out > 0, ErrorCode::AmountOutMustBeGreaterThanZero);
        }
        HopBounds::ExactOut { max_in, amount_out } => {
            require!(actual_in <= cap(max_in), ErrorCode::MaxAmountInExceeded);
            require!(actual_out >= amount_out, ErrorCode::ExactOutNotReached);
        }
    }
    Ok(())
}

/// Walk the CPI account list and reject any protected token account that is not
/// this hop's source, destination or a vouched extra.
fn enforce_token_allowlist(
    infos: &[AccountInfo],
    sa_authority: &Pubkey,
    protected_user: Option<Pubkey>,
    allowed_source: Pubkey,
    allowed_destination: Pubkey,
    extra_allowed: &[Pubkey],
) -> Result<()> {
    for ai in infos.iter() {
        if !is_token_account(ai) {
            continue;
        }
        let key = ai.key();
        // `is_token_account` just passed — reuse it instead of re-testing.
        let owner = token_owner_prechecked(ai)?;
        if token_account_rejected(
            &owner,
            &key,
            sa_authority,
            protected_user,
            &allowed_source,
            &allowed_destination,
            extra_allowed,
        ) {
            return Err(ErrorCode::UnexpectedSaTokenAccount.into());
        }
    }
    Ok(())
}

/// True iff the account belongs to a protected party and is not a designated leg.
#[inline]
fn token_account_rejected(
    owner: &Pubkey,
    key: &Pubkey,
    sa_authority: &Pubkey,
    protected_user: Option<Pubkey>,
    allowed_source: &Pubkey,
    allowed_destination: &Pubkey,
    extra_allowed: &[Pubkey],
) -> bool {
    let is_protected = address_eq(owner, sa_authority)
        || protected_user.as_ref().is_some_and(|p| address_eq(p, owner));
    is_protected
        && !address_eq(key, allowed_source)
        && !address_eq(key, allowed_destination)
        && !extra_allowed.iter().any(|e| address_eq(e, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(b: u8) -> Pubkey {
        Pubkey::new_from_array([b; 32])
    }

    /// Anchor error number behind a failed bound, so a test pins which one broke.
    fn code(e: anchor_lang::error::Error) -> u32 {
        match e {
            anchor_lang::error::Error::AnchorError(ae) => ae.error_code_number,
            other => panic!("expected AnchorError, got {other:?}"),
        }
    }

    /// A representative `sa_native_topup` — roughly one token account's rent.
    const TOL: u64 = 2_100_000;

    /// Exact-in accepts [90%, amount_in], widened by the tolerance only on native.
    #[test]
    fn exact_in_spend_floor_and_cap() {
        let ei = |amount_in| HopBounds::ExactIn { amount_in };
        assert!(check_hop_bounds(ei(1_000), LegKind::Token, 0, 1_000, 1).is_ok());
        assert!(check_hop_bounds(ei(1_000), LegKind::Token, 0, 900, 1).is_ok());
        let e = check_hop_bounds(ei(1_000), LegKind::Token, 0, 899, 1).unwrap_err();
        assert_eq!(code(e), u32::from(ErrorCode::InvalidActualAmountIn));
        let e = check_hop_bounds(ei(1_000), LegKind::Token, TOL, 1_001, 1).unwrap_err();
        assert_eq!(code(e), u32::from(ErrorCode::InvalidActualAmountIn));
        assert!(check_hop_bounds(ei(1_000), LegKind::Native, TOL, 1_000 + TOL, 1).is_ok());
        let e = check_hop_bounds(ei(1_000), LegKind::Native, TOL, 1_001 + TOL, 1).unwrap_err();
        assert_eq!(code(e), u32::from(ErrorCode::InvalidActualAmountIn));
    }

    /// Exact-in rejects a zero output.
    #[test]
    fn exact_in_requires_nonzero_output() {
        let e = check_hop_bounds(HopBounds::ExactIn { amount_in: 1_000 }, LegKind::Token, 0, 1_000, 0)
            .unwrap_err();
        assert_eq!(code(e), u32::from(ErrorCode::AmountOutMustBeGreaterThanZero));
    }

    /// Exact-out has no spend floor — landing under `max_in` is the normal case.
    #[test]
    fn exact_out_has_no_spend_floor() {
        let eo = HopBounds::ExactOut { max_in: 1_000_000, amount_out: 500 };
        assert!(check_hop_bounds(eo, LegKind::Token, 0, 1, 500).is_ok());
        assert!(check_hop_bounds(eo, LegKind::Native, TOL, 0, 500).is_ok());
    }

    /// Exact-out caps the spend at `max_in`, widened by the tolerance on native.
    #[test]
    fn exact_out_caps_spend() {
        let eo = HopBounds::ExactOut { max_in: 1_000, amount_out: 5 };
        assert!(check_hop_bounds(eo, LegKind::Token, 0, 1_000, 5).is_ok());
        let e = check_hop_bounds(eo, LegKind::Token, TOL, 1_001, 5).unwrap_err();
        assert_eq!(code(e), u32::from(ErrorCode::MaxAmountInExceeded));
        assert!(check_hop_bounds(eo, LegKind::Native, TOL, 1_000 + TOL, 5).is_ok());
        let e = check_hop_bounds(eo, LegKind::Native, TOL, 1_001 + TOL, 5).unwrap_err();
        assert_eq!(code(e), u32::from(ErrorCode::MaxAmountInExceeded));
    }

    /// Exact-out floors the output at `amount_out`, inclusive.
    #[test]
    fn exact_out_floors_output() {
        let eo = HopBounds::ExactOut { max_in: 1_000, amount_out: 500 };
        let e = check_hop_bounds(eo, LegKind::Token, 0, 900, 499).unwrap_err();
        assert_eq!(code(e), u32::from(ErrorCode::ExactOutNotReached));
        assert!(check_hop_bounds(eo, LegKind::Token, 0, 900, 500).is_ok());
    }

    /// With `protected_user = None` only SA-owned accounts are constrained.
    #[test]
    fn guard_sa_only_on_proxy_routes() {
        let sa = pk(1);
        let src = pk(10);
        let dst = pk(11);
        let extra = pk(12);
        let pool = pk(20);
        let user = pk(30);

        assert!(token_account_rejected(&sa, &pk(99), &sa, None, &src, &dst, &[extra]));
        assert!(!token_account_rejected(&sa, &src, &sa, None, &src, &dst, &[extra]));
        assert!(!token_account_rejected(&sa, &dst, &sa, None, &src, &dst, &[extra]));
        assert!(!token_account_rejected(&sa, &extra, &sa, None, &src, &dst, &[extra]));
        assert!(!token_account_rejected(&pool, &pk(99), &sa, None, &src, &dst, &[extra]));
        assert!(!token_account_rejected(&user, &pk(99), &sa, None, &src, &dst, &[extra]));
    }

    /// With `protected_user = Some(user)` the payer is constrained alongside the SA.
    #[test]
    fn guard_protects_direct_user() {
        let sa = pk(1);
        let user = pk(30);
        let src = pk(10);
        let dst = pk(11);
        let pool = pk(20);

        assert!(token_account_rejected(&user, &pk(99), &sa, Some(user), &src, &dst, &[]));
        assert!(!token_account_rejected(&user, &src, &sa, Some(user), &src, &dst, &[]));
        assert!(!token_account_rejected(&user, &dst, &sa, Some(user), &src, &dst, &[]));
        assert!(token_account_rejected(&sa, &pk(98), &sa, Some(user), &src, &dst, &[]));
        assert!(!token_account_rejected(&pool, &pk(97), &sa, Some(user), &src, &dst, &[]));
    }
}
