use anchor_lang::prelude::*;

use crate::{
    constants::{MAX_PLATFORM_FEE_BPS, MAX_VENUES},
    error::ErrorCode,
};

/// Singleton router configuration, mutable by `admin`. Zero-copy: the venue
/// allowlist is too large for the BPF stack, so it is accessed by reference.
#[account(zero_copy)]
#[repr(C)]
pub struct RouterConfig {
    /// May mutate this config.
    pub admin: Pubkey,
    /// Dormant: fees are per-route. Kept for account-layout stability.
    pub fee_authority: Pubkey,
    /// Venue program-id allowlist. Only these may be invoked by a hop.
    pub venues: [Pubkey; MAX_VENUES],
    /// Dormant: superseded by the per-route fee. Kept for layout stability.
    pub fee_bps: u16,
    /// Number of populated entries in `venues`.
    pub venue_count: u16,
    /// 0 = active, 1 = paused.
    pub paused: u8,
    /// Canonical bump of this PDA.
    pub bump: u8,
}

impl RouterConfig {
    pub const SIZE: usize = 8   // discriminator
        + 32                    // admin
        + 32                    // fee_authority
        + 32 * MAX_VENUES       // venues
        + 2                     // fee_bps
        + 2                     // venue_count
        + 1                     // paused
        + 1; // bump

    #[inline]
    pub fn is_paused(&self) -> bool {
        self.paused != 0
    }

    #[inline]
    pub fn is_allowed(&self, program_id: &Pubkey) -> bool {
        self.venues[..self.venue_count as usize].contains(program_id)
    }

    /// Append a venue to the allowlist, rejecting duplicates and forbidden ids.
    pub fn add_venue(&mut self, program_id: Pubkey) -> Result<()> {
        require!(
            !Self::is_forbidden_venue(&program_id),
            ErrorCode::VenueProgramForbidden
        );
        require!(!self.is_allowed(&program_id), ErrorCode::VenueAlreadyPresent);
        let idx = self.venue_count as usize;
        require!(idx < MAX_VENUES, ErrorCode::VenueListFull);
        self.venues[idx] = program_id;
        self.venue_count += 1;
        Ok(())
    }

    /// Ids that are never venues: the token/system/ATA programs and this program.
    pub fn is_forbidden_venue(program_id: &Pubkey) -> bool {
        use crate::shared::{spl_token, system_program, token_2022};
        *program_id == crate::ID
            || *program_id == system_program::ID
            || *program_id == spl_token::ID
            || *program_id == token_2022::ID
            || *program_id == crate::constants::ATA_PROGRAM
    }

    /// Drop a venue, swapping the last populated slot into its place.
    pub fn remove_venue(&mut self, program_id: &Pubkey) -> Result<()> {
        let count = self.venue_count as usize;
        let pos = self.venues[..count]
            .iter()
            .position(|v| v == program_id)
            .ok_or(ErrorCode::VenueNotFound)?;
        self.venues[pos] = self.venues[count - 1];
        self.venues[count - 1] = Pubkey::default();
        self.venue_count -= 1;
        Ok(())
    }

