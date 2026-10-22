package store

import (
	"context"
	"database/sql"
	"encoding/hex"
	"errors"
	"fmt"
	"sort"
	"strconv"
	"strings"
	"time"

	driver "github.com/go-sql-driver/mysql"
	"toolforge.dev/plugin-stats/internal/catalog"
	"toolforge.dev/plugin-stats/internal/receipt"
)

var (
	ErrNotFound = errors.New("not found")
	ErrConflict = errors.New("conflicting metadata")
	ErrGone     = errors.New("expired or withdrawn")
	ErrRevoked  = errors.New("revoked package")
)

const SchemaVersion = 1

type Store struct{ DB *sql.DB }
type Attempt struct {
	receipt.Claims
	ObjectKey string
	Confirmed bool
}
type VersionStats struct {
	Version   string `json:"version"`
	Downloads string `json:"downloads"`
}
type PluginStats struct {
	PluginID          string         `json:"pluginId"`
	TotalDownloads    string         `json:"totalDownloads"`
	Versions          []VersionStats `json:"versions"`
	VersionsTruncated bool           `json:"versionsTruncated,omitempty"`
}
type Stats struct {
	Plugins      []PluginStats `json:"plugins"`
	CountedSince time.Time     `json:"countedSince"`
	AsOf         time.Time     `json:"asOf"`
}
type VersionResult struct {
	PluginID       string    `json:"pluginId"`
	Version        string    `json:"version"`
	Downloads      string    `json:"downloads"`
	TotalDownloads string    `json:"totalDownloads"`
	CountedSince   time.Time `json:"countedSince"`
	AsOf           time.Time `json:"asOf"`
}
type PluginResult struct {
	PluginStats
	CountedSince time.Time `json:"countedSince"`
	AsOf         time.Time `json:"asOf"`
	Limit        int       `json:"limit"`
	Offset       int       `json:"offset"`
	HasMore      bool      `json:"hasMore"`
}
type TotalResult struct {
	TotalDownloads string    `json:"totalDownloads"`
	CountedSince   time.Time `json:"countedSince"`
	AsOf           time.Time `json:"asOf"`
}

func (s *Store) Ready(ctx context.Context) error {
	var v int
	var dirty bool
	if err := s.DB.QueryRowContext(ctx, "SELECT version,dirty FROM schema_migrations LIMIT 1").Scan(&v, &dirty); err != nil {
		return errors.New("schema unavailable")
	}
	if v != SchemaVersion || dirty {
		return errors.New("schema incompatible")
	}
	return nil
}

func (s *Store) CatalogReady(ctx context.Context) bool {
	var generation uint64
	return s.DB.QueryRowContext(ctx, "SELECT generation FROM catalog_state WHERE id=1").Scan(&generation) == nil && generation > 0
}

func (s *Store) Issue(ctx context.Context, id, version, attempt, kid string, now time.Time) (Attempt, error) {
	var out Attempt
	now = now.UTC().Truncate(time.Second)
	err := s.transaction(ctx, func(tx *sql.Tx) error {
		var p catalog.Package
		if err := tx.QueryRowContext(ctx, `SELECT plugin_id,version,LOWER(HEX(sha256)),size_bytes,object_key,status FROM plugin_versions WHERE plugin_id=? AND version=? FOR SHARE`, id, version).Scan(&p.ID, &p.Version, &p.SHA256, &p.Size, &p.ObjectKey, &p.Status); err != nil {
			return notFound(err)
		}
		if p.Status == "revoked" {
			return ErrRevoked
		}
		var a Attempt
		var issued, expires time.Time
		var status string
		err := tx.QueryRowContext(ctx, `SELECT LOWER(HEX(attempt_id)),plugin_id,version,LOWER(HEX(sha256)),size_bytes,receipt_kid,issued_at,expires_at,status FROM download_attempts WHERE attempt_id=UNHEX(?) FOR UPDATE`, attempt).Scan(&a.AttemptID, &a.PluginID, &a.Version, &a.SHA256, &a.Size, &a.Kid, &issued, &expires, &status)
		if err == nil {
			if a.PluginID != id || a.Version != version || a.SHA256 != p.SHA256 || a.Size != p.Size {
				return ErrConflict
			}
			if !expires.After(now) {
				return ErrGone
			}
			a.IssuedAt = issued.Unix()
			a.ExpiresAt = expires.Unix()
			a.ObjectKey = p.ObjectKey
			a.Confirmed = status == "confirmed"
			out = a
			return nil
		}
		if !errors.Is(err, sql.ErrNoRows) {
			return err
		}
		if p.Status != "active" {
			return ErrGone
		}
		out = Attempt{Claims: receipt.Claims{Kid: kid, AttemptID: attempt, PluginID: id, Version: version, SHA256: p.SHA256, Size: p.Size, IssuedAt: now.Unix(), ExpiresAt: now.Add(receipt.Lifetime).Unix()}, ObjectKey: p.ObjectKey}
		if _, err := tx.ExecContext(ctx, `INSERT INTO download_attempts (attempt_id,plugin_id,version,sha256,size_bytes,receipt_kid,issued_at,expires_at) VALUES (UNHEX(?),?,?,UNHEX(?),?,?,?,?)`, attempt, id, version, p.SHA256, p.Size, kid, now, now.Add(receipt.Lifetime)); err != nil {
			return err
		}
		return increment(ctx, tx, id, version, "attempts", now)
	})
	return out, err
}

