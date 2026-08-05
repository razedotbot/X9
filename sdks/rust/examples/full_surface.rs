//! Tour of the swap surface a self-hosted raze-router serves. Run with:
//!
//!   cargo run --example full_surface
//!
//! Nothing is submitted on-chain — the router returns unsigned transactions
//! that you sign and send yourself.

use raze_trading::{BuyOpts, QuoteParams, RazeTrading, SellOpts};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Your license key. It travels as X-Api-Key, which the router checks first.
    let client = RazeTrading::builder("rzl_your_license_key")
        .base_url("http://localhost:8082")
        .build()?;

    // Health is unauthenticated.
    let health = client.health().await?;
    println!("health: {} slot={}", health.status, health.slot);

    // Exact-in quote: 1 SOL into the mint, 0.5% slippage.
    let quote = client
        .quote(
            "TokenMintPubkey",
            Some(&QuoteParams {
                amount: Some(1_000_000_000),
                slippage_bps: Some(50),
                ..Default::default()
            }),
        )
        .await?;
    println!("quote out: {} via {}", quote.amount_out, quote.platform);

    // Buy. Send the key: an unauthenticated call falls back to the compiled
    // public fee tier, whose recipient is not yours. A non-SOL input needs
    // `input_mint` + `input_amount_raw`.
    let buy = client
        .buy(&BuyOpts {
            wallet_addresses: vec!["YourWalletPubkey".into()],
            token_address: "TokenMintPubkey".into(),
            sol_amount: Some(0.1),
            ..Default::default()
        })
        .await?;
    // Entries are index-aligned with `wallet_addresses`; a wallet the router
    // could not build for comes back as an empty string, so skip those.
    let built = buy.transactions.iter().filter(|t| !t.is_empty()).count();
    println!("buy txs: {} of {}", built, buy.transactions.len());

    // Sell the whole position.
    let sell = client
        .sell(&SellOpts {
            wallet_addresses: vec!["YourWalletPubkey".into()],
            token_address: "TokenMintPubkey".into(),
            percentage: Some(100.0),
            ..Default::default()
        })
        .await?;
    println!("sell entries: {}", sell.transactions.len());

    Ok(())
}
