//! The swap instructions: exact-in, exact-out and the unified envelope,
//! sharing one accounts struct and one hop trust boundary.
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};
use anchor_lang::system_program::{self, Transfer};
use anchor_spl::token::{self, CloseAccount, SyncNative, Token};
use anchor_spl::token_interface::TokenInterface;
use crate::shared::{
    spl_token, system_program as system_program_id, wsol, HopMeta, RouteArgsExactOut,
    RouteArgsUnified, RouteArgsV2, RouteMode, LEG_NATIVE, LEG_TOKEN,
};

use crate::{
    constants::{ATA_PROGRAM, MAX_HOPS, MAX_PLATFORM_FEE_BPS, SEED_SA, ZERO_ADDRESS},
    error::ErrorCode,
    fee::{
        charge_boundary_fee, compute_fee, fund_sa_native_buffer, sweep_sa_native_residual,
        BoundaryFeeSource,
    },
    hop::{execute_hop, HopBounds, LegKind},
    state::RouterConfig,
    utils::{mint_decimals, token_mint, token_owner, transfer_checked},
};

/// Accounts shared by every swap instruction. The boundary user accounts,
/// mints and token programs are present only when that side is a token leg.
#[derive(Accounts)]
pub struct RouteUniversal<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(address = crate::constants::CONFIG_ADDRESS)]
    pub config: AccountLoader<'info, RouterConfig>,

    /// CHECK: validated by `resolve_sa`; signer / native-lamport leg only.
    #[account(mut)]
    pub sa_authority: UncheckedAccount<'info>,

    /// CHECK: not deserialized — the handler validates it via the raw token
    #[account(mut)]
    pub user_source_token: Option<UncheckedAccount<'info>>,

    /// CHECK: not deserialized — validated by a raw `token_mint` pin plus the
    #[account(mut)]
    pub user_destination_token: Option<UncheckedAccount<'info>>,

    /// CHECK: not deserialized — only its key (pinned by `require_keys_eq`
    pub source_mint: Option<UncheckedAccount<'info>>,

    /// CHECK: key-pinned + raw decimals only, never deserialized.
    pub destination_mint: Option<UncheckedAccount<'info>>,

    pub source_token_program: Option<Interface<'info, TokenInterface>>,

    pub destination_token_program: Option<Interface<'info, TokenInterface>>,

    /// CHECK: address pinned to the canonical wSOL mint.
    #[account(address = wsol::ID)]
    pub wsol_mint: UncheckedAccount<'info>,

    pub wsol_token_program: Program<'info, Token>,

    /// CHECK: address pinned to the canonical ATA program.
    #[account(address = ATA_PROGRAM)]
    pub associated_token_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,

    /// CHECK: validated by the `charge_*` leaf `charge_boundary_fee` picks —
    #[account(mut)]
    pub platform_fee_account: UncheckedAccount<'info>,
}

