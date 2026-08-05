
use anchor_lang::{AccountDeserialize, Discriminator, InstructionData, ToAccountMetas};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

const SEED_CONFIG: &[u8] = b"router-config";

fn program_id() -> Pubkey {
    raze_router::ID
}

fn config_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_CONFIG], &program_id())
}

/// Boot a test context with the program loaded.
async fn start() -> (ProgramTestContext, Pubkey) {
    let deploy_dir = format!("{}/target/deploy", env!("CARGO_MANIFEST_DIR"));
    std::env::set_var("SBF_OUT_DIR", &deploy_dir);

    let pt = ProgramTest::new("raze_router", program_id(), None);
    let ctx = pt.start_with_context().await;
    let (config, _) = config_pda();
    (ctx, config)
}

async fn send(
    ctx: &mut ProgramTestContext,
    ix: Instruction,
    extra_signers: &[&Keypair],
) -> Result<(), String> {
    let payer = ctx.payer.insecure_clone();
    let mut signers: Vec<&Keypair> = vec![&payer];
    signers.extend_from_slice(extra_signers);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &signers,
        ctx.last_blockhash,
    );
    ctx.banks_client
        .process_transaction(tx)
        .await
        .map_err(|e| format!("{e:?}"))
}

/// Read the config account back as a typed struct.
async fn read_config(ctx: &mut ProgramTestContext, config: Pubkey) -> raze_router::state::RouterConfig {
    let acct = ctx
        .banks_client
        .get_account(config)
        .await
        .unwrap()
        .expect("config account");
    assert_eq!(&acct.data[..8], raze_router::state::RouterConfig::DISCRIMINATOR);
    raze_router::state::RouterConfig::try_deserialize(&mut acct.data.as_slice()).unwrap()
}

/// Build the `initialize_config` instruction.
fn init_ix(ctx: &ProgramTestContext, config: Pubkey, fee_authority: Pubkey, fee_bps: u16) -> Instruction {
    Instruction {
        program_id: program_id(),
        accounts: raze_router::accounts::InitializeConfig {
            admin: ctx.payer.pubkey(),
            config,
            system_program: solana_sdk::system_program::ID,
        }
        .to_account_metas(None),
        data: raze_router::instruction::InitializeConfig { fee_authority, fee_bps }.data(),
    }
}

/// Build an admin instruction from pre-encoded data.
fn admin_ix(config: Pubkey, admin: Pubkey, data: Vec<u8>) -> Instruction {
    Instruction {
        program_id: program_id(),
        accounts: raze_router::accounts::AdminOnly { admin, config }.to_account_metas(None),
        data,
    }
}

/// Initialize, then exercise each admin mutation end to end.
#[tokio::test]
async fn init_and_admin_lifecycle() {
    let (mut ctx, config) = start().await;
    let admin = ctx.payer.pubkey();
    let fee_authority = Pubkey::new_unique();

    let ix = init_ix(&ctx, config, fee_authority, 25);
    send(&mut ctx, ix, &[]).await.unwrap();

    let cfg = read_config(&mut ctx, config).await;
    assert_eq!(cfg.admin, admin);
    assert_eq!(cfg.fee_authority, fee_authority);
    assert_eq!(cfg.fee_bps, 25);
    assert_eq!(cfg.venue_count, 0);
    assert_eq!(cfg.paused, 0);

    let pumpswap: Pubkey = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA".parse().unwrap();
    let raydium_cp: Pubkey = "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C".parse().unwrap();

    send(&mut ctx, admin_ix(config, admin, raze_router::instruction::AddVenue { program_id: pumpswap }.data()), &[]).await.unwrap();
    send(&mut ctx, admin_ix(config, admin, raze_router::instruction::AddVenue { program_id: raydium_cp }.data()), &[]).await.unwrap();
    let cfg = read_config(&mut ctx, config).await;
    assert_eq!(cfg.venue_count, 2);
    assert!(cfg.is_allowed(&pumpswap) && cfg.is_allowed(&raydium_cp));

    send(&mut ctx, admin_ix(config, admin, raze_router::instruction::RemoveVenue { program_id: pumpswap }.data()), &[]).await.unwrap();
    let cfg = read_config(&mut ctx, config).await;
    assert_eq!(cfg.venue_count, 1);
    assert!(!cfg.is_allowed(&pumpswap) && cfg.is_allowed(&raydium_cp));

    send(&mut ctx, admin_ix(config, admin, raze_router::instruction::SetPaused { paused: true }.data()), &[]).await.unwrap();
    assert_eq!(read_config(&mut ctx, config).await.paused, 1);
}

