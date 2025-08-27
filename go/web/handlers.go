package web

import (
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"os"
	"strconv"

	"github.com/go-chi/chi/v5"
	"vuio-go/state"
)

func getState(r *http.Request) *state.AppState {
	return r.Context().Value("state").(*state.AppState)
}

func rootHandler(w http.ResponseWriter, r *http.Request) {
	_, _ = w.Write([]byte("VuIO Media Server (Go version)"))
}

func descriptionHandler(w http.ResponseWriter, r *http.Request) {
	state := getState(r)
	xml := generateDescriptionXML(state)
	w.Header().Set("Content-Type", "text/xml; charset=utf-8")
	_, _ = w.Write([]byte(xml))
}

func contentDirectorySCPDHandler(w http.ResponseWriter, r *http.Request) {
	xml := generateSCPDXML()
	w.Header().Set("Content-Type", "text/xml; charset=utf-8")
	_, _ = w.Write([]byte(xml))
}

func soapHandler(next func(w http.ResponseWriter, r *http.Request) (string, error)) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		resultXML, err := next(w, r)
		if err != nil {
			slog.Error("SOAP handler error", "error", err)
			http.Error(w, "UPnP Error", http.StatusInternalServerError)
			return
		}
		w.Header().Set("Content-Type", "text/xml; charset=utf-8")
		w.Header().Set("EXT", "")
		_, _ = w.Write([]byte(resultXML))
	})
}

func contentDirectoryControlHandler(w http.ResponseWriter, r *http.Request) (string, error) {
	state := getState(r)
	body, err := io.ReadAll(r.Body)
	if err != nil {
		return "", err
	}

	bodyStr := string(body)
	if action, ok := getSOAPAction(bodyStr, "Browse"); ok {
		params := parseBrowseParams(action)
		slog.Info("Browse request", "ObjectID", params.ObjectID, "StartIndex", params.StartingIndex, "Count", params.RequestedCount)

		listing, err := state.DB.GetDirectoryListing(params.ObjectID, "")
		if err != nil {
			return "", err
		}
		
		totalMatches := len(listing.Subdirectories) + len(listing.Files)
		
		response := generateBrowseResponse(params.ObjectID, listing.Subdirectories, listing.Files, totalMatches, state)
		return response, nil
	}

	return "", fmt.Errorf("unsupported SOAP action")
}

func serveMediaHandler(w http.ResponseWriter, r *http.Request) {
	state := getState(r)
	idStr := chi.URLParam(r, "id")

	id, err := strconv.ParseInt(idStr, 10, 64)
	if err != nil {
		http.Error(w, "Invalid ID", http.StatusBadRequest)
		return
	}

	fileInfo, err := state.DB.GetFileByID(id)
	if err != nil {
		slog.Error("Error getting file from DB", "id", id, "error", err)
		http.Error(w, "Internal Server Error", http.StatusInternalServerError)
		return
	}
	if fileInfo == nil {
		http.NotFound(w, r)
		return
	}

	file, err := os.Open(fileInfo.Path)
	if err != nil {
		slog.Error("Failed to open media file", "path", fileInfo.Path, "error", err)
		http.NotFound(w, r)
		return
	}
	defer file.Close()

	w.Header().Set("Content-Type", fileInfo.MimeType)
	http.ServeContent(w, r, fileInfo.Filename, fileInfo.Modified, file)
}

func eventSubscribeHandler(w http.ResponseWriter, r *http.Request) {
	// This is a stub implementation. A real one would manage subscriptions.
	slog.Info("Received event subscription request", "method", r.Method, "callback", r.Header.Get("CALLBACK"))
	w.Header().Set("SID", "uuid:fake-subscription-id")
	w.Header().Set("TIMEOUT", "Second-1800")
	w.WriteHeader(http.StatusOK)
}