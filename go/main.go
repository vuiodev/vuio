package main

import (
	"context"
	"flag"
	"log/slog"
	"os"
	"os/signal"
	"sync"
	"syscall"
	"time"

	"vuio-go/config"
	"vuio-go/database"
	"vuio-go/logging"
	"vuio-go/media"
	"vuio-go/platform"
	"vuio-go/ssdp"
	"vuio-go/state"
	"vuio-go/watcher"
	"vuio-go/web"
)

func main() {
	// Early parse of CLI flags for logging setup
	debug := flag.Bool("debug", false, "Enable debug logging")
	configPath := flag.String("config", "", "Path to configuration file")
	flag.Parse()

	logging.Init(*debug)

	slog.Info("Starting VuIO Server...")

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Handle shutdown signals
	shutdownChan := make(chan os.Signal, 1)
	signal.Notify(shutdownChan, syscall.SIGINT, syscall.SIGTERM)
	go func() {
		sig := <-shutdownChan
		slog.Info("Received shutdown signal", "signal", sig)
		cancel()
		// Second signal forces exit
		<-shutdownChan
		slog.Warn("Received second signal, forcing exit")
		os.Exit(1)
	}()

	// Detect platform information
	plat, err := platform.Detect()
	if err != nil {
		slog.Error("Failed to detect platform information", "error", err)
		os.Exit(1)
	}
	slog.Info("Platform detected", "os", plat.OS, "arch", plat.Arch)

	// Initialize configuration
	cfg, err := config.Initialize(*configPath, flag.Args())
	if err != nil {
		slog.Error("Failed to initialize configuration", "error", err)
		os.Exit(1)
	}

	// Initialize database
	db, err := database.NewSqliteDatabase(cfg.GetDatabasePath())
	if err != nil {
		slog.Error("Failed to initialize database", "error", err)
		os.Exit(1)
	}
	if err := db.Initialize(); err != nil {
		slog.Error("Failed to run database migrations", "error", err)
		os.Exit(1)
	}

	// Create shared application state
	appState := state.New(cfg, db, plat)

	// Perform initial media scan
	if cfg.Media.ScanOnStartup {
		slog.Info("Performing initial media scan...")
		scanner := media.NewScanner(db)
		if err := scanner.ScanAllDirectories(cfg); err != nil {
			slog.Error("Initial media scan failed", "error", err)
			// Don't exit, server can still run
		}
	} else {
		slog.Info("Skipping media scan on startup as configured")
	}

	var wg sync.WaitGroup

	// Start file system watcher
	if cfg.Media.WatchForChanges {
		fileWatcher, err := watcher.New(appState)
		if err != nil {
			slog.Error("Failed to initialize file watcher", "error", err)
		} else {
			wg.Add(1)
			go func() {
				defer wg.Done()
				fileWatcher.Start(ctx)
			}()
		}
	}

	// Start SSDP service
	ssdpService, err := ssdp.New(appState)
	if err != nil {
		slog.Error("Failed to create SSDP service", "error", err)
		os.Exit(1)
	}
	wg.Add(1)
	go func() {
		defer wg.Done()
		ssdpService.Start(ctx)
	}()

	// Start web server
	server := web.NewServer(appState)
	wg.Add(1)
	go func() {
		defer wg.Done()
		if err := server.Start(ctx); err != nil {
			slog.Error("Web server failed", "error", err)
			cancel() // shutdown other services if web server fails to start
		}
	}()

	// Wait for shutdown signal
	<-ctx.Done()

	slog.Info("Shutting down services...")

	// Create a context with a timeout for graceful shutdown
	shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer shutdownCancel()

	// This part is a bit tricky as the web server shutdown is blocking
	// and we might want to wait for other goroutines too.
	// For simplicity, we just trigger the web server shutdown.
	if err := server.Shutdown(shutdownCtx); err != nil {
		slog.Error("Error during web server shutdown", "error", err)
	}

	// Wait for all goroutines to finish
	wg.Wait()
	slog.Info("Shutdown complete.")
}