/// Exact-in swap: deposit, run the hops, take the fee, check `min_return`,
/// withdraw and sweep the SA back to zero.
pub fn route_universal_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, RouteUniversal<'info>>,
    mut args: RouteArgsV2,
) -> Result<()> {
    let config = ctx.accounts.config.load()?;
    require!(!config.is_paused(), ErrorCode::Paused);
    require!(args.amount_in > 0, ErrorCode::AmountInMustBeGreaterThanZero);
    require!(args.min_return > 0, ErrorCode::MinReturnMustBeGreaterThanZero);
    require!(!args.hops.is_empty(), ErrorCode::NoHops);
    require!(args.hops.len() <= MAX_HOPS, ErrorCode::TooManyHops);
    require!(args.fee_bps <= MAX_PLATFORM_FEE_BPS, ErrorCode::FeeTooHigh);
    for h in args.hops.iter() {
        require!(
            h.source_kind <= LEG_NATIVE && h.dest_kind <= LEG_NATIVE,
            ErrorCode::InvalidLegKind
        );
    }

    let src_boundary = args.hops[0].source_kind;
    let dst_boundary = args.hops[args.hops.len() - 1].dest_kind;

    let sa_authority = ctx.accounts.sa_authority.key();
    let payer_key = ctx.accounts.payer.key();
    let (sa_bump, sa_index) = crate::resolve_sa(&sa_authority, &payer_key)?;
    let sa_index_arr = [sa_index];
    let sa_bump_arr = [sa_bump];
    let sa_seeds: &[&[&[u8]]] = &[&[SEED_SA, &sa_index_arr, &sa_bump_arr]];

    let remaining = ctx.remaining_accounts;

    let route_has_native = args
        .hops
        .iter()
        .any(|h| h.source_kind == LEG_NATIVE || h.dest_kind == LEG_NATIVE);
    let (sa_wsol_ata, payer_wsol_ata) = if route_has_native {
        (
            crate::constants::SA_WSOL_ATA[sa_index as usize],
            Pubkey::find_program_address(
                &[payer_key.as_ref(), spl_token::ID.as_ref(), wsol::ID.as_ref()],
                &ATA_PROGRAM,
            )
            .0,
        )
    } else {
        (crate::constants::ZERO_ADDRESS, crate::constants::ZERO_ADDRESS)
    };
    let native_hop_extra = [sa_wsol_ata, payer_wsol_ata];
    let native_hop_extra_sa = [sa_wsol_ata];

    let first_source_idx = args.hops[0].source_token_index as usize;
    let last_dest_idx = args.hops[args.hops.len() - 1].destination_token_index as usize;
    let sa_source = remaining
        .get(first_source_idx)
        .ok_or(ErrorCode::InvalidAccountsLength)?
        .to_account_info();
    let sa_destination = remaining
        .get(last_dest_idx)
        .ok_or(ErrorCode::InvalidAccountsLength)?
        .to_account_info();

    let lean_dest: Option<Pubkey> = if dst_boundary == LEG_TOKEN {
        ctx.accounts
            .user_destination_token
            .as_ref()
            .map(|u| u.key())
            .filter(|k| *k == sa_destination.key())
    } else {
        None
    };

    let lean_dest_native = dst_boundary == LEG_NATIVE && sa_destination.key() == payer_key;

    let lean_source: Option<Pubkey> = match src_boundary {
        LEG_TOKEN => ctx
            .accounts
            .user_source_token
            .as_ref()
            .map(|u| u.key())
            .filter(|k| *k == sa_source.key()),
        LEG_NATIVE => (sa_source.key() == payer_key).then_some(payer_key),
        _ => None,
    };
    let lean_source_native = lean_source.is_some() && src_boundary == LEG_NATIVE;

    let any_lean = lean_source.is_some() || lean_dest.is_some() || lean_dest_native;

    let fund_topup = if lean_source.is_some() { 0 } else { args.sa_native_topup };
    fund_sa_native_buffer(
        ctx.accounts.system_program.to_account_info(),
        ctx.accounts.payer.to_account_info(),
        ctx.accounts.sa_authority.to_account_info(),
        fund_topup,
    )?;

    if src_boundary == LEG_TOKEN {
        let user_src = ctx
            .accounts
            .user_source_token
            .as_ref()
            .ok_or(ErrorCode::MissingBoundaryAccount)?;
        let src_mint = ctx
            .accounts
            .source_mint
            .as_ref()
            .ok_or(ErrorCode::MissingBoundaryAccount)?;
        let src_tp = ctx
            .accounts
            .source_token_program
            .as_ref()
            .ok_or(ErrorCode::MissingBoundaryAccount)?;
        require_keys_eq!(token_owner(&user_src.to_account_info())?, ctx.accounts.payer.key(), ErrorCode::InvalidSourceTokenAccount);
        require_keys_eq!(token_mint(&user_src.to_account_info())?, src_mint.key(), ErrorCode::BoundaryMintMismatch);
        require_keys_eq!(token_mint(&sa_source)?, src_mint.key(), ErrorCode::BoundaryMintMismatch);
        if lean_source.is_none() {
            transfer_checked(
                src_tp.to_account_info(),
                user_src.to_account_info(),
                src_mint.to_account_info(),
                sa_source.clone(),
                ctx.accounts.payer.to_account_info(),
                args.amount_in,
                mint_decimals(&src_mint.to_account_info())?,
                None,
            )?;
        }
    } else if lean_source.is_none() {
        require_keys_eq!(sa_source.key(), sa_authority, ErrorCode::InvalidSourceTokenAccount);
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.payer.to_account_info(),
                    to: ctx.accounts.sa_authority.to_account_info(),
                },
            ),
            args.amount_in,
        )?;
    } else {
        require_keys_eq!(sa_source.key(), payer_key, ErrorCode::InvalidSourceTokenAccount);
    }

    let mut running = args.amount_in;
    let mut fee = 0u64;
    if args.fee_bps > 0 && args.fee_on_input {
        fee = compute_fee(args.amount_in, args.fee_bps as u64)?;
        let source = if src_boundary == LEG_TOKEN {
            let src_mint = ctx.accounts.source_mint.as_ref().ok_or(ErrorCode::MissingBoundaryAccount)?;
            let src_tp = ctx.accounts.source_token_program.as_ref().ok_or(ErrorCode::MissingBoundaryAccount)?;
            BoundaryFeeSource::Token {
                lean: lean_source.is_some(),
                token_program: src_tp.to_account_info(),
                token_account: sa_source.clone(),
                mint: src_mint.to_account_info(),
                decimals: mint_decimals(&src_mint.to_account_info())?,
            }
        } else {
            BoundaryFeeSource::Native {
                lean: lean_source_native,
            }
        };
        charge_boundary_fee(
            fee,
            source,
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.platform_fee_account.to_account_info(),
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.sa_authority.to_account_info(),
            sa_seeds,
        )?;
        running = running.checked_sub(fee).ok_or(ErrorCode::CalculationError)?;
        require!(running > 0, ErrorCode::AmountInMustBeGreaterThanZero);
    }

    let mut prev_to = ZERO_ADDRESS;
    for i in 0..args.hops.len() {
        if i > 0 {
            let prev_dest_kind = args.hops[i - 1].dest_kind;
            let next_source_kind = args.hops[i].source_kind;
            if prev_dest_kind == LEG_TOKEN && next_source_kind == LEG_NATIVE {
                let prev_dest_ai = remaining
                    .get(args.hops[i - 1].destination_token_index as usize)
                    .ok_or(ErrorCode::InvalidAccountsLength)?;
                if prev_dest_ai.key() == payer_wsol_ata {
                    token::close_account(CpiContext::new(
                        ctx.accounts.wsol_token_program.to_account_info(),
                        CloseAccount {
                            account: prev_dest_ai.to_account_info(),
                            destination: ctx.accounts.payer.to_account_info(),
                            authority: ctx.accounts.payer.to_account_info(),
                        },
                    ))?;
                    let create_ix = Instruction {
                        program_id: ATA_PROGRAM,
                        accounts: vec![
                            AccountMeta::new(payer_key, true),
                            AccountMeta::new(payer_wsol_ata, false),
                            AccountMeta::new_readonly(payer_key, false),
                            AccountMeta::new_readonly(wsol::ID, false),
                            AccountMeta::new_readonly(system_program_id::ID, false),
                            AccountMeta::new_readonly(spl_token::ID, false),
                        ],
                        data: vec![1],
                    };
                    anchor_lang::solana_program::program::invoke(
                        &create_ix,
                        &[
                            ctx.accounts.payer.to_account_info(),
                            prev_dest_ai.to_account_info(),
                            ctx.accounts.payer.to_account_info(),
                            ctx.accounts.wsol_mint.to_account_info(),
                            ctx.accounts.system_program.to_account_info(),
                            ctx.accounts.wsol_token_program.to_account_info(),
                            ctx.accounts.associated_token_program.to_account_info(),
                        ],
                    )?;
                    prev_to = payer_key;
                } else {
                    require_keys_eq!(prev_dest_ai.key(), sa_wsol_ata, ErrorCode::InvalidWsolTransition);
                    token::close_account(CpiContext::new_with_signer(
                        ctx.accounts.wsol_token_program.to_account_info(),
                        CloseAccount {
                            account: prev_dest_ai.to_account_info(),
                            destination: ctx.accounts.sa_authority.to_account_info(),
                            authority: ctx.accounts.sa_authority.to_account_info(),
                        },
                        sa_seeds,
                    ))?;
                    let create_ix = Instruction {
                        program_id: ATA_PROGRAM,
                        accounts: vec![
                            AccountMeta::new(sa_authority, true),
                            AccountMeta::new(sa_wsol_ata, false),
                            AccountMeta::new_readonly(sa_authority, false),
                            AccountMeta::new_readonly(wsol::ID, false),
                            AccountMeta::new_readonly(system_program_id::ID, false),
                            AccountMeta::new_readonly(spl_token::ID, false),
                        ],
                        data: vec![1],
                    };
                    invoke_signed(
                        &create_ix,
                        &[
                            ctx.accounts.sa_authority.to_account_info(),
                            prev_dest_ai.to_account_info(),
                            ctx.accounts.sa_authority.to_account_info(),
                            ctx.accounts.wsol_mint.to_account_info(),
                            ctx.accounts.system_program.to_account_info(),
                            ctx.accounts.wsol_token_program.to_account_info(),
                            ctx.accounts.associated_token_program.to_account_info(),
                        ],
                        sa_seeds,
                    )?;
                    prev_to = sa_authority;
                }
            } else if prev_dest_kind == LEG_NATIVE && next_source_kind == LEG_TOKEN {
                let next_source_ai = remaining
                    .get(args.hops[i].source_token_index as usize)
                    .ok_or(ErrorCode::InvalidAccountsLength)?;
                require_keys_eq!(token_mint(next_source_ai)?, wsol::ID, ErrorCode::InvalidWsolTransition);
                if prev_to == payer_key {
                    require_keys_eq!(token_owner(next_source_ai)?, payer_key, ErrorCode::InvalidWsolTransition);
                    system_program::transfer(
                        CpiContext::new(
                            ctx.accounts.system_program.to_account_info(),
                            Transfer {
                                from: ctx.accounts.payer.to_account_info(),
                                to: next_source_ai.to_account_info(),
                            },
                        ),
                        running,
                    )?;
                } else {
                    require_keys_eq!(prev_to, sa_authority, ErrorCode::InvalidWsolTransition);
                    require_keys_eq!(token_owner(next_source_ai)?, sa_authority, ErrorCode::InvalidWsolTransition);
                    system_program::transfer(
                        CpiContext::new_with_signer(
                            ctx.accounts.system_program.to_account_info(),
                            Transfer {
                                from: ctx.accounts.sa_authority.to_account_info(),
                                to: next_source_ai.to_account_info(),
                            },
                            sa_seeds,
                        ),
                        running,
                    )?;
                }
                token::sync_native(CpiContext::new(
                    ctx.accounts.wsol_token_program.to_account_info(),
                    SyncNative {
                        account: next_source_ai.to_account_info(),
                    },
                ))?;
                prev_to = next_source_ai.key();
            }
        }

        let hv2 = &mut args.hops[i];
        let source_kind = leg_kind(hv2.source_kind)?;
        let dest_kind = leg_kind(hv2.dest_kind)?;
        let splice_xor_mask = hv2.splice_xor_mask;
        let mut hop = HopMeta {
            program_id_index: hv2.program_id_index,
            source_token_index: hv2.source_token_index,
            destination_token_index: hv2.destination_token_index,
            amount_in_offset: hv2.amount_in_offset,
            accounts: std::mem::take(&mut hv2.accounts),
            data: std::mem::take(&mut hv2.data),
        };
        let extra_allowed: &[Pubkey] =
            if source_kind == LegKind::Native || dest_kind == LegKind::Native {
                if any_lean {
                    &native_hop_extra
                } else {
                    &native_hop_extra_sa
                }
            } else {
                &[]
            };
        let allowed_user_dest = if i == args.hops.len() - 1 { lean_dest } else { None };
        let lean_payer = any_lean.then_some(payer_key);
        let (_actual_in, out, to) = execute_hop(
            &mut hop,
            remaining,
            HopBounds::ExactIn { amount_in: running },
            &sa_authority,
            &config,
            i,
            prev_to,
            sa_seeds,
            source_kind,
            dest_kind,
            extra_allowed,
            &splice_xor_mask,
            args.sa_native_topup,
            allowed_user_dest,
            lean_payer,
        )?;
        running = out;
        prev_to = to;
    }
    let total_out = running;

    let net = if args.fee_bps > 0 && !args.fee_on_input {
        fee = compute_fee(total_out, args.fee_bps as u64)?;
        let net = total_out.checked_sub(fee).ok_or(ErrorCode::CalculationError)?;
        require!(net >= args.min_return, ErrorCode::MinReturnNotReached);
        let source = if dst_boundary == LEG_TOKEN {
            let dst_mint = ctx.accounts.destination_mint.as_ref().ok_or(ErrorCode::MissingBoundaryAccount)?;
            let dst_tp = ctx.accounts.destination_token_program.as_ref().ok_or(ErrorCode::MissingBoundaryAccount)?;
            let token_account = if lean_dest.is_some() {
                ctx.accounts
                    .user_destination_token
                    .as_ref()
                    .ok_or(ErrorCode::MissingBoundaryAccount)?
                    .to_account_info()
            } else {
                sa_destination.clone()
            };
            BoundaryFeeSource::Token {
                lean: lean_dest.is_some(),
                token_program: dst_tp.to_account_info(),
                token_account,
                mint: dst_mint.to_account_info(),
                decimals: mint_decimals(&dst_mint.to_account_info())?,
            }
        } else {
            BoundaryFeeSource::Native {
                lean: lean_dest_native,
            }
        };
        charge_boundary_fee(
            fee,
            source,
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.platform_fee_account.to_account_info(),
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.sa_authority.to_account_info(),
            sa_seeds,
        )?;
        net
    } else {
        require!(total_out >= args.min_return, ErrorCode::MinReturnNotReached);
        total_out
    };

    if dst_boundary == LEG_TOKEN {
        let user_dst = ctx
            .accounts
            .user_destination_token
            .as_ref()
            .ok_or(ErrorCode::MissingBoundaryAccount)?;
        let dst_mint = ctx
            .accounts
            .destination_mint
            .as_ref()
            .ok_or(ErrorCode::MissingBoundaryAccount)?;
        let dst_tp = ctx
            .accounts
            .destination_token_program
            .as_ref()
            .ok_or(ErrorCode::MissingBoundaryAccount)?;
        require_keys_eq!(token_mint(&user_dst.to_account_info())?, dst_mint.key(), ErrorCode::BoundaryMintMismatch);
        require_keys_eq!(token_mint(&sa_destination)?, dst_mint.key(), ErrorCode::BoundaryMintMismatch);
        if lean_dest.is_none() {
            transfer_checked(
                dst_tp.to_account_info(),
                sa_destination,
                dst_mint.to_account_info(),
                user_dst.to_account_info(),
                ctx.accounts.sa_authority.to_account_info(),
                net,
                mint_decimals(&dst_mint.to_account_info())?,
                Some(sa_seeds),
            )?;
        }
    } else if lean_dest_native {
    } else {
        require_keys_eq!(sa_destination.key(), sa_authority, ErrorCode::InvalidDestinationTokenAccount);
        system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.sa_authority.to_account_info(),
                    to: ctx.accounts.payer.to_account_info(),
                },
                sa_seeds,
            ),
            net,
        )?;
    }

    sweep_sa_native_residual(
        ctx.accounts.system_program.to_account_info(),
        ctx.accounts.sa_authority.to_account_info(),
        ctx.accounts.payer.to_account_info(),
        sa_seeds,
    )?;

    emit!(RouteExecuted {
        payer: ctx.accounts.payer.key(),
        source_mint: if src_boundary == LEG_TOKEN {
            ctx.accounts
                .source_mint
                .as_ref()
                .map(|m| m.key())
                .unwrap_or(wsol::ID)
        } else {
            wsol::ID
        },
        destination_mint: if dst_boundary == LEG_TOKEN {
            ctx.accounts
                .destination_mint
                .as_ref()
                .map(|m| m.key())
                .unwrap_or(wsol::ID)
        } else {
            wsol::ID
        },
        amount_in: args.amount_in,
        amount_out: net,
        fee,
        hops: args.hops.len() as u8,
    });

    Ok(())
}

