export interface RazeTradingConfig {
  baseUrl?: string;
  timeout?: number;
  maxRetries?: number;
}

// GET /health

export interface HealthData {
  status: string;
  slot: number;
  accounts: number;
}

export interface HealthResponse {
  success: boolean;
  data: HealthData;
}

// GET /swap/sol/quote/:mint

export interface QuoteOpts {
  inputMint?: string;
  slippageBps?: number;
  amount?: number;
}

export interface QuoteData {
  mint: string;
  avgPrice: number;
  amountIn: number;
  amountOut: number;
  platform: string;
  pool: string;
  timeTaken: number;
}

export interface QuoteResponse {
  success: boolean;
  data: QuoteData;
  error?: string;
}

// POST /swap/sol/buy

export interface BuyOpts {
  walletAddresses: string[];
  tokenAddress: string;
  solAmount?: number;
  /** Per-wallet SOL amounts (alternative to solAmount). Length must match walletAddresses. */
  amounts?: number[];
  slippageBps?: number;
  encoding?: "base64" | "base58";
  inputMint?: string;
  /** Required when inputMint is set (non-SOL input). Raw token units. */
  inputAmountRaw?: number;
  tipWallet?: string;
  tipLamports?: number;
  feeWallet?: string;
  feeBps?: number;
  feeTipLamports?: number;
  /** Priority fee per transaction in lamports (compute budget). */
  transactionsFeeLamports?: number;
}

// POST /swap/sol/sell

export interface SellOpts {
  walletAddresses: string[];
  tokenAddress: string;
  percentage?: number;
  tokensAmount?: number | number[];
  slippageBps?: number;
  encoding?: "base64" | "base58";
  outputMint?: string;
  tipWallet?: string;
  tipLamports?: number;
  feeWallet?: string;
  feeBps?: number;
  feeTipLamports?: number;
  /** Priority fee per transaction in lamports (compute budget). */
  transactionsFeeLamports?: number;
}

// Shared swap response for buy and sell

export interface SwapResponse {
  success: boolean;
  transactions?: string[];
  error?: string;
}

// POST /swap/sol/instructions

export interface InstructionsOpts {
  wallet: string;
  inputMint: string;
  outputMint: string;
  amount: number;
  slippageBps?: number;
  swapMode?: "exactIn" | "exactOut";
  tipWallet?: string;
  tipLamports?: number;
  feeWallet?: string;
  feeBps?: number;
  feeTipLamports?: number;
  /** Priority fee per transaction in lamports (compute budget). */
  transactionsFeeLamports?: number;
}

export interface InstructionsResponse {
  success: boolean;
  instructions?: unknown[];
  addressLookupTableAddresses: string[];
  error?: string;
}
