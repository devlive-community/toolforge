package registry

import (
	"bytes"
	"context"
	"net/http"
	"net/http/httptest"
	"sync/atomic"
	"testing"
	"time"
)

func TestRefreshKeepsNamesAndSurvivesFailures(t *testing.T) {
	status := http.StatusOK
	body := `{"plugins":[{"id":"org.devlive.toolforge.hash","version":"0.1.2","category":"dev","accent":"orange","names":{"en-US":"Hash","zh-CN":"哈希计算"},"sha256":"x"}]}`
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(status)
		_, _ = w.Write([]byte(body))
	}))
	defer server.Close()
	c := &Cache{URL: server.URL, Client: server.Client()}
	if got := c.Lookup([]string{"org.devlive.toolforge.hash"}); len(got) != 0 {
		t.Fatal("empty cache returned names")
	}
	if err := c.Refresh(context.Background()); err != nil {
		t.Fatal(err)
	}
	got := c.Lookup([]string{"org.devlive.toolforge.hash", "unknown"})
	if len(got) != 1 || got["org.devlive.toolforge.hash"].Names["zh-CN"] != "哈希计算" || got["org.devlive.toolforge.hash"].Category != "dev" {
		t.Fatalf("lookup: %+v", got)
	}
	status = http.StatusBadGateway
	if err := c.Refresh(context.Background()); err == nil {
		t.Fatal("failed refresh reported success")
	}
	if len(c.Lookup([]string{"org.devlive.toolforge.hash"})) != 1 {
		t.Fatal("failed refresh dropped cached names")
	}
	status, body = http.StatusOK, `not json`
	if err := c.Refresh(context.Background()); err == nil {
		t.Fatal("invalid index accepted")
	}
	var nilCache *Cache
	if len(nilCache.Lookup([]string{"x"})) != 0 {
		t.Fatal("nil cache returned names")
	}
}

func TestRefreshRejectsOversizedIndex(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		_, _ = w.Write(bytes.Repeat([]byte("a"), MaxIndexBytes+1))
	}))
	defer server.Close()
	c := &Cache{URL: server.URL, Client: server.Client()}
	if err := c.Refresh(context.Background()); err == nil {
		t.Fatal("oversized index accepted")
	}
}

func TestRunRefreshesUntilContextEnds(t *testing.T) {
	var calls atomic.Int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		calls.Add(1)
		_, _ = w.Write([]byte(`{"plugins":[]}`))
	}))
	defer server.Close()
	c := &Cache{URL: server.URL, Client: server.Client()}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	done := make(chan struct{})
	go func() {
		// Refresh failures are reported, never fatal; the loop stops on ctx.
		c.Run(ctx, 15*time.Millisecond, func(error) { t.Error("refresh failed") })
		close(done)
	}()
	deadline := time.Now().Add(2 * time.Second)
	for calls.Load() < 2 && time.Now().Before(deadline) {
		time.Sleep(5 * time.Millisecond)
	}
	cancel()
	select {
	case <-done:
	case <-time.After(time.Second):
		t.Fatal("refresh loop ignored context cancellation")
	}
	if calls.Load() < 2 {
		t.Fatalf("refresh ran %d times", calls.Load())
	}
}