/// Map a wire leg-kind byte onto [`LegKind`].
#[inline]
fn leg_kind(v: u8) -> Result<LegKind> {
    match v {
        LEG_TOKEN => Ok(LegKind::Token),
        LEG_NATIVE => Ok(LegKind::Native),
        _ => Err(ErrorCode::InvalidLegKind.into()),
    }
}

/// Emitted once per successful route.
#[event]
pub struct RouteExecuted {
    pub payer: Pubkey,
    pub source_mint: Pubkey,
    pub destination_mint: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
    pub fee: u64,
    pub hops: u8,
}

/// Resolve the unified envelope's `mode` and delegate to the matching handler.
pub fn route_unified_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, RouteUniversal<'info>>,
    args: RouteArgsUnified,
) -> Result<()> {
    match args.split().ok_or(ErrorCode::InvalidRouteMode)? {
        RouteMode::ExactIn(v2) => route_universal_handler(ctx, v2),
        RouteMode::ExactOut(eo) => route_exact_out_handler(ctx, eo),
    }
}

/// Exact-out swap: deposit `max_amount_in`, run the hop, refund the unspent
/// input, take the fee on the remainder and deliver `amount_out`.
pub fn route_exact_out_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, RouteUniversal<'info>>,
    mut args: RouteArgsExactOut,
) -> Result<()> {
    let config = ctx.accounts.config.load()?;
    require!(!config.is_paused(), ErrorCode::Paused);
    require!(args.amount_out > 0, ErrorCode::AmountOutMustBeGreaterThanZero);
    require!(
        args.max_amount_in > 0,
        ErrorCode::AmountInMustBeGreaterThanZero
    );
    require!(args.hops.len() == 1, ErrorCode::NativeRouteMustBeSingleHop);
    require!(args.fee_bps <= MAX_PLATFORM_FEE_BPS, ErrorCode::FeeTooHigh);
    let h = &args.hops[0];
    require!(
        h.source_kind <= LEG_NATIVE && h.dest_kind <= LEG_NATIVE,
        ErrorCode::InvalidLegKind
    );

    let src_native = h.source_kind == LEG_NATIVE;
    let dst_native = h.dest_kind == LEG_NATIVE;

    let sa_authority = ctx.accounts.sa_authority.key();
    let payer_key = ctx.accounts.payer.key();
    let (sa_bump, sa_index) = crate::resolve_sa(&sa_authority, &payer_key)?;
    let sa_index_arr = [sa_index];
    let sa_bump_arr = [sa_bump];
    let sa_seeds: &[&[&[u8]]] = &[&[SEED_SA, &sa_index_arr, &sa_bump_arr]];
    let remaining = ctx.remaining_accounts;

    let has_native = src_native || dst_native;
    let sa_wsol_ata = if has_native {
        crate::constants::SA_WSOL_ATA[sa_index as usize]
    } else {
        crate::constants::ZERO_ADDRESS
    };

    let sa_source = remaining
        .get(h.source_token_index as usize)
        .ok_or(ErrorCode::InvalidAccountsLength)?
        .to_account_info();
    let sa_destination = remaining
        .get(h.destination_token_index as usize)
        .ok_or(ErrorCode::InvalidAccountsLength)?
        .to_account_info();

    let lean_source = if src_native {
        sa_source.key() == payer_key
    } else {
        ctx.accounts
            .user_source_token
            .as_ref()
            .is_some_and(|u| u.key() == sa_source.key())
    };
    let lean_dest = if dst_native {
        sa_destination.key() == payer_key
    } else {
        ctx.accounts
            .user_destination_token
            .as_ref()
            .is_some_and(|u| u.key() == sa_destination.key())
    };
    let lean = lean_source || lean_dest;
    let payer_wsol_ata = if has_native {
        Pubkey::find_program_address(
            &[payer_key.as_ref(), spl_token::ID.as_ref(), wsol::ID.as_ref()],
            &ATA_PROGRAM,
        )
        .0
    } else {
        crate::constants::ZERO_ADDRESS
    };

    let sa_pre_lamports = ctx.accounts.sa_authority.to_account_info().lamports();
    let native_deposit = args
        .max_amount_in
        .checked_add(args.sa_native_topup)
        .ok_or(ErrorCode::CalculationError)?;

    if lean_source {
        if !src_native {
            let user_src = ctx
                .accounts
                .user_source_token
                .as_ref()
                .ok_or(ErrorCode::MissingBoundaryAccount)?;
            let src_mint = ctx
                .accounts
                .source_mint
                .as_ref()
                .ok_or(ErrorCode::MissingBoundaryAccount)?;
            require_keys_eq!(
                token_owner(&user_src.to_account_info())?,
                ctx.accounts.payer.key(),
                ErrorCode::InvalidSourceTokenAccount
            );
            require_keys_eq!(token_mint(&user_src.to_account_info())?, src_mint.key(), ErrorCode::BoundaryMintMismatch);
        }
    } else if src_native {
        require_keys_eq!(
            sa_source.key(),
            sa_authority,
            ErrorCode::InvalidSourceTokenAccount
        );
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.payer.to_account_info(),
                    to: ctx.accounts.sa_authority.to_account_info(),
                },
            ),
            native_deposit,
        )?;
    } else {
        let user_src = ctx
            .accounts
            .user_source_token
            .as_ref()
            .ok_or(ErrorCode::MissingBoundaryAccount)?;
        let src_mint = ctx
            .accounts
            .source_mint
            .as_ref()
            .ok_or(ErrorCode::MissingBoundaryAccount)?;
        let src_tp = ctx
            .accounts
            .source_token_program
            .as_ref()
            .ok_or(ErrorCode::MissingBoundaryAccount)?;
        require_keys_eq!(
            token_owner(&user_src.to_account_info())?,
            ctx.accounts.payer.key(),
            ErrorCode::InvalidSourceTokenAccount
        );
        require_keys_eq!(token_mint(&user_src.to_account_info())?, src_mint.key(), ErrorCode::BoundaryMintMismatch);
        require_keys_eq!(
            token_mint(&sa_source)?,
            src_mint.key(),
            ErrorCode::BoundaryMintMismatch
        );
        transfer_checked(
            src_tp.to_account_info(),
            user_src.to_account_info(),
            src_mint.to_account_info(),
            sa_source.clone(),
            ctx.accounts.payer.to_account_info(),
            args.max_amount_in,
            mint_decimals(&src_mint.to_account_info())?,
            None,
        )?;
    }

    let source_kind = if src_native {
        LegKind::Native
    } else {
        LegKind::Token
    };
    let dest_kind = if dst_native {
        LegKind::Native
    } else {
        LegKind::Token
    };
    let hv2 = &mut args.hops[0];
    let mut hop = HopMeta {
        program_id_index: hv2.program_id_index,
        source_token_index: hv2.source_token_index,
        destination_token_index: hv2.destination_token_index,
        amount_in_offset: -1,
        accounts: std::mem::take(&mut hv2.accounts),
        data: std::mem::take(&mut hv2.data),
    };
    let native_hop_extra = [sa_wsol_ata, payer_wsol_ata];
    let native_hop_extra_sa = [sa_wsol_ata];
    let extra_allowed: &[Pubkey] =
        if source_kind == LegKind::Native || dest_kind == LegKind::Native {
            if lean {
                &native_hop_extra
            } else {
                &native_hop_extra_sa
            }
        } else {
            &[]
        };
    let allowed_user_dest = if !dst_native {
        ctx.accounts
            .user_destination_token
            .as_ref()
            .map(|u| u.key())
            .filter(|k| *k == sa_destination.key())
    } else {
        None
    };
    const NO_SPLICE_XOR: [u8; 8] = [0; 8];
    let (actual_in, actual_out, _to) = execute_hop(
        &mut hop,
        remaining,
        HopBounds::ExactOut {
            max_in: args.max_amount_in,
            amount_out: args.amount_out,
        },
        &sa_authority,
        &config,
        0,
        ZERO_ADDRESS,
        sa_seeds,
        source_kind,
        dest_kind,
        extra_allowed,
        &NO_SPLICE_XOR,
        args.sa_native_topup,
        allowed_user_dest,
        lean.then_some(payer_key),
    )?;

    let mut remaining_in = if lean_source {
        let upper = if src_native { native_deposit } else { args.max_amount_in };
        upper.saturating_sub(actual_in)
    } else if src_native {
        let above_baseline = ctx
            .accounts
            .sa_authority
            .to_account_info()
            .lamports()
            .saturating_sub(sa_pre_lamports);
        above_baseline.min(native_deposit.saturating_sub(actual_in))
    } else {
        args.max_amount_in.saturating_sub(actual_in)
    };

    let mut fee = 0u64;
    let mut net_out = actual_out;
    if args.fee_bps > 0 {
        if args.fee_on_input {
            fee = compute_fee(actual_in, args.fee_bps as u64)?;
            require!(remaining_in >= fee, ErrorCode::MaxAmountInExceeded);
            let source = if src_native {
                BoundaryFeeSource::Native { lean: lean_source }
            } else {
                let src_mint = ctx.accounts.source_mint.as_ref().ok_or(ErrorCode::MissingBoundaryAccount)?;
                let src_tp = ctx.accounts.source_token_program.as_ref().ok_or(ErrorCode::MissingBoundaryAccount)?;
                BoundaryFeeSource::Token {
                    lean: lean_source,
                    token_program: src_tp.to_account_info(),
                    token_account: sa_source.clone(),
                    mint: src_mint.to_account_info(),
                    decimals: mint_decimals(&src_mint.to_account_info())?,
                }
            };
            charge_boundary_fee(
                fee,
                source,
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.platform_fee_account.to_account_info(),
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.sa_authority.to_account_info(),
                sa_seeds,
            )?;
            remaining_in = remaining_in.checked_sub(fee).ok_or(ErrorCode::CalculationError)?;
        } else {
            fee = compute_fee(actual_out, args.fee_bps as u64)?;
            net_out = actual_out.checked_sub(fee).ok_or(ErrorCode::CalculationError)?;
            let source = if dst_native {
                BoundaryFeeSource::Native { lean: lean_dest }
            } else {
                let dst_mint = ctx.accounts.destination_mint.as_ref().ok_or(ErrorCode::MissingBoundaryAccount)?;
                let dst_tp = ctx.accounts.destination_token_program.as_ref().ok_or(ErrorCode::MissingBoundaryAccount)?;
                BoundaryFeeSource::Token {
                    lean: lean_dest,
                    token_program: dst_tp.to_account_info(),
                    token_account: sa_destination.clone(),
                    mint: dst_mint.to_account_info(),
                    decimals: mint_decimals(&dst_mint.to_account_info())?,
                }
            };
            charge_boundary_fee(
                fee,
                source,
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.platform_fee_account.to_account_info(),
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.sa_authority.to_account_info(),
                sa_seeds,
            )?;
        }
    }

    if !lean_source && remaining_in > 0 {
        if src_native {
            system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.sa_authority.to_account_info(),
                        to: ctx.accounts.payer.to_account_info(),
                    },
                    sa_seeds,
                ),
                remaining_in,
            )?;
        } else {
            let user_src = ctx.accounts.user_source_token.as_ref().ok_or(ErrorCode::MissingBoundaryAccount)?;
            let src_mint = ctx.accounts.source_mint.as_ref().ok_or(ErrorCode::MissingBoundaryAccount)?;
            let src_tp = ctx.accounts.source_token_program.as_ref().ok_or(ErrorCode::MissingBoundaryAccount)?;
            transfer_checked(
                src_tp.to_account_info(),
                sa_source.clone(),
                src_mint.to_account_info(),
                user_src.to_account_info(),
                ctx.accounts.sa_authority.to_account_info(),
                remaining_in,
                mint_decimals(&src_mint.to_account_info())?,
                Some(sa_seeds),
            )?;
        }
    }

    if lean_dest {
        if !dst_native {
            let user_dst = ctx
                .accounts
                .user_destination_token
                .as_ref()
                .ok_or(ErrorCode::MissingBoundaryAccount)?;
            let dst_mint = ctx
                .accounts
                .destination_mint
                .as_ref()
                .ok_or(ErrorCode::MissingBoundaryAccount)?;
            require_keys_eq!(token_mint(&user_dst.to_account_info())?, dst_mint.key(), ErrorCode::BoundaryMintMismatch);
        }
    } else if dst_native {
        require_keys_eq!(
            sa_destination.key(),
            sa_authority,
            ErrorCode::InvalidDestinationTokenAccount
        );
        system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.sa_authority.to_account_info(),
                    to: ctx.accounts.payer.to_account_info(),
                },
                sa_seeds,
            ),
            net_out,
        )?;
    } else {
        let user_dst = ctx
            .accounts
            .user_destination_token
            .as_ref()
            .ok_or(ErrorCode::MissingBoundaryAccount)?;
        let dst_mint = ctx
            .accounts
            .destination_mint
            .as_ref()
            .ok_or(ErrorCode::MissingBoundaryAccount)?;
        let dst_tp = ctx
            .accounts
            .destination_token_program
            .as_ref()
            .ok_or(ErrorCode::MissingBoundaryAccount)?;
        require_keys_eq!(token_mint(&user_dst.to_account_info())?, dst_mint.key(), ErrorCode::BoundaryMintMismatch);
        require_keys_eq!(
            token_mint(&sa_destination)?,
            dst_mint.key(),
            ErrorCode::BoundaryMintMismatch
        );
        transfer_checked(
            dst_tp.to_account_info(),
            sa_destination,
            dst_mint.to_account_info(),
            user_dst.to_account_info(),
            ctx.accounts.sa_authority.to_account_info(),
            net_out,
            mint_decimals(&dst_mint.to_account_info())?,
            Some(sa_seeds),
        )?;
    }

    emit!(RouteExecuted {
        payer: ctx.accounts.payer.key(),
        source_mint: if src_native {
            wsol::ID
        } else {
            ctx.accounts
                .source_mint
                .as_ref()
                .map(|m| m.key())
                .unwrap_or(wsol::ID)
        },
        destination_mint: if dst_native {
            wsol::ID
        } else {
            ctx.accounts
                .destination_mint
                .as_ref()
                .map(|m| m.key())
                .unwrap_or(wsol::ID)
        },
        amount_in: actual_in,
        amount_out: net_out,
        fee,
        hops: 1,
    });

    Ok(())
}
