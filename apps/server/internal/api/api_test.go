package api

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"toolforge.dev/plugin-stats/internal/receipt"
	"toolforge.dev/plugin-stats/internal/store"
)

type fakeRepo struct {
	issued    int
	confirmed int
	err       error
	last      receipt.Claims
}

func (f *fakeRepo) Ready(context.Context) error { return f.err }
func (f *fakeRepo) Issue(_ context.Context, id, ver, attempt, kid string, now time.Time) (store.Attempt, error) {
	f.issued++
	f.last = receipt.Claims{Kid: kid, AttemptID: attempt, PluginID: id, Version: ver, SHA256: strings.Repeat("a", 64), Size: 42, IssuedAt: now.Unix(), ExpiresAt: now.Add(receipt.Lifetime).Unix()}
	return store.Attempt{Claims: f.last, ObjectKey: "packages/plugin/1.0.0/file.tfp"}, f.err
}
func (f *fakeRepo) Confirm(_ context.Context, c receipt.Claims, sha string, size uint64, _ time.Time) error {
	if c.SHA256 != sha || c.Size != size {
		return store.ErrConflict
	}
	f.confirmed++
	return f.err
}
func (f *fakeRepo) Version(context.Context, string, string) (store.VersionResult, error) {
	return store.VersionResult{Downloads: "9007199254740993"}, f.err
}
func (f *fakeRepo) Plugin(context.Context, string, int, int) (store.PluginResult, error) {
	return store.PluginResult{}, f.err
}
func (f *fakeRepo) Batch(context.Context, []string) (store.Stats, error) {
	return store.Stats{Plugins: []store.PluginStats{}}, f.err
}
func (f *fakeRepo) Total(context.Context) (store.TotalResult, error) {
	return store.TotalResult{TotalDownloads: "9007199254740993"}, f.err
}
func fixtureAPI() (*API, *fakeRepo) {
	signer, _ := receipt.New("k1", map[string][]byte{"k1": bytes.Repeat([]byte{7}, 32)})
	repo := &fakeRepo{}
	return &API{Store: repo, Signer: signer, AssetBaseURL: "https://plugins.toolforge.devlive.org", BuildSHA: "test-sha", Now: func() time.Time { return time.Unix(1700000000, 0) }}, repo
}
func TestDownloadAndReceiptHTTPContract(t *testing.T) {
	a, repo := fixtureAPI()
	h := a.Handler()
	req := httptest.NewRequest("GET", "/v1/plugins/plugin/versions/1.0.0/download", nil)
	req.Header.Set("Idempotency-Key", strings.Repeat("c", 32))
	w := httptest.NewRecorder()
	h.ServeHTTP(w, req)
	if w.Code != 302 || w.Header().Get("Cache-Control") != "no-store" || w.Header().Get("Location") != "https://plugins.toolforge.devlive.org/packages/plugin/1.0.0/file.tfp" {
		t.Fatalf("download status/headers: %d %v", w.Code, w.Header())
	}
	if w.Header().Get("X-ToolForge-Attempt-Id") != strings.Repeat("c", 32) || w.Header().Get("X-ToolForge-Receipt-Expires") == "" {
		t.Fatal("missing attempt headers")
	}
	token := w.Header().Get("X-ToolForge-Receipt")
	complete := func(body, auth string) int {
		r := httptest.NewRequest("POST", "/v1/downloads/"+repo.last.AttemptID+"/complete", strings.NewReader(body))
		r.Header.Set("Content-Type", "application/json")
		r.Header.Set("Authorization", auth)
		rr := httptest.NewRecorder()
		h.ServeHTTP(rr, r)
		return rr.Code
	}
	body := `{"sha256":"` + strings.Repeat("a", 64) + `","size":42}`
	if code := complete(body, "Bearer "+token); code != 200 {
		t.Fatalf("complete status %d", code)
	}
	if repo.confirmed != 1 {
		t.Fatal("confirmation not stored")
	}
	if code := complete(body, "Bearer bad"); code != 401 {
		t.Fatalf("bad auth %d", code)
	}
	if code := complete(`{"sha256":"`+strings.Repeat("b", 64)+`","size":42}`, "Bearer "+token); code != 409 {
		t.Fatalf("bad hash %d", code)
	}
	if code := complete(body+"{}", "Bearer "+token); code != 400 {
		t.Fatalf("trailing data %d", code)
	}
	if code := complete(strings.Repeat(" ", 5000)+body, "Bearer "+token); code != 400 {
		t.Fatalf("oversized data %d", code)
	}
	a.Now = func() time.Time { return time.Unix(repo.last.ExpiresAt, 0) }
	if code := complete(body, "Bearer "+token); code != 410 {
		t.Fatalf("expired receipt %d", code)
	}
}
func TestLegacyRedirectHeadAndBadParameters(t *testing.T) {
	a, repo := fixtureAPI()
	h := a.Handler()
	r := httptest.NewRequest("GET", "/v1/plugins/plugin/versions/1.0.0/download", nil)
	w := httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != 302 || len(repo.last.AttemptID) != 32 {
		t.Fatal("legacy request did not redirect")
	}
	before := repo.issued
	r = httptest.NewRequest("HEAD", "/v1/plugins/plugin/versions/1.0.0/download", nil)
	w = httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != 405 || repo.issued != before {
		t.Fatal("HEAD created an attempt")
	}
	for _, url := range []string{"/v1/stats?ids=", "/v1/stats?ids=" + strings.Repeat("a,", 100) + "b", "/v1/plugins/plugin/stats?limit=101", "/v1/plugins/plugin/stats?offset=-1"} {
		w = httptest.NewRecorder()
		h.ServeHTTP(w, httptest.NewRequest("GET", url, nil))
		if w.Code != 400 {
			t.Fatalf("accepted %q: %d", url, w.Code)
		}
	}
	r = httptest.NewRequest("GET", "/v1/plugins/plugin/versions/1.0.0/download", nil)
	r.Header.Set("Idempotency-Key", "BAD")
	w = httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != 400 {
		t.Fatal("accepted bad key")
	}
}
func TestStatsCountPrecisionReadinessAndSafeErrors(t *testing.T) {
	a, repo := fixtureAPI()
	h := a.Handler()
	w := httptest.NewRecorder()
	h.ServeHTTP(w, httptest.NewRequest("GET", "/v1/stats/total", nil))
	var body map[string]any
	if json.Unmarshal(w.Body.Bytes(), &body) != nil || body["totalDownloads"] != "9007199254740993" {
		t.Fatal("count precision lost")
	}
	if w.Header().Get("Cache-Control") != "public, max-age=30" {
		t.Fatal("missing statistics cache")
	}
	repo.err = errors.New("password=must-not-leak")
	for _, path := range []string{"/readyz", "/v1/stats/total"} {
		w = httptest.NewRecorder()
		h.ServeHTTP(w, httptest.NewRequest("GET", path, nil))
		if w.Code != 503 || strings.Contains(w.Body.String(), "must-not-leak") {
			t.Fatal("unsafe failure response")
		}
	}
	repo.err = store.ErrNotFound
	w = httptest.NewRecorder()
	h.ServeHTTP(w, httptest.NewRequest(http.MethodGet, "/v1/plugins/unknown/stats", nil))
	if w.Code != 404 {
		t.Fatal("unknown plugin is not 404")
	}
}
