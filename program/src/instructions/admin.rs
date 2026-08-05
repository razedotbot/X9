use anchor_lang::prelude::*;

use crate::{
    constants::{MAX_PLATFORM_FEE_BPS, MAX_VENUES, SEED_CONFIG},
    error::ErrorCode,
    state::RouterConfig,
};

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = RouterConfig::SIZE,
        seeds = [SEED_CONFIG],
        bump,
    )]
    pub config: AccountLoader<'info, RouterConfig>,

    pub system_program: Program<'info, System>,
}

/// Initialize the singleton config; the signer becomes admin.
pub fn initialize_config_handler(
    ctx: Context<InitializeConfig>,
    fee_authority: Pubkey,
    fee_bps: u16,
) -> Result<()> {
    require!(fee_bps <= MAX_PLATFORM_FEE_BPS, ErrorCode::FeeTooHigh);
    let config = &mut ctx.accounts.config.load_init()?;
    config.admin = ctx.accounts.admin.key();
    config.fee_authority = fee_authority;
    config.venues = [Pubkey::default(); MAX_VENUES];
    config.fee_bps = fee_bps;
    config.venue_count = 0;
    config.paused = 0;
    config.bump = ctx.bumps.config;
    Ok(())
}

/// Shared accounts for every admin mutation.
#[derive(Accounts)]
pub struct AdminOnly<'info> {
    pub admin: Signer<'info>,

    #[account(mut, address = crate::constants::CONFIG_ADDRESS)]
    pub config: AccountLoader<'info, RouterConfig>,
}

impl<'info> AdminOnly<'info> {
    /// Load mutably, requiring the signer to be the stored admin.
    fn load_admin(&self) -> Result<std::cell::RefMut<'_, RouterConfig>> {
        let config = self.config.load_mut()?;
        require_keys_eq!(config.admin, self.admin.key(), ErrorCode::Unauthorized);
        Ok(config)
    }
}

/// Append a venue program id to the allowlist.
pub fn add_venue_handler(ctx: Context<AdminOnly>, program_id: Pubkey) -> Result<()> {
    ctx.accounts.load_admin()?.add_venue(program_id)
}

/// Drop a venue program id from the allowlist.
pub fn remove_venue_handler(ctx: Context<AdminOnly>, program_id: Pubkey) -> Result<()> {
    ctx.accounts.load_admin()?.remove_venue(&program_id)
}

/// Set the platform fee in bps.
pub fn set_fee_handler(ctx: Context<AdminOnly>, fee_bps: u16) -> Result<()> {
    require!(fee_bps <= MAX_PLATFORM_FEE_BPS, ErrorCode::FeeTooHigh);
    ctx.accounts.load_admin()?.fee_bps = fee_bps;
    Ok(())
}

/// Toggle the paused flag.
pub fn set_paused_handler(ctx: Context<AdminOnly>, paused: bool) -> Result<()> {
    ctx.accounts.load_admin()?.paused = paused as u8;
    Ok(())
}

/// Hand admin over to another address; the zero address is rejected.
pub fn set_admin_handler(ctx: Context<AdminOnly>, new_admin: Pubkey) -> Result<()> {
    require!(
        new_admin != Pubkey::default(),
        ErrorCode::InvalidAdminAddress
    );
    ctx.accounts.load_admin()?.admin = new_admin;
    Ok(())
}

/// Set the wallet that receives platform fees.
pub fn set_fee_authority_handler(ctx: Context<AdminOnly>, new_fee_authority: Pubkey) -> Result<()> {
    ctx.accounts.load_admin()?.fee_authority = new_fee_authority;
    Ok(())
}