/// Both `initialize_config` and `set_fee` refuse a fee over the cap.
#[tokio::test]
async fn fee_cap_enforced() {
    let (mut ctx, config) = start().await;
    let admin = ctx.payer.pubkey();
    let ix = init_ix(&ctx, config, Pubkey::new_unique(), 25);
    send(&mut ctx, ix, &[]).await.unwrap();

    let err = send(&mut ctx, admin_ix(config, admin, raze_router::instruction::SetFee { fee_bps: 10_000 }.data()), &[]).await.unwrap_err();
    assert!(err.contains("6018"), "expected FeeTooHigh(6018), got: {err}");

    send(&mut ctx, admin_ix(config, admin, raze_router::instruction::SetFee { fee_bps: 80 }.data()), &[]).await.unwrap();
    assert_eq!(read_config(&mut ctx, config).await.fee_bps, 80);
}

const SPL_TOKEN_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

/// A synthetic SPL mint account with the given decimals.
fn spl_mint_account(decimals: u8) -> solana_sdk::account::Account {
    let mut data = vec![0u8; 82];
    data[44] = decimals;
    data[45] = 1;
    solana_sdk::account::Account {
        lamports: 1_000_000_000,
        data,
        owner: SPL_TOKEN_ID.parse().unwrap(),
        executable: false,
        rent_epoch: 0,
    }
}

/// Exact-in rejects bad amounts, empty and over-long hop lists, and a pause.
#[tokio::test]
async fn route_universal_preflight_guards() {
    use raze_router::shared::{HopMetaV2, RouteArgsV2, LEG_TOKEN};

    let (mut ctx, config) = start().await;
    let ix = init_ix(&ctx, config, Pubkey::new_unique(), 0);
    send(&mut ctx, ix, &[]).await.unwrap();

    let payer = ctx.payer.pubkey();
    let spl_token: Pubkey = SPL_TOKEN_ID.parse().unwrap();
    let wsol: Pubkey = WSOL_MINT.parse().unwrap();
    let sa = raze_router::constants::SA_POOL
        [raze_router::constants::sa_pool_index(&payer) as usize]
        .0;
    ctx.set_account(&wsol, &spl_mint_account(9).into());

    let accounts = raze_router::accounts::RouteUniversal {
        payer,
        config,
        sa_authority: sa,
        user_source_token: None,
        user_destination_token: None,
        source_mint: None,
        destination_mint: None,
        source_token_program: None,
        destination_token_program: None,
        wsol_mint: wsol,
        wsol_token_program: spl_token,
        associated_token_program: ATA_PROGRAM_ID.parse().unwrap(),
        system_program: solana_sdk::system_program::ID,
        platform_fee_account: Pubkey::new_unique(),
    }
    .to_account_metas(None);

    let uni_ix = |args: RouteArgsV2| Instruction {
        program_id: program_id(),
        accounts: accounts.clone(),
        data: raze_router::instruction::RouteUniversal { args }.data(),
    };
    let hop = |source_kind: u8, dest_kind: u8| HopMetaV2 {
        program_id_index: 0,
        source_token_index: 1,
        destination_token_index: 2,
        amount_in_offset: -1,
        splice_xor_mask: [0; 8],
        source_kind,
        dest_kind,
        accounts: vec![],
        data: vec![],
    };
    let args = |hops: Vec<HopMetaV2>, fee_bps: u16| RouteArgsV2 {
        amount_in: 1_000_000,
        min_return: 1,
        hops,
        fee_bps,
        fee_on_input: false,
        sa_native_topup: 0,
    };

    let err = send(&mut ctx, uni_ix(args(vec![], 0)), &[]).await.unwrap_err();
    assert!(err.contains("6003"), "expected NoHops(6003), got: {err}");

    let err = send(&mut ctx, uni_ix(args(vec![hop(7, LEG_TOKEN)], 0)), &[]).await.unwrap_err();
    assert!(err.contains("6026"), "expected InvalidLegKind(6026), got: {err}");

    let err = send(&mut ctx, uni_ix(args(vec![hop(LEG_TOKEN, LEG_TOKEN)], 10_000)), &[]).await.unwrap_err();
    assert!(err.contains("6018"), "expected FeeTooHigh(6018), got: {err}");
}

