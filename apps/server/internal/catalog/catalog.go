// Package catalog validates the immutable official release catalog. Import is a
// local administrative operation, never an HTTP endpoint.
package catalog

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"path"
	"regexp"
	"strings"
	"time"
)

const MaxCatalogBytes = 16 << 20
const MaxPackages = 10000
const MaxPackageSize = 64 * 1024 * 1024

var IDPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._-]{0,190}$`)
var VersionPattern = regexp.MustCompile(`^[0-9A-Za-z][0-9A-Za-z.+-]{0,63}$`)
var SHAPattern = regexp.MustCompile(`^[0-9a-f]{64}$`)
var AttemptPattern = regexp.MustCompile(`^[0-9a-f]{32}$`)
var CommitPattern = regexp.MustCompile(`^[0-9a-f]{40}([0-9a-f]{24})?$`)

// Generation is a monotonically increasing release sequence, normally the
// GitHub run number. It prevents delayed workflows publishing an older catalog.
type Catalog struct {
	SchemaVersion int       `json:"schemaVersion"`
	SourceCommit  string    `json:"sourceCommit"`
	Generation    uint64    `json:"generation"`
	Plugins       []Package `json:"plugins"`
}
type Package struct {
	ID          string    `json:"id"`
	Version     string    `json:"version"`
	SHA256      string    `json:"sha256"`
	Size        uint64    `json:"size"`
	ObjectKey   string    `json:"objectKey"`
	Status      string    `json:"status"`
	PublishedAt time.Time `json:"publishedAt,omitempty"`
}

func Read(r io.Reader) (Catalog, string, error) {
	var c Catalog
	raw, err := io.ReadAll(io.LimitReader(r, MaxCatalogBytes+1))
	if err != nil || len(raw) > MaxCatalogBytes {
		return c, "", errors.New("catalog cannot be read or exceeds limit")
	}
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&c); err != nil {
		return c, "", errors.New("invalid catalog JSON")
	}
	if dec.Decode(new(any)) != io.EOF {
		return c, "", errors.New("unexpected trailing catalog data")
	}
	if err := c.Validate(); err != nil {
		return c, "", err
	}
	// Digest canonical JSON, so harmless whitespace changes remain idempotent.
	canonical, _ := json.Marshal(c)
	digest := sha256.Sum256(canonical)
	return c, hex.EncodeToString(digest[:]), nil
}

func (c Catalog) Validate() error {
	if c.SchemaVersion != 1 || !CommitPattern.MatchString(c.SourceCommit) || c.Generation == 0 {
		return errors.New("catalog requires schemaVersion 1, sourceCommit and positive generation")
	}
	if len(c.Plugins) == 0 || len(c.Plugins) > MaxPackages {
		return errors.New("catalog package count out of range")
	}
	seen := make(map[string]bool, len(c.Plugins))
	for _, p := range c.Plugins {
		if !IDPattern.MatchString(p.ID) || !VersionPattern.MatchString(p.Version) || !SHAPattern.MatchString(p.SHA256) || p.Size == 0 || p.Size > MaxPackageSize {
			return fmt.Errorf("invalid metadata for %q version %q", p.ID, p.Version)
		}
		if p.Status != "active" && p.Status != "withdrawn" && p.Status != "revoked" {
			return errors.New("invalid package status")
		}
		// This is a path, never a client-supplied URL. Prevent encoded traversal,
		// fragments, queries, backslashes, and escape outside the package prefix.
		expected := "packages/" + p.ID + "/" + p.Version + "/" + p.SHA256 + ".tfp"
		if p.ObjectKey != expected || path.Clean(p.ObjectKey) != p.ObjectKey || strings.ContainsAny(p.ObjectKey, "%?#\\") {
			return errors.New("objectKey must match immutable package path")
		}
		key := p.ID + "\x00" + p.Version
		if seen[key] {
			return errors.New("duplicate plugin version")
		}
		seen[key] = true
	}
	return nil
}
