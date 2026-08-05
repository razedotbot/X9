package razetrading

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"math"
	"net/http"
	"strconv"
	"strings"
	"time"
)

// httpTransport handles low-level HTTP communication with retry and backoff.
type httpTransport struct {
	baseURL    string
	apiKey     string
	timeout    time.Duration
	maxRetries int
	client     *http.Client
}

func newHTTPTransport(cfg config) *httpTransport {
	client := cfg.httpClient
	if client == nil {
		client = &http.Client{Timeout: cfg.timeout}
	}
	return &httpTransport{
		baseURL:    strings.TrimRight(cfg.baseURL, "/"),
		apiKey:     cfg.apiKey,
		timeout:    cfg.timeout,
		maxRetries: cfg.maxRetries,
		client:     client,
	}
}

func (h *httpTransport) get(ctx context.Context, path string, params map[string]string) ([]byte, error) {
	url := h.baseURL + path
	if len(params) > 0 {
		parts := make([]string, 0, len(params))
		for k, v := range params {
			parts = append(parts, k+"="+v)
		}
		url += "?" + strings.Join(parts, "&")
	}
	return h.do(ctx, http.MethodGet, url, nil)
}

func (h *httpTransport) post(ctx context.Context, path string, body interface{}) ([]byte, error) {
	data, err := json.Marshal(body)
	if err != nil {
		return nil, fmt.Errorf("marshal request body: %w", err)
	}
	return h.do(ctx, http.MethodPost, h.baseURL+path, data)
}

func (h *httpTransport) do(ctx context.Context, method, url string, body []byte) ([]byte, error) {
	var lastErr error

	for attempt := 0; attempt <= h.maxRetries; attempt++ {
		if attempt > 0 {
			delay := time.Duration(200*math.Pow(2, float64(attempt-1))) * time.Millisecond
			timer := time.NewTimer(delay)
			select {
			case <-ctx.Done():
				timer.Stop()
				return nil, ctx.Err()
			case <-timer.C:
			}
		}

		var reqBody io.Reader
		if body != nil {
			reqBody = bytes.NewReader(body)
		}

		req, err := http.NewRequestWithContext(ctx, method, url, reqBody)
		if err != nil {
			return nil, fmt.Errorf("create request: %w", err)
		}

		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("User-Agent", "raze-trading-go/0.3.0")
		// Route the credential to the header the router reads for its kind.
		if h.apiKey != "" {
			if strings.HasPrefix(h.apiKey, "sk_") || strings.HasPrefix(h.apiKey, "eyJ") {
				req.Header.Set("Authorization", "Bearer "+h.apiKey)
			} else {
				// A license key lands here: X-Api-Key is checked first.
				req.Header.Set("X-Api-Key", h.apiKey)
			}
		}

		resp, err := h.client.Do(req)
		if err != nil {
			lastErr = &RazeError{Message: err.Error(), Code: "REQUEST_FAILED"}
			continue
		}

		respBody, err := io.ReadAll(resp.Body)
		resp.Body.Close()
		if err != nil {
			lastErr = &RazeError{Message: "read response: " + err.Error(), Code: "READ_FAILED"}
			continue
		}

		if resp.StatusCode >= 200 && resp.StatusCode < 300 {
			return respBody, nil
		}

		errMsg := extractErrorMessage(respBody)

		switch {
		case resp.StatusCode == 401:
			return nil, &AuthError{RazeError{
				Message:    orDefault(errMsg, "Unauthorized"),
				StatusCode: 401,
				Code:       "AUTH_ERROR",
				Body:       string(respBody),
			}}
		case resp.StatusCode == 400 || resp.StatusCode == 422:
			return nil, &ValidationError{RazeError{
				Message:    orDefault(errMsg, fmt.Sprintf("Validation error (%d)", resp.StatusCode)),
				StatusCode: resp.StatusCode,
				Code:       "VALIDATION_ERROR",
				Body:       string(respBody),
			}}
		case resp.StatusCode == 429:
			retryAfter := 0
			if ra := resp.Header.Get("Retry-After"); ra != "" {
				retryAfter, _ = strconv.Atoi(ra)
			}
			return nil, &RateLimitError{
				RazeError: RazeError{
					Message:    "Rate limit exceeded",
					StatusCode: 429,
					Code:       "RATE_LIMIT",
					Body:       string(respBody),
				},
				RetryAfter: retryAfter,
			}
		case resp.StatusCode >= 500:
			lastErr = &ServerError{RazeError{
				Message:    orDefault(errMsg, fmt.Sprintf("Server error (%d)", resp.StatusCode)),
				StatusCode: resp.StatusCode,
				Code:       "SERVER_ERROR",
				Body:       string(respBody),
			}}
			continue
		default:
			return nil, &RazeError{
				Message:    orDefault(errMsg, fmt.Sprintf("HTTP %d", resp.StatusCode)),
				StatusCode: resp.StatusCode,
				Code:       "HTTP_ERROR",
				Body:       string(respBody),
			}
		}
	}

	if lastErr != nil {
		return nil, lastErr
	}
	return nil, &RazeError{Message: "Request failed", Code: "UNKNOWN"}
}

func extractErrorMessage(body []byte) string {
	var obj struct {
		Error string `json:"error"`
	}
	if json.Unmarshal(body, &obj) == nil && obj.Error != "" {
		return obj.Error
	}
	s := string(body)
	if len(s) < 500 {
		return s
	}
	return ""
}

func orDefault(s, fallback string) string {
	if s != "" {
		return s
	}
	return fallback
}
