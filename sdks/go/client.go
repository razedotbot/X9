package razetrading

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"net/http"
	"time"
)

// Client is the Raze trading API client.
type Client struct {
	http *httpTransport
}

// New creates a new Client with the given API key and optional configuration.
func New(apiKey string, opts ...Option) *Client {
	cfg := config{
		baseURL:    "http://localhost:8082",
		apiKey:     apiKey,
		timeout:    30 * time.Second,
		maxRetries: 2,
	}
	for _, o := range opts {
		o(&cfg)
	}
	return &Client{http: newHTTPTransport(cfg)}
}

// WithBaseURL overrides the default base URL (http://localhost:8082).
func WithBaseURL(u string) Option {
	return func(c *config) { c.baseURL = u }
}

// WithTimeout sets the HTTP request timeout.
func WithTimeout(d time.Duration) Option {
	return func(c *config) { c.timeout = d }
}

// WithMaxRetries sets the maximum number of retry attempts for retriable errors.
func WithMaxRetries(n int) Option {
	return func(c *config) { c.maxRetries = n }
}

// WithHTTPClient sets a custom *http.Client for the transport.
func WithHTTPClient(hc *http.Client) Option {
	return func(c *config) { c.httpClient = hc }
}

// Health checks the API server health.
// GET /health
func (c *Client) Health(ctx context.Context) (*HealthResponse, error) {
	data, err := c.http.get(ctx, "/health", nil)
	if err != nil {
		return nil, err
	}
	var resp ApiResponse[HealthResponse]
	if err := json.Unmarshal(data, &resp); err != nil {
		return nil, fmt.Errorf("unmarshal health response: %w", err)
	}
	if !resp.Success {
		return nil, &RazeError{Message: orDefault(resp.Error, "health check failed"), Code: "HEALTH_ERROR"}
	}
	return &resp.Data, nil
}

// Quote fetches a swap quote for the given token mint.
// GET /swap/sol/quote/:mint
func (c *Client) Quote(ctx context.Context, mint string, params *QuoteParams) (*QuoteResponse, error) {
	qp := make(map[string]string)
	if params != nil {
		if params.InputMint != "" {
			qp["inputMint"] = params.InputMint
		}
		if params.SlippageBps != nil {
			qp["slippageBps"] = fmt.Sprintf("%d", *params.SlippageBps)
		}
		if params.Amount != nil {
			qp["amount"] = fmt.Sprintf("%d", *params.Amount)
		}
	}
	data, err := c.http.get(ctx, "/swap/sol/quote/"+mint, qp)
	if err != nil {
		return nil, err
	}
	var resp QuoteResponse
	if err := json.Unmarshal(data, &resp); err != nil {
		return nil, fmt.Errorf("unmarshal quote response: %w", err)
	}
	return &resp, nil
}

// Buy creates swap-buy transactions for the given wallets and token.
// POST /swap/sol/buy
func (c *Client) Buy(ctx context.Context, opts BuyOpts) (*SwapResponse, error) {
	return c.postSwap(ctx, "/swap/sol/buy", opts)
}

// Sell creates swap-sell transactions for the given wallets and token.
// POST /swap/sol/sell
func (c *Client) Sell(ctx context.Context, opts SellOpts) (*SwapResponse, error) {
	return c.postSwap(ctx, "/swap/sol/sell", opts)
}

// Instructions returns raw swap instructions for client-side transaction building.
// POST /swap/sol/instructions
func (c *Client) Instructions(ctx context.Context, opts InstructionsOpts) (*InstructionsResponse, error) {
	data, err := c.http.post(ctx, "/swap/sol/instructions", opts)
	if err != nil {
		return nil, err
	}
	var resp InstructionsResponse
	if err := json.Unmarshal(data, &resp); err != nil {
		return nil, fmt.Errorf("unmarshal instructions response: %w", err)
	}
	return &resp, nil
}

func (c *Client) postSwap(ctx context.Context, path string, body interface{}) (*SwapResponse, error) {
	data, err := c.http.post(ctx, path, body)
	if err != nil {
		return nil, err
	}
	var resp SwapResponse
	if err := json.Unmarshal(data, &resp); err != nil {
		return nil, fmt.Errorf("unmarshal swap response: %w", err)
	}
	return &resp, nil
}
