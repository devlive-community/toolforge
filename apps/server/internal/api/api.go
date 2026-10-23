package api

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"log/slog"
	"mime"
	"net/http"
	"strconv"
	"strings"
	"time"

	"toolforge.dev/plugin-stats/internal/asset"
	"toolforge.dev/plugin-stats/internal/catalog"
	"toolforge.dev/plugin-stats/internal/dashboard"
	"toolforge.dev/plugin-stats/internal/receipt"
	"toolforge.dev/plugin-stats/internal/registry"
	"toolforge.dev/plugin-stats/internal/store"
)

type Repository interface {
	Ready(context.Context) error
	Issue(context.Context, string, string, string, string, time.Time) (store.Attempt, error)
	Confirm(context.Context, receipt.Claims, string, uint64, time.Time) error
	Version(context.Context, string, string) (store.VersionResult, error)
	Plugin(context.Context, string, int, int) (store.PluginResult, error)
	Batch(context.Context, []string) (store.Stats, error)
	Total(context.Context) (store.TotalResult, error)
	Dashboard(context.Context) (store.DashboardResult, error)
}
type API struct {
	Store        Repository
	Signer       *receipt.Signer
	AssetBaseURL string
	// Presigner signs COS download URLs for private buckets; when nil the
	// plain asset URL is served and the bucket must allow anonymous reads.
	Presigner *asset.Presigner
	// Registry supplies plugin names for the dashboard; nil shows IDs only.
	Registry *registry.Cache
	BuildSHA string
	Now      func() time.Time
	Logger   *slog.Logger
}