/// Exact-out rejects bad amounts, multi-hop input, and a pause.
#[tokio::test]
async fn route_exact_out_preflight_guards() {
    use raze_router::shared::{HopMetaV2, RouteArgsExactOut, LEG_NATIVE, LEG_TOKEN};

    let (mut ctx, config) = start().await;
    let ix = init_ix(&ctx, config, Pubkey::new_unique(), 0);
    send(&mut ctx, ix, &[]).await.unwrap();

    let payer = ctx.payer.pubkey();
    let spl_token: Pubkey = SPL_TOKEN_ID.parse().unwrap();
    let wsol: Pubkey = WSOL_MINT.parse().unwrap();
    let sa = raze_router::constants::SA_POOL
        [raze_router::constants::sa_pool_index(&payer) as usize]
        .0;
    ctx.set_account(&wsol, &spl_mint_account(9).into());

    let accounts = raze_router::accounts::RouteUniversal {
        payer,
        config,
        sa_authority: sa,
        user_source_token: None,
        user_destination_token: None,
        source_mint: None,
        destination_mint: None,
        source_token_program: None,
        destination_token_program: None,
        wsol_mint: wsol,
        wsol_token_program: spl_token,
        associated_token_program: ATA_PROGRAM_ID.parse().unwrap(),
        system_program: solana_sdk::system_program::ID,
        platform_fee_account: Pubkey::new_unique(),
    }
    .to_account_metas(None);

    let exact_out_ix = |args: RouteArgsExactOut| Instruction {
        program_id: program_id(),
        accounts: accounts.clone(),
        data: raze_router::instruction::RouteExactOut { args }.data(),
    };
    let hop = |source_kind: u8, dest_kind: u8| HopMetaV2 {
        program_id_index: 0,
        source_token_index: 1,
        destination_token_index: 2,
        amount_in_offset: -1,
        splice_xor_mask: [0; 8],
        source_kind,
        dest_kind,
        accounts: vec![],
        data: vec![],
    };
    let base = |amount_out: u64, max_in: u64, hops: Vec<HopMetaV2>, fee_bps: u16| {
        RouteArgsExactOut {
            amount_out,
            max_amount_in: max_in,
            hops,
            fee_bps,
            fee_on_input: false,
            sa_native_topup: 0,
        }
    };

    let err = send(&mut ctx, exact_out_ix(base(0, 1_000, vec![hop(LEG_TOKEN, LEG_TOKEN)], 0)), &[])
        .await
        .unwrap_err();
    assert!(err.contains("6016"), "expected AmountOutMustBeGreaterThanZero(6016), got: {err}");

    let err = send(&mut ctx, exact_out_ix(base(1_000, 0, vec![hop(LEG_TOKEN, LEG_TOKEN)], 0)), &[])
        .await
        .unwrap_err();
    assert!(err.contains("6001"), "expected AmountInMustBeGreaterThanZero(6001), got: {err}");

    let two = vec![hop(LEG_TOKEN, LEG_TOKEN), hop(LEG_TOKEN, LEG_TOKEN)];
    let err = send(&mut ctx, exact_out_ix(base(1_000, 2_000, two, 0)), &[])
        .await
        .unwrap_err();
    assert!(err.contains("6024"), "expected NativeRouteMustBeSingleHop(6024), got: {err}");

    let err = send(&mut ctx, exact_out_ix(base(1_000, 2_000, vec![hop(LEG_TOKEN, LEG_TOKEN)], 10_000)), &[])
        .await
        .unwrap_err();
    assert!(err.contains("6018"), "expected FeeTooHigh(6018), got: {err}");

    let err = send(&mut ctx, exact_out_ix(base(1_000, 2_000, vec![hop(7, LEG_TOKEN)], 0)), &[])
        .await
        .unwrap_err();
    assert!(err.contains("6026"), "expected InvalidLegKind(6026), got: {err}");

    let err = send(&mut ctx, exact_out_ix(base(1_000, 2_000, vec![hop(LEG_TOKEN, LEG_NATIVE)], 0)), &[])
        .await
        .unwrap_err();
    assert!(err.contains("6006"), "expected InvalidAccountsLength(6006), got: {err}");
}

