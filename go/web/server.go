package web

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"net/http"
	"time"

	"vuio-go/state"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
)

// Server wraps the HTTP server.
type Server struct {
	state *state.AppState
	http  *http.Server
}

// NewServer creates a new web server.
func NewServer(state *state.AppState) *Server {
	return &Server{state: state}
}

// Start runs the web server.
func (s *Server) Start(ctx context.Context) error {
	cfg := s.state.GetConfig()
	addr := fmt.Sprintf("%s:%d", cfg.Server.Interface, cfg.Server.Port)
	slog.Info("Starting web server", "address", addr)

	s.http = &http.Server{
		Addr:    addr,
		Handler: s.router(),
	}

	errChan := make(chan error, 1)
	go func() {
		if err := s.http.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
			errChan <- err
		}
		close(errChan)
	}()

	select {
	case err := <-errChan:
		return err
	case <-ctx.Done():
		slog.Info("Web server context canceled, shutting down.")
		return s.Shutdown(context.Background())
	}
}

// Shutdown gracefully shuts down the server.
func (s *Server) Shutdown(ctx context.Context) error {
	slog.Info("Shutting down web server...")
	if s.http != nil {
		return s.http.Shutdown(ctx)
	}
	return nil
}

func (s *Server) router() http.Handler {
	r := chi.NewRouter()

	r.Use(middleware.RequestID)
	r.Use(middleware.RealIP)
	r.Use(middleware.Logger)
	r.Use(middleware.Recoverer)
	r.Use(middleware.Timeout(60 * time.Second))

	// Middleware to inject state
	r.Use(func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			ctx := context.WithValue(r.Context(), "state", s.state)
			next.ServeHTTP(w, r.WithContext(ctx))
		})
	})

	r.Get("/", rootHandler)
	r.Get("/description.xml", descriptionHandler)
	r.Get("/ContentDirectory.xml", contentDirectorySCPDHandler)
	r.Handle("/control/ContentDirectory", soapHandler(contentDirectoryControlHandler))
	r.Handle("/event/ContentDirectory", http.HandlerFunc(eventSubscribeHandler))
	r.Get("/media/{id}", serveMediaHandler)

	return r
}
