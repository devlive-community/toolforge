package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strings"
	"sync"
	"testing"
	"time"

	driver "github.com/go-sql-driver/mysql"
	"github.com/golang-migrate/migrate/v4"
	mysqlmigrate "github.com/golang-migrate/migrate/v4/database/mysql"
	"github.com/golang-migrate/migrate/v4/source/iofs"
	"toolforge.dev/plugin-stats/internal/catalog"
	"toolforge.dev/plugin-stats/migrations"
)

// The caller explicitly opts into destructive tests using a dedicated *_test
// database. Never point TEST_MYSQL_DSN at a development or production database.
func integrationStore(t *testing.T) *Store {
	t.Helper()
	dsn := os.Getenv("TEST_MYSQL_DSN")
	if dsn == "" {
		t.Skip("TEST_MYSQL_DSN not configured; MySQL integration tests skipped")
	}
	cfg, err := driver.ParseDSN(dsn)
	if err != nil || !strings.HasSuffix(cfg.DBName, "_test") {
		t.Fatal("TEST_MYSQL_DSN must name an isolated database ending in _test")
	}
	cfg.ParseTime = true
	cfg.Loc = time.UTC
	cfg.MultiStatements = true
	if cfg.Params == nil {
		cfg.Params = map[string]string{}
	}
	cfg.Params["time_zone"] = "'+00:00'"
	db, err := sql.Open("mysql", cfg.FormatDSN())
	if err != nil {
		t.Fatal("open test database failed")
	}
	db.SetMaxOpenConns(20)
	t.Cleanup(func() { _ = db.Close() })
	for _, table := range []string{"download_attempts", "plugin_version_stats", "plugin_stats", "plugin_versions", "catalog_state", "schema_migrations"} {
		if _, err := db.Exec("DROP TABLE IF EXISTS " + table); err != nil {
			t.Fatal("reset test schema failed")
		}
	}
	src, err := iofs.New(migrations.Files, ".")
	if err != nil {
		t.Fatal(err)
	}
	md, err := mysqlmigrate.WithInstance(db, &mysqlmigrate.Config{})
	if err != nil {
		t.Fatal(err)
	}
	m, err := migrate.NewWithInstance("iofs", src, "mysql", md)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _, _ = m.Close() })
	if err := m.Up(); err != nil {
		t.Fatal(err)
	}
	return &Store{DB: db}
}
func testPackage(id, version string) catalog.Package {
	sha := strings.Repeat("a", 64)
	return catalog.Package{ID: id, Version: version, SHA256: sha, Size: 42, ObjectKey: "packages/" + id + "/" + version + "/" + sha + ".tfp", Status: "active"}
}
func importCatalog(t *testing.T, s *Store, generation uint64, packages []catalog.Package) catalog.Catalog {
	t.Helper()
	c := catalog.Catalog{SchemaVersion: 1, SourceCommit: strings.Repeat("b", 40), Generation: generation, Plugins: packages}
	raw, _ := json.Marshal(c)
	c, digest, err := catalog.Read(strings.NewReader(string(raw)))
	if err != nil {
		t.Fatal(err)
	}
	if err = s.Import(context.Background(), c, digest, time.Now().UTC()); err != nil {
		t.Fatal(err)
	}
	return c
}
func expectCounters(t *testing.T, s *Store, attempts, downloads uint64) {
	t.Helper()
	var a, d uint64
	if err := s.DB.QueryRow(`SELECT attempts,downloads FROM plugin_stats WHERE plugin_id='plugin'`).Scan(&a, &d); err != nil {
		t.Fatal(err)
	}
	if a != attempts || d != downloads {
		t.Fatalf("plugin counters = %d/%d; want %d/%d", a, d, attempts, downloads)
	}
	var va, vd uint64
	if err := s.DB.QueryRow(`SELECT attempts,downloads FROM plugin_version_stats WHERE plugin_id='plugin' AND version='1.0.0'`).Scan(&va, &vd); err != nil {
		t.Fatal(err)
	}
	if va != a || vd != d {
		t.Fatalf("version/total divergence: %d/%d vs %d/%d", va, vd, a, d)
	}
}
func TestMySQLTransactionsAndStatistics(t *testing.T) {
	s := integrationStore(t)
	ctx := context.Background()
	now := time.Now().UTC().Truncate(time.Second)
	if err := s.Ready(ctx); err != nil {
		t.Fatal(err)
	}
	if s.CatalogReady(ctx) {
		t.Fatal("empty catalog reported ready")
	}
	packages := []catalog.Package{testPackage("plugin", "1.0.0"), testPackage("other", "1.0.0")}
	c := importCatalog(t, s, 1, packages)
	if !s.CatalogReady(ctx) {
		t.Fatal("imported catalog not ready")
	}
	t.Run("parallel issue is idempotent", func(t *testing.T) {
		errs := make(chan error, 50)
		var wg sync.WaitGroup
		for i := 0; i < 50; i++ {
			wg.Go(func() { _, err := s.Issue(ctx, "plugin", "1.0.0", strings.Repeat("c", 32), "k1", now); errs <- err })
		}
		wg.Wait()
		close(errs)
		for err := range errs {
			if err != nil {
				t.Fatal(err)
			}
		}
		expectCounters(t, s, 1, 0)
	})
	a, err := s.Issue(ctx, "plugin", "1.0.0", strings.Repeat("c", 32), "k1", now)
	if err != nil {
		t.Fatal(err)
	}
	t.Run("parallel confirmation increments once", func(t *testing.T) {
		errs := make(chan error, 100)
		var wg sync.WaitGroup
		for i := 0; i < 100; i++ {
			wg.Go(func() { errs <- s.Confirm(ctx, a.Claims, a.SHA256, a.Size, now.Add(time.Second)) })
		}
		wg.Wait()
		close(errs)
		for err := range errs {
			if err != nil {
				t.Fatal(err)
			}
		}
		expectCounters(t, s, 1, 1)
		if err := s.Confirm(ctx, a.Claims, strings.Repeat("f", 64), a.Size, now); !errors.Is(err, ErrConflict) {
			t.Fatalf("tampered body: %v", err)
		}
		if _, err := s.Issue(ctx, "other", "1.0.0", a.AttemptID, "k1", now); !errors.Is(err, ErrConflict) {
			t.Fatalf("cross plugin key collision: %v", err)
		}
	})
	b, err := s.Issue(ctx, "plugin", "1.0.0", strings.Repeat("d", 32), "k1", now)
	if err != nil {
		t.Fatal(err)
	}
	t.Run("failed counter update rolls back confirmation", func(t *testing.T) {
		if _, err := s.DB.Exec(`DELETE FROM plugin_version_stats WHERE plugin_id='plugin' AND version='1.0.0'`); err != nil {
			t.Fatal(err)
		}
		if err := s.Confirm(ctx, b.Claims, b.SHA256, b.Size, now); err == nil {
			t.Fatal("missing aggregate did not fail")
		}
		var status string
		if err := s.DB.QueryRow(`SELECT status FROM download_attempts WHERE attempt_id=UNHEX(?)`, b.AttemptID).Scan(&status); err != nil {
			t.Fatal(err)
		}
		if status != "issued" {
			t.Fatal("partial confirmation committed")
		}
		var downloads uint64
		if err := s.DB.QueryRow(`SELECT downloads FROM plugin_stats WHERE plugin_id='plugin'`).Scan(&downloads); err != nil {
			t.Fatal(err)
		}
		if downloads != 1 {
			t.Fatal("partial plugin increment committed")
		}
		if _, err := s.DB.Exec(`INSERT INTO plugin_version_stats(plugin_id,version,attempts,downloads,updated_at) VALUES ('plugin','1.0.0',2,1,UTC_TIMESTAMP(6))`); err != nil {
			t.Fatal(err)
		}
		expectCounters(t, s, 2, 1)
	})
	t.Run("catalog import idempotence conflicts and withdrawal", func(t *testing.T) {
		raw, _ := json.Marshal(c)
		same, digest, _ := catalog.Read(strings.NewReader(string(raw)))
		if err := s.Import(ctx, same, digest, now); err != nil {
			t.Fatal(err)
		}
		conflict := c
		conflict.Generation = 2
		conflict.Plugins = append([]catalog.Package(nil), c.Plugins...)
		conflict.Plugins[0].SHA256 = strings.Repeat("f", 64)
		conflict.Plugins[0].ObjectKey = "packages/plugin/1.0.0/" + conflict.Plugins[0].SHA256 + ".tfp"
		raw, _ = json.Marshal(conflict)
		conflict, digest, _ = catalog.Read(strings.NewReader(string(raw)))
		if err := s.Import(ctx, conflict, digest, now); !errors.Is(err, ErrConflict) {
			t.Fatalf("immutable metadata changed: %v", err)
		}
		withdrawn := append([]catalog.Package(nil), packages...)
		withdrawn[0].Status = "withdrawn"
		importCatalog(t, s, 2, withdrawn)
		if _, err := s.Issue(ctx, "plugin", "1.0.0", strings.Repeat("e", 32), "k1", now); !errors.Is(err, ErrGone) {
			t.Fatalf("withdrawn download allowed: %v", err)
		}
		if err := s.Confirm(ctx, b.Claims, b.SHA256, b.Size, now); err != nil {
			t.Fatal("withdrawal rejected issued receipt", err)
		}
		expectCounters(t, s, 2, 2)
		raw, _ = json.Marshal(c)
		stale, digest, _ := catalog.Read(strings.NewReader(string(raw)))
		if err := s.Import(ctx, stale, digest, now); !errors.Is(err, ErrConflict) {
			t.Fatal("old generation imported")
		}
	})
	t.Run("revoked and expired receipts", func(t *testing.T) {
		active := append([]catalog.Package(nil), packages...)
		importCatalog(t, s, 3, active)
		issued, err := s.Issue(ctx, "other", "1.0.0", strings.Repeat("e", 32), "k1", now)
		if err != nil {
			t.Fatal(err)
		}
		revoked := append([]catalog.Package(nil), packages...)
		revoked[1].Status = "revoked"
		importCatalog(t, s, 4, revoked)
		if err := s.Confirm(ctx, issued.Claims, issued.SHA256, issued.Size, now); !errors.Is(err, ErrRevoked) {
			t.Fatal("revocation did not reject receipt")
		}
		raw, _ := json.Marshal(catalog.Catalog{SchemaVersion: 1, SourceCommit: strings.Repeat("b", 40), Generation: 5, Plugins: active})
		revert, digest, _ := catalog.Read(strings.NewReader(string(raw)))
		if err := s.Import(ctx, revert, digest, now); !errors.Is(err, ErrConflict) {
			t.Fatal("revoked package resurrected", err)
		}
		packages[1].Status = "revoked"
		active[1].Status = "revoked"
		importCatalog(t, s, 5, active)
		expired, err := s.Issue(ctx, "plugin", "1.0.0", strings.Repeat("f", 32), "k1", now.Add(-8*24*time.Hour))
		if err != nil {
			t.Fatal(err)
		}
		if err := s.Confirm(ctx, expired.Claims, expired.SHA256, expired.Size, now); !errors.Is(err, ErrGone) {
			t.Fatal("expired attempt confirmed")
		}
	})
	t.Run("version pagination precision and cleanup", func(t *testing.T) {
		full := append([]catalog.Package(nil), packages...)
		for i := 1; i < 103; i++ {
			full = append(full, testPackage("plugin", fmt.Sprintf("2.0.%d", i)))
		}
		importCatalog(t, s, 6, full)
		batch, err := s.Batch(ctx, []string{"plugin", "unknown"})
		if err != nil {
			t.Fatal(err)
		}
		if len(batch.Plugins) != 1 || len(batch.Plugins[0].Versions) != 100 || !batch.Plugins[0].VersionsTruncated {
			t.Fatal("batch was not bounded")
		}
		detail, err := s.Plugin(ctx, "plugin", 50, 50)
		if err != nil || len(detail.Versions) != 50 || !detail.HasMore {
			t.Fatal("pagination failed", err)
		}
		v, err := s.Version(ctx, "plugin", "1.0.0")
		if err != nil || v.Downloads != "2" || v.TotalDownloads != "2" {
			t.Fatal("version totals wrong", err)
		}
		if _, err := s.Plugin(ctx, "unknown", 10, 0); !errors.Is(err, ErrNotFound) {
			t.Fatal("unknown plugin not distinguished")
		}
		if err := s.Cleanup(ctx, now.Add(40*24*time.Hour)); err != nil {
			t.Fatal(err)
		}
		total, err := s.Total(ctx)
		if err != nil || total.TotalDownloads != "2" {
			t.Fatal("cleanup changed aggregate", err)
		}
		// Cleanup cannot turn an old signed receipt into a new confirmation.
		if err := s.Confirm(ctx, a.Claims, a.SHA256, a.Size, now); !errors.Is(err, ErrNotFound) {
			t.Fatal("deleted receipt recreated", err)
		}
	})
}