/// A non-admin signer cannot mutate the config.
#[tokio::test]
async fn admin_gating() {
    let (mut ctx, config) = start().await;
    let ix = init_ix(&ctx, config, Pubkey::new_unique(), 25);
    send(&mut ctx, ix, &[]).await.unwrap();

    let outsider = Keypair::new();
    let ix = admin_ix(config, outsider.pubkey(), raze_router::instruction::SetFee { fee_bps: 0 }.data());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer.insecure_clone(), &outsider],
        ctx.last_blockhash,
    );
    let err = format!("{:?}", ctx.banks_client.process_transaction(tx).await.unwrap_err());
    assert!(err.contains("6020"), "expected Unauthorized(6020), got: {err}");
}

/// `add_venue` refuses the token, system, ATA and router programs.
#[tokio::test]
async fn venue_denylist_enforced() {
    let (mut ctx, config) = start().await;
    let admin = ctx.payer.pubkey();
    let ix = init_ix(&ctx, config, Pubkey::new_unique(), 0);
    send(&mut ctx, ix, &[]).await.unwrap();

    let t22: Pubkey = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse().unwrap();
    for forbidden in [
        program_id(),
        solana_sdk::system_program::ID,
        SPL_TOKEN_ID.parse().unwrap(),
        t22,
        ATA_PROGRAM_ID.parse().unwrap(),
    ] {
        let err = send(&mut ctx, admin_ix(config, admin, raze_router::instruction::AddVenue { program_id: forbidden }.data()), &[])
            .await
            .unwrap_err();
        assert!(err.contains("6034"), "expected VenueProgramForbidden(6034) for {forbidden}, got: {err}");
    }
    assert_eq!(read_config(&mut ctx, config).await.venue_count, 0);

    let pumpswap: Pubkey = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA".parse().unwrap();
    send(&mut ctx, admin_ix(config, admin, raze_router::instruction::AddVenue { program_id: pumpswap }.data()), &[]).await.unwrap();
    assert_eq!(read_config(&mut ctx, config).await.venue_count, 1);
}

/// `set_admin` refuses the zero address.
#[tokio::test]
async fn set_admin_rejects_zero_address() {
    let (mut ctx, config) = start().await;
    let admin = ctx.payer.pubkey();
    let ix = init_ix(&ctx, config, Pubkey::new_unique(), 0);
    send(&mut ctx, ix, &[]).await.unwrap();

    let err = send(&mut ctx, admin_ix(config, admin, raze_router::instruction::SetAdmin { new_admin: Pubkey::default() }.data()), &[])
        .await
        .unwrap_err();
    assert!(err.contains("6035"), "expected InvalidAdminAddress(6035), got: {err}");

    let new_admin = Pubkey::new_unique();
    send(&mut ctx, admin_ix(config, admin, raze_router::instruction::SetAdmin { new_admin }.data()), &[]).await.unwrap();
    assert_eq!(read_config(&mut ctx, config).await.admin, new_admin);
}

