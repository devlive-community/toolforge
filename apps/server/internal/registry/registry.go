// Package registry caches plugin names from the public plugin index for the
// dashboard. The statistics database stores IDs only; names are display data
// and a failed refresh keeps the previous names (or none) without failing requests.
package registry

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"sync"
	"time"
)

// MaxIndexBytes bounds the downloaded index.
const MaxIndexBytes = 4 << 20

type Plugin struct {
	Names    map[string]string `json:"names"`
	Category string            `json:"category"`
	Accent   string            `json:"accent,omitempty"`
	Version  string            `json:"version"`
}

type Cache struct {
	URL    string
	Client *http.Client

	mu      sync.RWMutex
	plugins map[string]Plugin
}

// Lookup returns cached metadata for the given plugin IDs; unknown IDs are omitted.
func (c *Cache) Lookup(ids []string) map[string]Plugin {
	out := make(map[string]Plugin, len(ids))
	if c == nil {
		return out
	}
	c.mu.RLock()
	defer c.mu.RUnlock()
	for _, id := range ids {
		if p, ok := c.plugins[id]; ok {
			out[id] = p
		}
	}
	return out
}

// Refresh downloads and parses the index, replacing the cache only on success.
func (c *Cache) Refresh(ctx context.Context) error {
	ctx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.URL, nil)
	if err != nil {
		return err
	}
	req.Header.Set("User-Agent", "toolforge-plugin-stats")
	client := c.Client
	if client == nil {
		client = http.DefaultClient
	}
	resp, err := client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return errors.New("registry index unavailable")
	}
	var index struct {
		Plugins []struct {
			ID string `json:"id"`
			Plugin
		} `json:"plugins"`
	}
	if err := json.NewDecoder(io.LimitReader(resp.Body, MaxIndexBytes)).Decode(&index); err != nil {
		return errors.New("invalid registry index")
	}
	plugins := make(map[string]Plugin, len(index.Plugins))
	for _, entry := range index.Plugins {
		if entry.ID != "" {
			plugins[entry.ID] = entry.Plugin
		}
	}
	c.mu.Lock()
	c.plugins = plugins
	c.mu.Unlock()
	return nil
}

// Run refreshes immediately and then on every interval until ctx ends.
func (c *Cache) Run(ctx context.Context, interval time.Duration, onError func(error)) {
	refresh := func() {
		if err := c.Refresh(ctx); err != nil && onError != nil && ctx.Err() == nil {
			onError(err)
		}
	}
	refresh()
	ticker := time.NewTicker(interval)
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			refresh()
		}
	}
}
