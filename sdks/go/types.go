package razetrading

import (
	"encoding/json"
	"net/http"
	"time"
)

// config holds client configuration set via Option functions.
type config struct {
	baseURL    string
	apiKey     string
	timeout    time.Duration
	maxRetries int
	httpClient *http.Client
}

// Option configures the Client.
type Option func(*config)

// ---------------------------------------------------------------------------
// Generic API wrapper
// ---------------------------------------------------------------------------

// ApiResponse is the generic envelope returned by GET endpoints.
type ApiResponse[T any] struct {
	Success bool   `json:"success"`
	Data    T      `json:"data,omitempty"`
	Error   string `json:"error,omitempty"`
}

// ---------------------------------------------------------------------------
// GET /health
// ---------------------------------------------------------------------------

// HealthResponse is the data payload of the health endpoint.
type HealthResponse struct {
	Status   string `json:"status"`
	Slot     uint64 `json:"slot"`
	Accounts int    `json:"accounts"`
}

// ---------------------------------------------------------------------------
// GET /swap/sol/quote/:mint
// ---------------------------------------------------------------------------

// QuoteParams holds query parameters for the Quote endpoint.
// Amount is uint64: the server's `amount` is u64; a 32-bit int
// would truncate large lamport amounts.
type QuoteParams struct {
	InputMint   string  `json:"inputMint,omitempty"`
	SlippageBps *int    `json:"slippageBps,omitempty"`
	Amount      *uint64 `json:"amount,omitempty"`
}

// QuoteData is the data payload of a successful quote.
type QuoteData struct {
	Mint      string  `json:"mint"`
	AvgPrice  float64 `json:"avgPrice"`
	AmountIn  uint64  `json:"amountIn"`
	AmountOut uint64  `json:"amountOut"`
	Platform  string  `json:"platform"`
	Pool      string  `json:"pool"`
	TimeTaken float64 `json:"timeTaken"`
}

// QuoteResponse is the full quote endpoint response.
type QuoteResponse struct {
	Success bool       `json:"success"`
	Data    *QuoteData `json:"data,omitempty"`
	Error   string     `json:"error,omitempty"`
}

// ---------------------------------------------------------------------------
// POST /swap/sol/buy
// ---------------------------------------------------------------------------

// BuyOpts is the request body for the buy endpoint.
type BuyOpts struct {
	WalletAddresses []string  `json:"walletAddresses"`
	TokenAddress    string    `json:"tokenAddress"`
	SolAmount       *float64  `json:"solAmount,omitempty"`
	// Amounts holds per-wallet SOL amounts (alternative to SolAmount).
	// Length must match WalletAddresses. Server field `amounts`.
	Amounts     []float64 `json:"amounts,omitempty"`
	SlippageBps *int      `json:"slippageBps,omitempty"`
	Encoding    string    `json:"encoding,omitempty"`
	InputMint   string    `json:"inputMint,omitempty"`
	// InputAmountRaw is the raw input amount in smallest units — REQUIRED
	// when InputMint is set (non-SOL input buys). Server field
	// `inputAmountRaw`.
	InputAmountRaw *uint64 `json:"inputAmountRaw,omitempty"`
	TipWallet      string  `json:"tipWallet,omitempty"`
	TipLamports    *uint64 `json:"tipLamports,omitempty"`
	FeeWallet      string  `json:"feeWallet,omitempty"`
	FeeBps         *int    `json:"feeBps,omitempty"`
	FeeTipLamports *uint64 `json:"feeTipLamports,omitempty"`
	// TransactionsFeeLamports is the priority fee per transaction in lamports
	// (compute budget). Server field `transactionsFeeLamports`.
	TransactionsFeeLamports *uint64 `json:"transactionsFeeLamports,omitempty"`
}

// ---------------------------------------------------------------------------
// POST /swap/sol/sell
// ---------------------------------------------------------------------------

// SellOpts is the request body for the sell endpoint.
// TokensAmount accepts a scalar or an array; pass a
// marshalled json.RawMessage (e.g. json.RawMessage("123") or "[1,2]").
type SellOpts struct {
	WalletAddresses []string        `json:"walletAddresses"`
	TokenAddress    string          `json:"tokenAddress"`
	Percentage      *float64        `json:"percentage,omitempty"`
	TokensAmount    json.RawMessage `json:"tokensAmount,omitempty"`
	SlippageBps     *int            `json:"slippageBps,omitempty"`
	Encoding        string          `json:"encoding,omitempty"`
	OutputMint      string          `json:"outputMint,omitempty"`
	TipWallet       string          `json:"tipWallet,omitempty"`
	TipLamports     *uint64         `json:"tipLamports,omitempty"`
	FeeWallet       string          `json:"feeWallet,omitempty"`
	FeeBps          *int            `json:"feeBps,omitempty"`
	FeeTipLamports  *uint64         `json:"feeTipLamports,omitempty"`
	// TransactionsFeeLamports is the priority fee per transaction in lamports.
	TransactionsFeeLamports *uint64 `json:"transactionsFeeLamports,omitempty"`
}

// ---------------------------------------------------------------------------
// POST /swap/sol/instructions
// ---------------------------------------------------------------------------

// InstructionsOpts is the request body for the instructions endpoint.
type InstructionsOpts struct {
	Wallet         string  `json:"wallet"`
	InputMint      string  `json:"inputMint"`
	OutputMint     string  `json:"outputMint"`
	Amount         uint64  `json:"amount"`
	SlippageBps    *int    `json:"slippageBps,omitempty"`
	SwapMode       string  `json:"swapMode,omitempty"`
	TipWallet      string  `json:"tipWallet,omitempty"`
	TipLamports    *uint64 `json:"tipLamports,omitempty"`
	FeeWallet      string  `json:"feeWallet,omitempty"`
	FeeBps         *int    `json:"feeBps,omitempty"`
	FeeTipLamports *uint64 `json:"feeTipLamports,omitempty"`
	// TransactionsFeeLamports is the priority fee per transaction in lamports.
	TransactionsFeeLamports *uint64 `json:"transactionsFeeLamports,omitempty"`
}

// InstructionsResponse is the response from the instructions endpoint.
type InstructionsResponse struct {
	Success                     bool              `json:"success"`
	Instructions                []json.RawMessage `json:"instructions,omitempty"`
	AddressLookupTableAddresses []string          `json:"addressLookupTableAddresses"`
	Error                       string            `json:"error,omitempty"`
}

// ---------------------------------------------------------------------------
// Shared swap response (buy / sell)
// ---------------------------------------------------------------------------

// SwapResponse is returned by the buy and sell endpoints.
type SwapResponse struct {
	Success      bool     `json:"success"`
	Transactions []string `json:"transactions,omitempty"`
	Error        string   `json:"error,omitempty"`
}
