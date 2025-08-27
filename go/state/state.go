package state

import (
	"sync"
	"sync/atomic"

	"vuio-go/config"
	"vuio-go/database"
	"vuio-go/platform"
)

// AppState holds the shared state of the application.
type AppState struct {
	Config          *config.AppConfig
	DB              database.Manager
	Platform        *platform.Platform
	ContentUpdateID atomic.Uint32
	mu              sync.RWMutex
}

// New creates a new AppState.
func New(cfg *config.AppConfig, db database.Manager, plat *platform.Platform) *AppState {
	s := &AppState{
		Config:   cfg,
		DB:       db,
		Platform: plat,
	}
	s.ContentUpdateID.Store(1)
	return s
}

// GetConfig returns the current configuration safely.
func (s *AppState) GetConfig() *config.AppConfig {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.Config
}

// SetConfig updates the configuration safely.
func (s *AppState) SetConfig(cfg *config.AppConfig) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.Config = cfg
}

// IncrementUpdateID increments the content update ID and returns the new value.
func (s *AppState) IncrementUpdateID() uint32 {
	return s.ContentUpdateID.Add(1)
}

// GetUpdateID returns the current content update ID.
func (s *AppState) GetUpdateID() uint32 {
	return s.ContentUpdateID.Load()
}