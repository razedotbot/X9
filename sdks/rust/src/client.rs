use std::time::Duration;

use crate::error::{RazeTradingError, Result};
use crate::http::HttpClient;
use crate::types::*;

pub struct RazeTrading {
    http: HttpClient,
}

pub struct RazeTradingBuilder {
    api_key: String,
    base_url: String,
    timeout: Duration,
    max_retries: u32,
}

impl RazeTradingBuilder {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_owned(),
            base_url: "http://localhost:8082".to_owned(),
            timeout: Duration::from_secs(30),
            max_retries: 2,
        }
    }

    pub fn base_url(mut self, url: &str) -> Self {
        self.base_url = url.to_owned();
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    pub fn build(self) -> Result<RazeTrading> {
        let http = HttpClient::new(&self.base_url, &self.api_key, self.timeout, self.max_retries)?;
        Ok(RazeTrading { http })
    }
}

impl RazeTrading {
    pub fn builder(api_key: &str) -> RazeTradingBuilder {
        RazeTradingBuilder::new(api_key)
    }

    /// GET /health -> ApiResponse<HealthData>
    /// Returns the inner HealthData on success, or an error.
    pub async fn health(&self) -> Result<HealthData> {
        let resp: ApiResponse<HealthData> = self.http.get("/health", &[]).await?;
        if resp.success {
            resp.data.ok_or_else(|| {
                RazeTradingError::Other("health response missing data field".into())
            })
        } else {
            Err(RazeTradingError::Other(
                resp.error.unwrap_or_else(|| "unknown error".into()),
            ))
        }
    }

    /// GET /swap/sol/quote/:mint?inputMint=&slippageBps=&amount=
    /// Returns the inner QuoteData on success, or an error.
    pub async fn quote(&self, mint: &str, params: Option<&QuoteParams>) -> Result<QuoteData> {
        let path = format!("/swap/sol/quote/{}", mint);
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(p) = params {
            if let Some(ref input_mint) = p.input_mint {
                query.push(("inputMint", input_mint.clone()));
            }
            if let Some(slippage_bps) = p.slippage_bps {
                query.push(("slippageBps", slippage_bps.to_string()));
            }
            if let Some(amount) = p.amount {
                query.push(("amount", amount.to_string()));
            }
        }
        let query_refs: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let resp: ApiResponse<QuoteData> = self.http.get(&path, &query_refs).await?;
        if resp.success {
            resp.data.ok_or_else(|| {
                RazeTradingError::Other("quote response missing data field".into())
            })
        } else {
            Err(RazeTradingError::Other(
                resp.error.unwrap_or_else(|| "unknown error".into()),
            ))
        }
    }

    /// POST /swap/sol/buy -> SwapResponse
    pub async fn buy(&self, opts: &BuyOpts) -> Result<SwapResponse> {
        let resp: SwapResponse = self.http.post("/swap/sol/buy", opts).await?;
        if resp.success {
            Ok(resp)
        } else {
            Err(RazeTradingError::Other(
                resp.error.unwrap_or_else(|| "buy failed".into()),
            ))
        }
    }

    /// POST /swap/sol/sell -> SwapResponse
    pub async fn sell(&self, opts: &SellOpts) -> Result<SwapResponse> {
        let resp: SwapResponse = self.http.post("/swap/sol/sell", opts).await?;
        if resp.success {
            Ok(resp)
        } else {
            Err(RazeTradingError::Other(
                resp.error.unwrap_or_else(|| "sell failed".into()),
            ))
        }
    }

    /// POST /swap/sol/instructions -> InstructionsResponse
    pub async fn instructions(&self, opts: &InstructionsOpts) -> Result<InstructionsResponse> {
        let resp: InstructionsResponse =
            self.http.post("/swap/sol/instructions", opts).await?;
        if resp.success {
            Ok(resp)
        } else {
            Err(RazeTradingError::Other(
                resp.error
                    .unwrap_or_else(|| "instructions failed".into()),
            ))
        }
    }

}
