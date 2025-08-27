package apperror

import (
	"fmt"
	"net/http"
)

// AppError represents a custom application error.
type AppError struct {
	Code    int
	Message string
	Err     error
}

func (e *AppError) Error() string {
	if e.Err != nil {
		return fmt.Sprintf("%s: %v", e.Message, e.Err)
	}
	return e.Message
}

// New creates a new AppError.
func New(code int, message string, err error) *AppError {
	return &AppError{Code: code, Message: message, Err: err}
}

// NotFound creates a new 404 Not Found error.
func NotFound(err error) *AppError {
	return New(http.StatusNotFound, "Not Found", err)
}

// InvalidRange creates a new 416 Range Not Satisfiable error.
func InvalidRange(err error) *AppError {
	return New(http.StatusRequestedRangeNotSatisfiable, "Invalid Range", err)
}

// Internal creates a new 500 Internal Server Error.
func Internal(err error) *AppError {
	return New(http.StatusInternalServerError, "Internal Server Error", err)
}

// BadRequest creates a new 400 Bad Request error.
func BadRequest(message string, err error) *AppError {
	return New(http.StatusBadRequest, message, err)
}