/// The unified instruction routes each mode and rejects unknown ones.
#[tokio::test]
async fn route_unified_dispatches_by_mode() {
    use raze_router::shared::{
        HopMetaV2, RouteArgsUnified, LEG_TOKEN, ROUTE_MODE_EXACT_IN, ROUTE_MODE_EXACT_OUT,
    };

    let (mut ctx, config) = start().await;
    let ix = init_ix(&ctx, config, Pubkey::new_unique(), 0);
    send(&mut ctx, ix, &[]).await.unwrap();

    let payer = ctx.payer.pubkey();
    let spl_token: Pubkey = SPL_TOKEN_ID.parse().unwrap();
    let wsol: Pubkey = WSOL_MINT.parse().unwrap();
    let sa = raze_router::constants::SA_POOL
        [raze_router::constants::sa_pool_index(&payer) as usize]
        .0;
    ctx.set_account(&wsol, &spl_mint_account(9).into());

    let accounts = raze_router::accounts::RouteUniversal {
        payer,
        config,
        sa_authority: sa,
        user_source_token: None,
        user_destination_token: None,
        source_mint: None,
        destination_mint: None,
        source_token_program: None,
        destination_token_program: None,
        wsol_mint: wsol,
        wsol_token_program: spl_token,
        associated_token_program: ATA_PROGRAM_ID.parse().unwrap(),
        system_program: solana_sdk::system_program::ID,
        platform_fee_account: Pubkey::new_unique(),
    }
    .to_account_metas(None);

    let unified_ix = |args: RouteArgsUnified| Instruction {
        program_id: program_id(),
        accounts: accounts.clone(),
        data: raze_router::instruction::RouteUnified { args }.data(),
    };
    let args = |mode: u8, amount_a: u64, amount_b: u64| RouteArgsUnified {
        mode,
        amount_a,
        amount_b,
        hops: vec![HopMetaV2 {
            program_id_index: 0,
            source_token_index: 1,
            destination_token_index: 2,
            amount_in_offset: -1,
            splice_xor_mask: [0; 8],
            source_kind: LEG_TOKEN,
            dest_kind: LEG_TOKEN,
            accounts: vec![],
            data: vec![],
        }],
        fee_bps: 0,
        fee_on_input: false,
        sa_native_topup: 0,
    };

    let err = send(&mut ctx, unified_ix(args(ROUTE_MODE_EXACT_IN, 0, 1_000)), &[])
        .await
        .unwrap_err();
    assert!(err.contains("6001"), "mode 0 must reach the exact-IN guard (6001), got: {err}");

    let err = send(&mut ctx, unified_ix(args(ROUTE_MODE_EXACT_OUT, 0, 1_000)), &[])
        .await
        .unwrap_err();
    assert!(err.contains("6016"), "mode 1 must reach the exact-OUT guard (6016), got: {err}");

    for bad in [2u8, 0xFF] {
        let err = send(&mut ctx, unified_ix(args(bad, 1_000, 900)), &[])
            .await
            .unwrap_err();
        assert!(err.contains("6036"), "mode {bad} must revert 6036, got: {err}");
    }

    let legacy = Instruction {
        program_id: program_id(),
        accounts: accounts.clone(),
        data: raze_router::instruction::RouteUniversal {
            args: raze_router::shared::RouteArgsV2 {
                amount_in: 0,
                min_return: 1_000,
                hops: vec![],
                fee_bps: 0,
                fee_on_input: false,
                sa_native_topup: 0,
            },
        }
        .data(),
    };
    let err = send(&mut ctx, legacy, &[]).await.unwrap_err();
    assert!(err.contains("6001"), "legacy route_universal must still work, got: {err}");
}
