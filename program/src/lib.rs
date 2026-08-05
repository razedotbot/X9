#![allow(unexpected_cfgs, clippy::too_many_arguments, clippy::result_large_err)]

use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod fee;
pub mod hop;
pub mod instructions;
pub mod shared;
pub mod state;
pub mod utils;

pub use constants::*;
pub use instructions::*;
pub use shared::{
    HopAccountMeta, HopMeta, HopMetaV2, RouteArgsExactOut, RouteArgsUnified, RouteArgsV2, RouteMode,
    LEG_NATIVE, LEG_TOKEN, ROUTE_MODE_EXACT_IN, ROUTE_MODE_EXACT_OUT,
};

declare_id!("RAZEX9pxDuRCrtwR5wxUPAX3pWwAkBzvM8hF2fKaRE9");

#[program]
pub mod raze_router {
    use super::*;

    /// Exact-in swap: fixed input, output floored at `min_return`.
    pub fn route_universal<'info>(
        ctx: Context<'_, '_, 'info, 'info, RouteUniversal<'info>>,
        args: RouteArgsV2,
    ) -> Result<()> {
        instructions::route_universal_handler(ctx, args)
    }

    /// Exact-out single-hop swap: fixed output, input capped at `max_amount_in`.
    pub fn route_exact_out<'info>(
        ctx: Context<'_, '_, 'info, 'info, RouteUniversal<'info>>,
        args: RouteArgsExactOut,
    ) -> Result<()> {
        instructions::route_exact_out_handler(ctx, args)
    }

    /// Both swap directions behind one discriminator, selected by `args.mode`.
    pub fn route_unified<'info>(
        ctx: Context<'_, '_, 'info, 'info, RouteUniversal<'info>>,
        args: RouteArgsUnified,
    ) -> Result<()> {
        instructions::route_unified_handler(ctx, args)
    }

    /// Create the singleton config. Caller becomes admin.
    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        fee_authority: Pubkey,
        fee_bps: u16,
    ) -> Result<()> {
        instructions::initialize_config_handler(ctx, fee_authority, fee_bps)
    }

    /// Allowlist a venue program id.
    pub fn add_venue(ctx: Context<AdminOnly>, program_id: Pubkey) -> Result<()> {
        instructions::add_venue_handler(ctx, program_id)
    }

    /// Remove a venue program id from the allowlist.
    pub fn remove_venue(ctx: Context<AdminOnly>, program_id: Pubkey) -> Result<()> {
        instructions::remove_venue_handler(ctx, program_id)
    }

    /// Update the platform fee (bps, capped at `MAX_PLATFORM_FEE_BPS`).
    pub fn set_fee(ctx: Context<AdminOnly>, fee_bps: u16) -> Result<()> {
        instructions::set_fee_handler(ctx, fee_bps)
    }

    /// Pause both swap instructions; admin instructions stay reachable.
    pub fn set_paused(ctx: Context<AdminOnly>, paused: bool) -> Result<()> {
        instructions::set_paused_handler(ctx, paused)
    }

    /// Transfer admin.
    pub fn set_admin(ctx: Context<AdminOnly>, new_admin: Pubkey) -> Result<()> {
        instructions::set_admin_handler(ctx, new_admin)
    }

    /// Change the wallet that receives platform fees.
    pub fn set_fee_authority(ctx: Context<AdminOnly>, new_fee_authority: Pubkey) -> Result<()> {
        instructions::set_fee_authority_handler(ctx, new_fee_authority)
    }
}
