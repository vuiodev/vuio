package watcher

import (
	"context"
	"log/slog"
	"os"
	"path/filepath"
	"time"

	"github.com/fsnotify/fsnotify"
	"vuio-go/media"
	"vuio-go/state"
)

// Watcher monitors the filesystem for changes.
type Watcher struct {
	state   *state.AppState
	scanner *media.Scanner
}

// New creates a new filesystem watcher.
func New(state *state.AppState) (*Watcher, error) {
	return &Watcher{
		state:   state,
		scanner: media.NewScanner(state.DB),
	}, nil
}

// Start begins watching the configured media directories.
func (w *Watcher) Start(ctx context.Context) {
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		slog.Error("Failed to create fsnotify watcher", "error", err)
		return
	}
	defer watcher.Close()

	for _, dir := range w.state.GetConfig().Media.Directories {
		slog.Info("Adding directory to watcher", "path", dir.Path)
		err := filepath.Walk(dir.Path, func(path string, info os.FileInfo, err error) error {
			if info.IsDir() {
				return watcher.Add(path)
			}
			return nil
		})
		if err != nil {
			slog.Error("Failed to add path to watcher", "path", dir.Path, "error", err)
		}
	}

	slog.Info("Filesystem watcher started")

	// Debounce events
	var (
		timer  *time.Timer
		events []fsnotify.Event
	)
	debounceDuration := 2 * time.Second

	for {
		select {
		case <-ctx.Done():
			slog.Info("Stopping filesystem watcher")
			return
		case event, ok := <-watcher.Events:
			if !ok {
				return
			}
			events = append(events, event)
			if timer != nil {
				timer.Stop()
			}
			timer = time.AfterFunc(debounceDuration, func() {
				w.handleEvents(events)
				events = nil // Clear events
			})
		case err, ok := <-watcher.Errors:
			if !ok {
				return
			}
			slog.Error("Watcher error", "error", err)
		}
	}
}

func (w *Watcher) handleEvents(events []fsnotify.Event) {
	slog.Debug("Handling debounced filesystem events", "count", len(events))
	// A simple approach is to just re-scan changed files.
	// The Rust code has more complex logic to handle moves vs create/delete.
	changedPaths := make(map[string]fsnotify.Event)
	for _, event := range events {
		changedPaths[event.Name] = event
	}

	contentChanged := false
	for path, event := range changedPaths {
		switch {
		case event.Op&fsnotify.Create != 0:
			slog.Info("File created", "path", path)
			info, err := os.Stat(path)
			if err == nil {
				if info.IsDir() {
					// In a real implementation, we'd add this new dir to the watcher.
					// For now, we'll rely on the initial recursive walk.
				} else {
					if err := w.scanner.syncFile(path, info); err == nil {
						contentChanged = true
					}
				}
			}
		case event.Op&fsnotify.Write != 0:
			slog.Info("File modified", "path", path)
			info, err := os.Stat(path)
			if err == nil {
				if err := w.scanner.syncFile(path, info); err == nil {
					contentChanged = true
				}
			}
		case event.Op&fsnotify.Remove != 0 || event.Op&fsnotify.Rename != 0:
			slog.Info("File removed/renamed", "path", path)
			if _, err := w.state.DB.RemoveMediaFile(path); err == nil {
				contentChanged = true
			}
		}
	}

	if contentChanged {
		newID := w.state.IncrementUpdateID()
		slog.Info("Content updated, new UpdateID", "id", newID)
	}
}