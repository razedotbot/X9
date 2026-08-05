use serde::{Deserialize, Serialize};

// --- Wrapper for endpoints that return {success, data, error} ---

#[derive(Debug, Clone, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

// --- Health ---

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthData {
    pub status: String,
    /// Server always populates these (`api/types.rs` `HealthResponse`); kept as
    /// `Option` only to tolerate older builds / health aliases. Defaults to 0.
    #[serde(default)]
    pub slot: u64,
    #[serde(default)]
    pub accounts: u64,
}

// --- Quote ---

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_mint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slippage_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteData {
    pub mint: String,
    pub avg_price: f64,
    pub amount_in: u64,
    pub amount_out: u64,
    pub platform: String,
    pub pool: String,
    pub time_taken: f64,
}

// --- Buy ---

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuyOpts {
    pub wallet_addresses: Vec<String>,
    pub token_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sol_amount: Option<f64>,
    /// Per-wallet SOL amounts (alternative to `sol_amount`). Length must match
    /// `wallet_addresses`. Server field `amounts`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amounts: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slippage_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_mint: Option<String>,
    /// Raw input amount in smallest units — **required** when `input_mint` is
    /// set (non-SOL input buys). Server field `inputAmountRaw`; the router
    /// rejects the request when it is missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_amount_raw: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tip_wallet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tip_lamports: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_wallet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_tip_lamports: Option<u64>,
    /// Priority fee per transaction in lamports (compute budget). Server field
    /// `transactionsFeeLamports`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transactions_fee_lamports: Option<u64>,
}

// --- Sell ---

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SellOpts {
    pub wallet_addresses: Vec<String>,
    pub token_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f64>,
    /// Scalar or array — server accepts either.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_amount: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slippage_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_mint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tip_wallet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tip_lamports: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_wallet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_tip_lamports: Option<u64>,
    /// Priority fee per transaction in lamports (compute budget).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transactions_fee_lamports: Option<u64>,
}

// --- Swap Response (shared by buy and sell) ---

#[derive(Debug, Clone, Deserialize)]
pub struct SwapResponse {
    pub success: bool,
    #[serde(default)]
    pub transactions: Vec<String>,
    pub error: Option<String>,
}

// --- Instructions ---

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionsOpts {
    pub wallet: String,
    pub input_mint: String,
    pub output_mint: String,
    pub amount: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slippage_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swap_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tip_wallet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tip_lamports: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_wallet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_tip_lamports: Option<u64>,
    /// Priority fee per transaction in lamports (compute budget).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transactions_fee_lamports: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionsResponse {
    pub success: bool,
    #[serde(default)]
    pub instructions: Vec<serde_json::Value>,
    #[serde(default)]
    pub address_lookup_table_addresses: Vec<String>,
    pub error: Option<String>,
}
