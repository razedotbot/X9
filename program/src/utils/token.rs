use anchor_lang::prelude::*;
use anchor_spl::token_2022;
use crate::shared::{spl_token, token_2022 as token_2022_program};

use crate::{
    constants::{
        MINT_DECIMALS_OFFSET, MINT_MIN_LEN, TOKEN_ACCOUNT_MIN_LEN, TOKEN_AMOUNT_OFFSET,
        TOKEN_OWNER_OFFSET,
    },
    error::ErrorCode,
};

/// `decimals` of a mint, read from raw bytes at [`MINT_DECIMALS_OFFSET`].
pub fn mint_decimals(ai: &AccountInfo) -> Result<u8> {
    require!(
        (ai.owner == &spl_token::ID || ai.owner == &token_2022_program::ID)
            && ai.data_len() >= MINT_MIN_LEN,
        ErrorCode::InvalidTokenAccount
    );
    let data = ai.try_borrow_data()?;
    Ok(data[MINT_DECIMALS_OFFSET])
}

/// Address equality as 4 unaligned u64 XORs instead of a 32-byte memcmp.
#[inline(always)]
pub fn address_eq(a: &Pubkey, b: &Pubkey) -> bool {
    let (x, y) = (a.as_ref().as_ptr(), b.as_ref().as_ptr());
    // SAFETY: `Pubkey` is a `[u8; 32]`, so four unaligned 8-byte reads from each
    // pointer stay in bounds; `read_unaligned` needs no alignment guarantee.
    let diff = unsafe {
        ((x as *const u64).read_unaligned() ^ (y as *const u64).read_unaligned())
            | ((x.add(8) as *const u64).read_unaligned()
                ^ (y.add(8) as *const u64).read_unaligned())
            | ((x.add(16) as *const u64).read_unaligned()
                ^ (y.add(16) as *const u64).read_unaligned())
            | ((x.add(24) as *const u64).read_unaligned()
                ^ (y.add(24) as *const u64).read_unaligned())
    };
    diff == 0
}

/// True iff the account is token-program owned and at least the base size.
#[inline]
pub fn is_token_account(ai: &AccountInfo) -> bool {
    (address_eq(ai.owner, &spl_token::ID) || address_eq(ai.owner, &token_2022_program::ID))
        && ai.data_len() >= TOKEN_ACCOUNT_MIN_LEN
}

/// Owner field, for callers that already ran [`is_token_account`] on this account.
#[inline]
pub fn token_owner_prechecked(ai: &AccountInfo) -> Result<Pubkey> {
    let data = ai.try_borrow_data()?;
    let bytes: [u8; 32] = data[TOKEN_OWNER_OFFSET..TOKEN_OWNER_OFFSET + 32]
        .try_into()
        .map_err(|_| ErrorCode::InvalidTokenAccount)?;
    Ok(Pubkey::new_from_array(bytes))
}

/// Mint field of a token account (`[0..32]`).
pub fn token_mint(ai: &AccountInfo) -> Result<Pubkey> {
    require!(is_token_account(ai), ErrorCode::InvalidTokenAccount);
    let data = ai.try_borrow_data()?;
    let bytes: [u8; 32] = data[..32]
        .try_into()
        .map_err(|_| ErrorCode::InvalidTokenAccount)?;
    Ok(Pubkey::new_from_array(bytes))
}

/// Owner field of a token account (`[32..64]`).
pub fn token_owner(ai: &AccountInfo) -> Result<Pubkey> {
    require!(is_token_account(ai), ErrorCode::InvalidTokenAccount);
    let data = ai.try_borrow_data()?;
    let bytes: [u8; 32] = data[TOKEN_OWNER_OFFSET..TOKEN_OWNER_OFFSET + 32]
        .try_into()
        .map_err(|_| ErrorCode::InvalidTokenAccount)?;
    Ok(Pubkey::new_from_array(bytes))
}

/// Amount field of a token account (`[64..72]` LE).
pub fn token_balance(ai: &AccountInfo) -> Result<u64> {
    require!(is_token_account(ai), ErrorCode::InvalidTokenAccount);
    let data = ai.try_borrow_data()?;
    let bytes: [u8; 8] = data[TOKEN_AMOUNT_OFFSET..TOKEN_AMOUNT_OFFSET + 8]
        .try_into()
        .map_err(|_| ErrorCode::InvalidTokenAccount)?;
    Ok(u64::from_le_bytes(bytes))
}