    /// Clamped accessor for the dormant config fee.
    #[inline]
    pub fn effective_fee_bps(&self) -> u64 {
        self.fee_bps.min(MAX_PLATFORM_FEE_BPS) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> RouterConfig {
        RouterConfig {
            admin: Pubkey::default(),
            fee_authority: Pubkey::default(),
            venues: [Pubkey::default(); MAX_VENUES],
            fee_bps: 0,
            venue_count: 0,
            paused: 0,
            bump: 255,
        }
    }

    /// Add, reject a duplicate, then swap-remove a middle entry.
    #[test]
    fn add_remove_allowlist() {
        let mut c = empty();
        let a = Pubkey::new_unique();
        let b = Pubkey::new_unique();
        let d = Pubkey::new_unique();

        c.add_venue(a).unwrap();
        c.add_venue(b).unwrap();
        c.add_venue(d).unwrap();
        assert_eq!(c.venue_count, 3);
        assert!(c.is_allowed(&a) && c.is_allowed(&b) && c.is_allowed(&d));
        assert!(!c.is_allowed(&Pubkey::new_unique()));

        assert!(c.add_venue(a).is_err());

        c.remove_venue(&b).unwrap();
        assert_eq!(c.venue_count, 2);
        assert!(!c.is_allowed(&b));
        assert!(c.is_allowed(&a) && c.is_allowed(&d));

        assert!(c.remove_venue(&b).is_err());
    }

    /// The list stops accepting entries at `MAX_VENUES`.
    #[test]
    fn allowlist_full() {
        let mut c = empty();
        for _ in 0..MAX_VENUES {
            c.add_venue(Pubkey::new_unique()).unwrap();
        }
        assert!(c.add_venue(Pubkey::new_unique()).is_err());
    }

    /// The denylisted ids are refused and leave the allowlist untouched.
    #[test]
    fn forbidden_venues_rejected() {
        use crate::shared::{spl_token, system_program, token_2022};
        let mut c = empty();
        assert!(c.add_venue(crate::ID).is_err());
        assert!(c.add_venue(system_program::ID).is_err());
        assert!(c.add_venue(spl_token::ID).is_err());
        assert!(c.add_venue(token_2022::ID).is_err());
        assert!(c.add_venue(crate::constants::ATA_PROGRAM).is_err());
        assert_eq!(c.venue_count, 0);
        let v = Pubkey::new_unique();
        c.add_venue(v).unwrap();
        assert!(c.is_allowed(&v));
    }

    /// Removing the tail slot leaves the survivors intact and clears the slot.
    #[test]
    fn remove_last_populated_slot() {
        let mut c = empty();
        let a = Pubkey::new_unique();
        let b = Pubkey::new_unique();
        let d = Pubkey::new_unique();
        c.add_venue(a).unwrap();
        c.add_venue(b).unwrap();
        c.add_venue(d).unwrap();

        c.remove_venue(&d).unwrap();
        assert_eq!(c.venue_count, 2);
        assert!(!c.is_allowed(&d));
        assert!(c.is_allowed(&a) && c.is_allowed(&b));
        assert_eq!(c.venues[2], Pubkey::default());
        assert!(!c.venues[..c.venue_count as usize].contains(&d));
    }

    /// The swapped-in tail value appears exactly once after a non-tail remove.
    #[test]
    fn swap_remove_leaves_no_duplicate() {
        let mut c = empty();
        let a = Pubkey::new_unique();
        let b = Pubkey::new_unique();
        let d = Pubkey::new_unique();
        c.add_venue(a).unwrap();
        c.add_venue(b).unwrap();
        c.add_venue(d).unwrap();

        c.remove_venue(&a).unwrap();
        assert_eq!(c.venue_count, 2);
        let live = &c.venues[..c.venue_count as usize];
        assert_eq!(live.iter().filter(|&&v| v == d).count(), 1);
        assert!(c.is_allowed(&b) && c.is_allowed(&d) && !c.is_allowed(&a));
        assert!(c.add_venue(d).is_err());
    }

    /// Draining to empty clears slot 0, does not underflow, and re-add reuses it.
    #[test]
    fn drain_to_empty_then_readd() {
        let mut c = empty();
        let a = Pubkey::new_unique();
        c.add_venue(a).unwrap();
        assert_eq!(c.venue_count, 1);

        c.remove_venue(&a).unwrap();
        assert_eq!(c.venue_count, 0);
        assert!(!c.is_allowed(&a));
        assert_eq!(c.venues[0], Pubkey::default());
        assert!(c.remove_venue(&a).is_err());
        assert_eq!(c.venue_count, 0);

        let b = Pubkey::new_unique();
        c.add_venue(b).unwrap();
        assert_eq!(c.venue_count, 1);
        assert_eq!(c.venues[0], b);
        assert!(c.is_allowed(&b));
    }

    /// `is_allowed` consults only the live `[..venue_count]` window.
    #[test]
    fn is_allowed_ignores_dead_slots() {
        let mut c = empty();
        assert!(!c.is_allowed(&Pubkey::default()));

        let a = Pubkey::new_unique();
        c.add_venue(a).unwrap();
        c.remove_venue(&a).unwrap();
        assert!(!c.is_allowed(&Pubkey::default()));
        assert!(!c.is_allowed(&a));
    }

    /// The config fee accessor clamps at the maximum.
    #[test]
    fn fee_is_capped() {
        let mut c = empty();
        c.fee_bps = 10_000;
        assert_eq!(c.effective_fee_bps(), MAX_PLATFORM_FEE_BPS as u64);
        c.fee_bps = 25;
        assert_eq!(c.effective_fee_bps(), 25);
    }

    /// Declared size equals discriminator + the Pod struct.
    #[test]
    fn size_matches_layout() {
        assert_eq!(RouterConfig::SIZE, 8 + std::mem::size_of::<RouterConfig>());
    }
}