func (a *API) Handler() http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("GET /healthz", func(w http.ResponseWriter, r *http.Request) {
		writeJSON(w, 200, map[string]string{"status": "ok", "buildSha": a.BuildSHA})
	})
	mux.HandleFunc("GET /readyz", func(w http.ResponseWriter, r *http.Request) {
		if a.Store.Ready(r.Context()) != nil {
			writeJSON(w, 503, map[string]string{"status": "unavailable", "buildSha": a.BuildSHA})
			return
		}
		catalogReady := false
		if repo, ok := a.Store.(interface{ CatalogReady(context.Context) bool }); ok {
			catalogReady = repo.CatalogReady(r.Context())
		}
		writeJSON(w, 200, map[string]any{"status": "ready", "buildSha": a.BuildSHA, "catalogReady": catalogReady})
	})
	mux.HandleFunc("GET /v1/plugins/{id}/versions/{version}/download", a.download)
	mux.HandleFunc("POST /v1/downloads/{attempt}/complete", a.complete)
	mux.HandleFunc("GET /v1/plugins/{id}/versions/{version}/stats", a.version)
	mux.HandleFunc("GET /v1/plugins/{id}/stats", a.plugin)
	mux.HandleFunc("GET /v1/stats", a.batch)
	mux.HandleFunc("GET /v1/stats/total", a.total)
	mux.HandleFunc("GET /v1/dashboard", a.dashboard)
	mux.HandleFunc("GET /{$}", dashboard.Index)
	mux.HandleFunc("GET /assets/{file}", dashboard.Asset)
	inFlight := make(chan struct{}, 128)
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Cache-Control", "no-store")
		w.Header().Set("X-Content-Type-Options", "nosniff")
		w.Header().Set("Referrer-Policy", "no-referrer")
		// Do not log URL queries, headers, bodies, tokens, or database errors.
		defer func() {
			if recover() != nil {
				if a.Logger != nil {
					a.Logger.Error("request panic")
				}
				writeError(w, 500, "internal_error")
			}
		}()
		if len(r.URL.RawQuery) > 8192 {
			writeError(w, 414, "query_too_long")
			return
		}
		select {
		case inFlight <- struct{}{}:
			defer func() { <-inFlight }()
		default:
			w.Header().Set("Retry-After", "1")
			writeError(w, 503, "busy")
			return
		}
		ctx, cancel := context.WithTimeout(r.Context(), 8*time.Second)
		defer cancel()
		mux.ServeHTTP(w, r.WithContext(ctx))
	})
}
func (a *API) now() time.Time {
	if a.Now != nil {
		return a.Now().UTC()
	}
	return time.Now().UTC()
}
func validPath(r *http.Request) bool {
	return catalog.IDPattern.MatchString(r.PathValue("id")) && catalog.VersionPattern.MatchString(r.PathValue("version"))
}
func (a *API) download(w http.ResponseWriter, r *http.Request) {
	// HEAD probes must not create attempts. Older clients without the new header
	// still get a redirect, but only updated clients can confirm success.
	if r.Method != http.MethodGet {
		w.Header().Set("Allow", "GET")
		writeError(w, 405, "method_not_allowed")
		return
	}
	if !validPath(r) {
		writeError(w, 400, "invalid_plugin_version")
		return
	}
	attempt := r.Header.Get("Idempotency-Key")
	if attempt == "" {
		var b [16]byte
		if _, err := rand.Read(b[:]); err != nil {
			a.fail(w, err)
			return
		}
		attempt = hex.EncodeToString(b[:])
	}
	if !catalog.AttemptPattern.MatchString(attempt) {
		writeError(w, 400, "invalid_idempotency_key")
		return
	}
	issued, err := a.Store.Issue(r.Context(), r.PathValue("id"), r.PathValue("version"), attempt, a.Signer.ActiveKid(), a.now())
	if err != nil {
		a.fail(w, err)
		return
	}
	token, err := a.Signer.Sign(issued.Claims)
	if err != nil {
		a.fail(w, err)
		return
	}
	location := strings.TrimRight(a.AssetBaseURL, "/") + "/" + issued.ObjectKey
	if a.Presigner != nil {
		location = a.Presigner.Presign(a.AssetBaseURL, issued.ObjectKey)
	}
	w.Header().Set("Location", location)
	w.Header().Set("X-ToolForge-Attempt-Id", issued.AttemptID)
	w.Header().Set("X-ToolForge-Receipt", token)
	w.Header().Set("X-ToolForge-Receipt-Expires", strconv.FormatInt(issued.ExpiresAt, 10))
	w.WriteHeader(http.StatusFound)
}
func (a *API) complete(w http.ResponseWriter, r *http.Request) {
	attempt := r.PathValue("attempt")
	if !catalog.AttemptPattern.MatchString(attempt) {
		writeError(w, 400, "invalid_attempt")
		return
	}
	contentType, _, contentTypeErr := mime.ParseMediaType(r.Header.Get("Content-Type"))
	if contentTypeErr != nil || contentType != "application/json" {
		writeError(w, 415, "application_json_required")
		return
	}
	auth := r.Header.Get("Authorization")
	if !strings.HasPrefix(auth, "Bearer ") {
		writeError(w, 401, "invalid_receipt")
		return
	}
	claims, err := a.Signer.Verify(strings.TrimPrefix(auth, "Bearer "), a.now())
	if err != nil {
		if errors.Is(err, receipt.ErrExpired) {
			writeError(w, 410, "receipt_expired")
		} else {
			writeError(w, 401, "invalid_receipt")
		}
		return
	}
	if claims.AttemptID != attempt {
		writeError(w, 409, "attempt_mismatch")
		return
	}
	var body struct {
		SHA256 string `json:"sha256"`
		Size   uint64 `json:"size"`
	}
	dec := json.NewDecoder(http.MaxBytesReader(w, r.Body, 4096))
	dec.DisallowUnknownFields()
	if dec.Decode(&body) != nil || dec.Decode(new(any)) != io.EOF || !catalog.SHAPattern.MatchString(body.SHA256) || body.Size == 0 || body.Size > catalog.MaxPackageSize {
		writeError(w, 400, "invalid_body")
		return
	}
	if err := a.Store.Confirm(r.Context(), claims, body.SHA256, body.Size, a.now()); err != nil {
		a.fail(w, err)
		return
	}
	writeJSON(w, 200, map[string]string{"attemptId": attempt, "status": "confirmed"})
}
func (a *API) version(w http.ResponseWriter, r *http.Request) {
	if !validPath(r) {
		writeError(w, 400, "invalid_plugin_version")
		return
	}
	out, err := a.Store.Version(r.Context(), r.PathValue("id"), r.PathValue("version"))
	if err != nil {
		a.fail(w, err)
		return
	}
	cacheJSON(w, out)
}
func (a *API) plugin(w http.ResponseWriter, r *http.Request) {
	if !catalog.IDPattern.MatchString(r.PathValue("id")) {
		writeError(w, 400, "invalid_plugin")
		return
	}
	limit, ok := intQuery(r, "limit", 50, 1, 100)
	if !ok {
		writeError(w, 400, "invalid_limit")
		return
	}
	offset, ok := intQuery(r, "offset", 0, 0, 100000)
	if !ok {
		writeError(w, 400, "invalid_offset")
		return
	}
	out, err := a.Store.Plugin(r.Context(), r.PathValue("id"), limit, offset)
	if err != nil {
		a.fail(w, err)
		return
	}
	cacheJSON(w, out)
}
func intQuery(r *http.Request, key string, fallback, min, max int) (int, bool) {
	v := r.URL.Query().Get(key)
	if v == "" {
		return fallback, true
	}
	n, err := strconv.Atoi(v)
	return n, err == nil && n >= min && n <= max
}
func (a *API) batch(w http.ResponseWriter, r *http.Request) {
	ids := strings.Split(r.URL.Query().Get("ids"), ",")
	if len(ids) > 100 {
		writeError(w, 400, "too_many_plugins")
		return
	}
	seen := make(map[string]bool, len(ids))
	unique := make([]string, 0, len(ids))
	for _, id := range ids {
		if !catalog.IDPattern.MatchString(id) {
			writeError(w, 400, "invalid_plugin")
			return
		}
		if !seen[id] {
			unique = append(unique, id)
			seen[id] = true
		}
	}
	out, err := a.Store.Batch(r.Context(), unique)
	if err != nil {
		a.fail(w, err)
		return
	}
	cacheJSON(w, out)
}
func (a *API) total(w http.ResponseWriter, r *http.Request) {
	out, err := a.Store.Total(r.Context())
	if err != nil {
		a.fail(w, err)
		return
	}
	cacheJSON(w, out)
}