func (s *Store) Confirm(ctx context.Context, c receipt.Claims, sha string, size uint64, now time.Time) error {
	if c.SHA256 != sha || c.Size != size {
		return ErrConflict
	}
	return s.transaction(ctx, func(tx *sql.Tx) error {
		var packageStatus string
		if err := tx.QueryRowContext(ctx, `SELECT status FROM plugin_versions WHERE plugin_id=? AND version=? FOR SHARE`, c.PluginID, c.Version).Scan(&packageStatus); err != nil {
			return notFound(err)
		}
		if packageStatus == "revoked" {
			return ErrRevoked
		}
		var a Attempt
		var issued, expires time.Time
		var status string
		err := tx.QueryRowContext(ctx, `SELECT LOWER(HEX(attempt_id)),plugin_id,version,LOWER(HEX(sha256)),size_bytes,receipt_kid,issued_at,expires_at,status FROM download_attempts WHERE attempt_id=UNHEX(?) FOR UPDATE`, c.AttemptID).Scan(&a.AttemptID, &a.PluginID, &a.Version, &a.SHA256, &a.Size, &a.Kid, &issued, &expires, &status)
		if err != nil {
			return notFound(err)
		}
		a.IssuedAt = issued.Unix()
		a.ExpiresAt = expires.Unix()
		if a.Claims != c {
			return ErrConflict
		}
		if !expires.After(now) {
			return ErrGone
		}
		if status == "confirmed" {
			return nil
		}
		if _, err := tx.ExecContext(ctx, `UPDATE download_attempts SET status='confirmed',confirmed_at=? WHERE attempt_id=UNHEX(?)`, now.UTC(), c.AttemptID); err != nil {
			return err
		}
		return increment(ctx, tx, c.PluginID, c.Version, "downloads", now.UTC())
	})
}

// Both totals always change in the same transaction and lock order.
func increment(ctx context.Context, tx *sql.Tx, id, version, column string, now time.Time) error {
	if column != "attempts" && column != "downloads" {
		return errors.New("invalid counter")
	}
	r, err := tx.ExecContext(ctx, `UPDATE plugin_stats SET `+column+`=`+column+`+1,updated_at=? WHERE plugin_id=?`, now, id)
	if err != nil {
		return err
	}
	n, err := r.RowsAffected()
	if err != nil {
		return err
	}
	if n != 1 {
		return errors.New("missing plugin counter")
	}
	r, err = tx.ExecContext(ctx, `UPDATE plugin_version_stats SET `+column+`=`+column+`+1,updated_at=? WHERE plugin_id=? AND version=?`, now, id, version)
	if err != nil {
		return err
	}
	n, err = r.RowsAffected()
	if err != nil {
		return err
	}
	if n != 1 {
		return errors.New("missing version counter")
	}
	return nil
}

