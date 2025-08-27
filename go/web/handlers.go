package web

import (
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"os"
	"path/filepath" // Added for path manipulation
	"strconv"
	"strings"

	"vuio-go/database"
	"vuio-go/state"

	"github.com/go-chi/chi/v5"
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

		var subdirs []database.MediaDirectory
		var files []database.MediaFile
		var err error

		if params.ObjectID == "0" {
			// Root object ID. The response will contain the virtual directories.
			// No database query needed for this level.
			subdirs = []database.MediaDirectory{}
			files = []database.MediaFile{}
		} else {
			// Determine browse path and filter from object ID.
			// This is simplified and assumes a single media root. A more robust implementation
			// would map 'video', 'audio', 'image' roots to different configured directories.
			mediaRoot := state.Config.GetPrimaryMediaDir()
			var browsePath string
			var mediaTypeFilter string
			var subPath string

			if strings.HasPrefix(params.ObjectID, "video") {
				mediaTypeFilter = "video"
				subPath = strings.TrimPrefix(params.ObjectID, "video")
			} else if strings.HasPrefix(params.ObjectID, "audio") {
				mediaTypeFilter = "audio"
				subPath = strings.TrimPrefix(params.ObjectID, "audio")
			} else if strings.HasPrefix(params.ObjectID, "image") {
				mediaTypeFilter = "image"
				subPath = strings.TrimPrefix(params.ObjectID, "image")
			} else {
				// Fallback or handle other object IDs if necessary
				return "", fmt.Errorf("invalid or unhandled ObjectID: %s", params.ObjectID)
			}

			// Remove any leading slash from the subPath to ensure filepath.Join works correctly
			subPath = strings.TrimPrefix(subPath, "/")
			browsePath = filepath.Join(mediaRoot, subPath)

			subdirs, files, err = state.DB.GetDirectoryListing(browsePath, mediaTypeFilter)
		}

		if err != nil {
			return "", err
		}

		totalMatches := len(subdirs) + len(files)
		// For the root object, there are always 3 virtual containers.
		if params.ObjectID == "0" {
			totalMatches = 3
		}

		response := generateBrowseResponse(params.ObjectID, subdirs, files, totalMatches, state)
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