// DashboardResponse adds display names from the plugin index to the aggregates.
type DashboardResponse struct {
	store.DashboardResult
	PluginInfo map[string]registry.Plugin `json:"pluginInfo"`
}

func (a *API) dashboard(w http.ResponseWriter, r *http.Request) {
	out, err := a.Store.Dashboard(r.Context())
	if err != nil {
		a.fail(w, err)
		return
	}
	ids := make([]string, 0, len(out.Ranking)+len(out.TopVersions)+len(out.Recent))
	for _, p := range out.Ranking {
		ids = append(ids, p.PluginID)
	}
	for _, v := range out.TopVersions {
		ids = append(ids, v.PluginID)
	}
	for _, d := range out.Recent {
		ids = append(ids, d.PluginID)
	}
	cacheJSON(w, DashboardResponse{DashboardResult: out, PluginInfo: a.Registry.Lookup(ids)})
}

func (a *API) fail(w http.ResponseWriter, err error) {
	switch {
	case errors.Is(err, store.ErrNotFound):
		writeError(w, 404, "not_found")
	case errors.Is(err, store.ErrConflict):
		writeError(w, 409, "metadata_conflict")
	case errors.Is(err, store.ErrGone), errors.Is(err, store.ErrRevoked):
		writeError(w, 410, "unavailable")
	default:
		if a.Logger != nil {
			a.Logger.Error("request failed", "category", "storage_or_signing")
		}
		writeError(w, 503, "temporarily_unavailable")
	}
}
func cacheJSON(w http.ResponseWriter, value any) {
	w.Header().Set("Cache-Control", "public, max-age=30")
	// These endpoints only expose aggregate download counts without credentials,
	// so any origin (including the static docs site) may read them.
	w.Header().Set("Access-Control-Allow-Origin", "*")
	writeJSON(w, 200, value)
}
func writeError(w http.ResponseWriter, status int, code string) {
	writeJSON(w, status, map[string]string{"error": code})
}
func writeJSON(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}