func (s *Store) Import(ctx context.Context, c catalog.Catalog, digest string, now time.Time) error {
	if err := c.Validate(); err != nil {
		return err
	}
	if !catalog.SHAPattern.MatchString(digest) {
		return ErrConflict
	}
	packages := append([]catalog.Package(nil), c.Plugins...)
	sort.Slice(packages, func(i, j int) bool {
		if packages[i].ID == packages[j].ID {
			return packages[i].Version < packages[j].Version
		}
		return packages[i].ID < packages[j].ID
	})
	return s.transaction(ctx, func(tx *sql.Tx) error {
		var priorDigest string
		var generation uint64
		if err := tx.QueryRowContext(ctx, `SELECT LOWER(HEX(digest)),generation FROM catalog_state WHERE id=1 FOR UPDATE`).Scan(&priorDigest, &generation); err != nil {
			return err
		}
		if priorDigest == digest {
			return nil
		}
		if c.Generation <= generation {
			return ErrConflict
		}
		// Absence in a complete catalog withdraws a version, preserving history and
		// allowing already issued receipts. Explicit revocations remain revoked.
		if _, err := tx.ExecContext(ctx, `UPDATE plugin_versions SET status='withdrawn' WHERE status='active'`); err != nil {
			return err
		}
		for _, p := range packages {
			var sha, key, priorStatus string
			var size uint64
			err := tx.QueryRowContext(ctx, `SELECT LOWER(HEX(sha256)),size_bytes,object_key,status FROM plugin_versions WHERE plugin_id=? AND version=? FOR UPDATE`, p.ID, p.Version).Scan(&sha, &size, &key, &priorStatus)
			if err == nil {
				if sha != p.SHA256 || size != p.Size || key != p.ObjectKey || (priorStatus == "revoked" && p.Status != "revoked") {
					return ErrConflict
				}
				if _, err = tx.ExecContext(ctx, `UPDATE plugin_versions SET status=? WHERE plugin_id=? AND version=?`, p.Status, p.ID, p.Version); err != nil {
					return err
				}
			} else if errors.Is(err, sql.ErrNoRows) {
				published := p.PublishedAt.UTC()
				if published.IsZero() {
					published = now.UTC()
				}
				if _, err = tx.ExecContext(ctx, `INSERT INTO plugin_versions (plugin_id,version,sha256,size_bytes,object_key,status,published_at) VALUES (?,?,UNHEX(?),?,?,?,?)`, p.ID, p.Version, p.SHA256, p.Size, p.ObjectKey, p.Status, published); err != nil {
					return err
				}
			} else {
				return err
			}
			if _, err := tx.ExecContext(ctx, `INSERT IGNORE INTO plugin_stats (plugin_id,updated_at) VALUES (?,?)`, p.ID, now.UTC()); err != nil {
				return err
			}
			if _, err := tx.ExecContext(ctx, `INSERT IGNORE INTO plugin_version_stats (plugin_id,version,updated_at) VALUES (?,?,?)`, p.ID, p.Version, now.UTC()); err != nil {
				return err
			}
		}
		_, err := tx.ExecContext(ctx, `UPDATE catalog_state SET digest=UNHEX(?),source_commit=?,generation=?,imported_at=? WHERE id=1`, digest, c.SourceCommit, c.Generation, now.UTC())
		return err
	})
}

func (s *Store) transaction(ctx context.Context, fn func(*sql.Tx) error) error {
	for i := 0; i < 4; i++ {
		tx, err := s.DB.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelReadCommitted})
		if err != nil {
			return err
		}
		err = fn(tx)
		if err == nil {
			err = tx.Commit()
		} else {
			_ = tx.Rollback()
		}
		if err == nil {
			return nil
		}
		_ = tx.Rollback()
		var me *driver.MySQLError
		if !errors.As(err, &me) || (me.Number != 1213 && me.Number != 1205 && me.Number != 1062) {
			return err
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-time.After(time.Duration(i+1) * 10 * time.Millisecond):
		}
	}
	return errors.New("database transaction retry limit reached")
}
func notFound(err error) error {
	if errors.Is(err, sql.ErrNoRows) {
		return ErrNotFound
	}
	return err
}