/// Move `amount` of a token: plain `transfer` on classic SPL (cheaper),
/// `transfer_checked` on Token-2022. `signer_seeds` is `None` when `authority`
/// is a transaction signer.
#[allow(clippy::too_many_arguments)]
pub fn transfer_checked<'info>(
    token_program: AccountInfo<'info>,
    from: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    to: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    amount: u64,
    decimals: u8,
    signer_seeds: Option<&[&[&[u8]]]>,
) -> Result<()> {
    if amount == 0 {
        return Ok(());
    }
    if token_program.key() == spl_token::ID {
        #[allow(deprecated)]
        let accounts = anchor_spl::token::Transfer { from, to, authority };
        let cpi = match signer_seeds {
            Some(seeds) => CpiContext::new_with_signer(token_program, accounts, seeds),
            None => CpiContext::new(token_program, accounts),
        };
        #[allow(deprecated)]
        return anchor_spl::token::transfer(cpi, amount);
    }
    let accounts = token_2022::TransferChecked { from, mint, to, authority };
    let cpi = match signer_seeds {
        Some(seeds) => CpiContext::new_with_signer(token_program, accounts, seeds),
        None => CpiContext::new(token_program, accounts),
    };
    token_2022::transfer_checked(cpi, amount, decimals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::solana_program::account_info::AccountInfo;

    fn spl_id() -> Pubkey {
        spl_token::ID
    }
    fn t22_id() -> Pubkey {
        token_2022_program::ID
    }

    /// A 165-byte token account: mint[0..32], owner[32..64], amount[64..72] LE.
    fn token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
        let mut data = vec![0u8; 165];
        data[..32].copy_from_slice(mint.as_ref());
        data[TOKEN_OWNER_OFFSET..TOKEN_OWNER_OFFSET + 32].copy_from_slice(owner.as_ref());
        data[TOKEN_AMOUNT_OFFSET..TOKEN_AMOUNT_OFFSET + 8].copy_from_slice(&amount.to_le_bytes());
        data[108] = 1;
        data
    }

    /// An 82-byte base mint with `decimals` at offset 44.
    fn mint_data(decimals: u8) -> Vec<u8> {
        let mut data = vec![0u8; MINT_MIN_LEN];
        data[MINT_DECIMALS_OFFSET] = decimals;
        data[45] = 1;
        data
    }

    /// An AccountInfo borrowing the provided key/owner/lamports/data.
    fn account_info<'a>(
        key: &'a Pubkey,
        owner: &'a Pubkey,
        lamports: &'a mut u64,
        data: &'a mut [u8],
    ) -> AccountInfo<'a> {
        AccountInfo::new(key, false, false, lamports, data, owner, false, 0)
    }

    /// The readers pick up mint, owner and amount at their fixed offsets.
    #[test]
    fn token_owner_and_balance_at_fixed_offsets() {
        let key = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let amount = 123_456_789_012_u64;
        let owner_prog = spl_id();
        let mut lamports = 2_039_280u64;
        let mut data = token_account_data(&mint, &owner, amount);
        let ai = account_info(&key, &owner_prog, &mut lamports, &mut data);

        assert!(is_token_account(&ai));
        assert_eq!(token_mint(&ai).unwrap(), mint);
        assert_eq!(token_owner(&ai).unwrap(), owner);
        assert_eq!(token_balance(&ai).unwrap(), amount);
    }

    /// Token-2022 shares the offsets; only the owning program differs.
    #[test]
    fn token_readers_work_for_token_2022_owner() {
        let key = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let owner_prog = t22_id();
        let mut lamports = 1u64;
        let mut data = token_account_data(&mint, &owner, u64::MAX);
        let ai = account_info(&key, &owner_prog, &mut lamports, &mut data);

        assert!(is_token_account(&ai));
        assert_eq!(token_owner(&ai).unwrap(), owner);
        assert_eq!(token_balance(&ai).unwrap(), u64::MAX);
    }

    /// An account owned by another program is not a token account.
    #[test]
    fn non_token_program_owner_rejected() {
        let key = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let bogus_owner = crate::shared::system_program::ID;
        let mut lamports = 1u64;
        let mut data = token_account_data(&mint, &owner, 10);
        let ai = account_info(&key, &bogus_owner, &mut lamports, &mut data);

        assert!(!is_token_account(&ai));
        assert!(token_owner(&ai).is_err());
        assert!(token_balance(&ai).is_err());
        assert!(token_mint(&ai).is_err());
    }

    /// 72 bytes is a token account, 71 is not, and the readers never slice past.
    #[test]
    fn min_len_boundary_for_token_account() {
        assert_eq!(TOKEN_ACCOUNT_MIN_LEN, 72);
        let key = Pubkey::new_unique();
        let owner_prog = spl_id();

        let mut lamports = 1u64;
        let mut data72 = vec![0u8; 72];
        data72[TOKEN_AMOUNT_OFFSET..72].copy_from_slice(&7u64.to_le_bytes());
        let ai = account_info(&key, &owner_prog, &mut lamports, &mut data72);
        assert!(is_token_account(&ai));
        assert_eq!(token_balance(&ai).unwrap(), 7);

        let mut lamports2 = 1u64;
        let mut data71 = vec![0u8; 71];
        let ai2 = account_info(&key, &owner_prog, &mut lamports2, &mut data71);
        assert!(!is_token_account(&ai2));
        assert!(token_balance(&ai2).is_err());
        assert!(token_owner(&ai2).is_err());
    }

    /// Decimals read back at offset 44 for both a 6- and a 9-decimal mint.
    #[test]
    fn mint_decimals_at_offset_44_and_min_len() {
        assert_eq!(MINT_DECIMALS_OFFSET, 44);
        assert_eq!(MINT_MIN_LEN, 82);
        let key = Pubkey::new_unique();
        let owner_prog = spl_id();

        let mut lamports = 1u64;
        let mut data = mint_data(6);
        let ai = account_info(&key, &owner_prog, &mut lamports, &mut data);
        assert_eq!(mint_decimals(&ai).unwrap(), 6);

        let mut lamports9 = 1u64;
        let mut data9 = mint_data(9);
        let ai9 = account_info(&key, &owner_prog, &mut lamports9, &mut data9);
        assert_eq!(mint_decimals(&ai9).unwrap(), 9);
    }

    /// A short buffer or a wrong owning program errors instead of panicking.
    #[test]
    fn mint_decimals_rejects_short_or_wrong_owner() {
        let key = Pubkey::new_unique();
        let owner_prog = spl_id();

        let mut lamports = 1u64;
        let mut short = vec![0u8; MINT_MIN_LEN - 1];
        let ai = account_info(&key, &owner_prog, &mut lamports, &mut short);
        assert!(mint_decimals(&ai).is_err());

        let bogus = crate::shared::system_program::ID;
        let mut lamports2 = 1u64;
        let mut data = mint_data(8);
        let ai2 = account_info(&key, &bogus, &mut lamports2, &mut data);
        assert!(mint_decimals(&ai2).is_err());
    }

    /// `address_eq` agrees with `==` for a bit flipped at every byte position.
    #[test]
    fn address_eq_matches_derived_partial_eq() {
        let a = Pubkey::new_unique();
        assert!(address_eq(&a, &a.clone()));
        assert!(!address_eq(&a, &Pubkey::new_unique()));

        assert!(address_eq(
            &Pubkey::new_from_array([0u8; 32]),
            &Pubkey::new_from_array([0u8; 32])
        ));
        assert!(address_eq(
            &Pubkey::new_from_array([0xFF; 32]),
            &Pubkey::new_from_array([0xFF; 32])
        ));

        for i in 0..32usize {
            let mut bytes = [7u8; 32];
            let base = Pubkey::new_from_array(bytes);
            bytes[i] ^= 0x01;
            let flipped = Pubkey::new_from_array(bytes);
            assert!(!address_eq(&base, &flipped), "byte {i} not compared");
            assert_eq!(address_eq(&base, &flipped), base == flipped, "byte {i}");
        }
    }
}
