package razetrading

import (
	"errors"
	"fmt"
)

// RazeError is the base error type for all API errors.
type RazeError struct {
	Message    string
	StatusCode int
	Code       string
	Body       string
}

func (e *RazeError) Error() string {
	if e.StatusCode > 0 {
		return fmt.Sprintf("%s (status %d)", e.Message, e.StatusCode)
	}
	return e.Message
}

// AuthError is returned when the API key is missing or invalid (HTTP 401).
type AuthError struct{ RazeError }

// ValidationError is returned for bad requests (HTTP 400/422).
type ValidationError struct{ RazeError }

// RateLimitError is returned when the rate limit is exceeded (HTTP 429).
type RateLimitError struct {
	RazeError
	RetryAfter int
}

// ServerError is returned for server-side failures (HTTP 5xx). These are retried.
type ServerError struct{ RazeError }

// IsRazeError reports whether err is any Raze API error.
func IsRazeError(err error) bool {
	var target *RazeError
	return errors.As(err, &target)
}

// IsAuthError reports whether err is an authentication error.
func IsAuthError(err error) bool {
	var target *AuthError
	return errors.As(err, &target)
}

// IsValidationError reports whether err is a validation error.
func IsValidationError(err error) bool {
	var target *ValidationError
	return errors.As(err, &target)
}

// IsRateLimitError reports whether err is a rate limit error.
func IsRateLimitError(err error) bool {
	var target *RateLimitError
	return errors.As(err, &target)
}

// IsServerError reports whether err is a server error.
func IsServerError(err error) bool {
	var target *ServerError
	return errors.As(err, &target)
}