func (s *Store) read(ctx context.Context, fn func(*sql.Tx, time.Time, time.Time) error) error {
	tx, err := s.DB.BeginTx(ctx, &sql.TxOptions{ReadOnly: true, Isolation: sql.LevelRepeatableRead})
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	var since, asof time.Time
	if err = tx.QueryRowContext(ctx, `SELECT counted_since,UTC_TIMESTAMP(6) FROM catalog_state WHERE id=1`).Scan(&since, &asof); err != nil {
		return err
	}
	if err = fn(tx, since.UTC(), asof.UTC()); err != nil {
		return err
	}
	return tx.Commit()
}
func (s *Store) Version(ctx context.Context, id, version string) (VersionResult, error) {
	out := VersionResult{PluginID: id, Version: version}
	err := s.read(ctx, func(tx *sql.Tx, since, asof time.Time) error {
		var n, total uint64
		err := tx.QueryRowContext(ctx, `SELECT v.downloads,p.downloads FROM plugin_version_stats v JOIN plugin_stats p USING(plugin_id) WHERE v.plugin_id=? AND v.version=?`, id, version).Scan(&n, &total)
		if err != nil {
			return notFound(err)
		}
		out.Downloads = strconv.FormatUint(n, 10)
		out.TotalDownloads = strconv.FormatUint(total, 10)
		out.CountedSince = since
		out.AsOf = asof
		return nil
	})
	return out, err
}
func (s *Store) Plugin(ctx context.Context, id string, limit, offset int) (PluginResult, error) {
	out := PluginResult{PluginStats: PluginStats{PluginID: id, Versions: []VersionStats{}}, Limit: limit, Offset: offset}
	err := s.read(ctx, func(tx *sql.Tx, since, asof time.Time) error {
		var n uint64
		if err := tx.QueryRowContext(ctx, `SELECT downloads FROM plugin_stats WHERE plugin_id=?`, id).Scan(&n); err != nil {
			return notFound(err)
		}
		out.TotalDownloads = strconv.FormatUint(n, 10)
		out.CountedSince = since
		out.AsOf = asof
		rows, err := tx.QueryContext(ctx, `SELECT v.version,s.downloads FROM plugin_versions v JOIN plugin_version_stats s USING(plugin_id,version) WHERE v.plugin_id=? ORDER BY v.published_at DESC,v.version DESC LIMIT ? OFFSET ?`, id, limit+1, offset)
		if err != nil {
			return err
		}
		defer rows.Close()
		for rows.Next() {
			var v VersionStats
			var count uint64
			if err := rows.Scan(&v.Version, &count); err != nil {
				return err
			}
			v.Downloads = strconv.FormatUint(count, 10)
			if len(out.Versions) < limit {
				out.Versions = append(out.Versions, v)
			} else {
				out.HasMore = true
			}
		}
		return rows.Err()
	})
	return out, err
}
func (s *Store) Batch(ctx context.Context, ids []string) (Stats, error) {
	out := Stats{Plugins: []PluginStats{}}
	if len(ids) == 0 || len(ids) > 100 {
		return out, ErrConflict
	}
	args := make([]any, len(ids))
	for i, id := range ids {
		args[i] = id
	}
	placeholders := strings.TrimSuffix(strings.Repeat("?,", len(ids)), ",")
	err := s.read(ctx, func(tx *sql.Tx, since, asof time.Time) error {
		out.CountedSince = since
		out.AsOf = asof
		rows, err := tx.QueryContext(ctx, `SELECT plugin_id,downloads FROM plugin_stats WHERE plugin_id IN (`+placeholders+`)`, args...)
		if err != nil {
			return err
		}
		m := make(map[string]*PluginStats, len(ids))
		for rows.Next() {
			var p PluginStats
			var n uint64
			if err := rows.Scan(&p.PluginID, &n); err != nil {
				rows.Close()
				return err
			}
			p.TotalDownloads = strconv.FormatUint(n, 10)
			p.Versions = []VersionStats{}
			m[p.PluginID] = &p
		}
		err = rows.Err()
		_ = rows.Close()
		if err != nil {
			return err
		}
		rows, err = tx.QueryContext(ctx, `WITH ranked AS (SELECT v.plugin_id,v.version,s.downloads,ROW_NUMBER() OVER (PARTITION BY v.plugin_id ORDER BY v.published_at DESC,v.version DESC) AS rn FROM plugin_versions v JOIN plugin_version_stats s USING(plugin_id,version) WHERE v.plugin_id IN (`+placeholders+`)) SELECT plugin_id,version,downloads,rn FROM ranked WHERE rn<=101 ORDER BY plugin_id,rn`, args...)
		if err != nil {
			return err
		}
		defer rows.Close()
		for rows.Next() {
			var id string
			var v VersionStats
			var n, rn uint64
			if err := rows.Scan(&id, &v.Version, &n, &rn); err != nil {
				return err
			}
			if p := m[id]; p != nil {
				if rn > 100 {
					p.VersionsTruncated = true
				} else {
					v.Downloads = strconv.FormatUint(n, 10)
					p.Versions = append(p.Versions, v)
				}
			}
		}
		if err := rows.Err(); err != nil {
			return err
		}
		// Unknown IDs are omitted, never invented as zero-download official plugins.
		for _, id := range ids {
			if p := m[id]; p != nil {
				out.Plugins = append(out.Plugins, *p)
			}
		}
		return nil
	})
	return out, err
}
func (s *Store) Total(ctx context.Context) (TotalResult, error) {
	var out TotalResult
	err := s.read(ctx, func(tx *sql.Tx, since, asof time.Time) error {
		// SUM of BIGINT may exceed uint64; MySQL DECIMAL is safely read as text.
		if err := tx.QueryRowContext(ctx, `SELECT CAST(COALESCE(SUM(downloads),0) AS CHAR) FROM plugin_stats`).Scan(&out.TotalDownloads); err != nil {
			return err
		}
		out.CountedSince = since
		out.AsOf = asof
		return nil
	})
	return out, err
}

// Cleanup only deletes bounded batches of expired attempts. Confirmed attempts
// live at least 30 days. Aggregate counts and catalog history are never deleted.
func (s *Store) Cleanup(ctx context.Context, now time.Time) error {
	_, err := s.DB.ExecContext(ctx, `DELETE FROM download_attempts WHERE expires_at<? AND (confirmed_at IS NULL OR confirmed_at<?) LIMIT 1000`, now.UTC(), now.UTC().Add(-30*24*time.Hour))
	return err
}

// DigestBytes is useful to tests and migration tools that compare canonical hashes.
func DigestBytes(sha string) ([]byte, error) {
	b, err := hex.DecodeString(sha)
	if err != nil || len(b) != 32 {
		return nil, fmt.Errorf("invalid digest")
	}
	return b, nil
}
