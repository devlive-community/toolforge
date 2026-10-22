package main

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/go-sql-driver/mysql"
	"github.com/golang-migrate/migrate/v4"
	mysqlmigrate "github.com/golang-migrate/migrate/v4/database/mysql"
	"github.com/golang-migrate/migrate/v4/source/iofs"
	"toolforge.dev/plugin-stats/internal/api"
	"toolforge.dev/plugin-stats/internal/asset"
	"toolforge.dev/plugin-stats/internal/catalog"
	"toolforge.dev/plugin-stats/internal/config"
	"toolforge.dev/plugin-stats/internal/store"
	"toolforge.dev/plugin-stats/migrations"
)

var buildSHA = "development"

func main() {
	if err := run(os.Args[1:]); err != nil {
		slog.Error("command failed", "reason", err.Error())
		os.Exit(1)
	}
}
func run(args []string) error {
	command := "serve"
	if len(args) > 0 {
		command = args[0]
	}
	if command == "version" {
		fmt.Println(buildSHA)
		return nil
	}
	serving := command == "serve"
	if !(serving && (len(args) == 0 || len(args) == 1)) && !(command == "migrate" && len(args) == 2 && args[1] == "up") && !(command == "catalog" && len(args) == 3 && args[1] == "import") {
		return errors.New("usage: plugin-stats [serve | version | migrate up | catalog import <file>]")
	}
	cfg, err := config.Read(serving)
	if err != nil {
		return err
	}
	if command == "migrate" {
		migrationDB, parseErr := mysql.ParseDSN(cfg.DSN)
		if parseErr != nil {
			return errors.New("invalid migration database configuration")
		}
		migrationDB.MultiStatements = true
		cfg.DSN = migrationDB.FormatDSN()
	}
	db, err := sql.Open("mysql", cfg.DSN)
	if err != nil {
		return errors.New("cannot initialize database")
	}
	defer db.Close()
	db.SetMaxOpenConns(12)
	db.SetMaxIdleConns(4)
	db.SetConnMaxLifetime(3 * time.Minute)
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()
	pingCtx, pingCancel := context.WithTimeout(ctx, 10*time.Second)
	err = db.PingContext(pingCtx)
	pingCancel()
	if err != nil {
		return errors.New("cannot connect to database")
	}
	if command == "migrate" {
		return migrateUp(db)
	}
	storage := &store.Store{DB: db}
	if command == "catalog" {
		f, err := os.Open(args[2])
		if err != nil {
			return errors.New("cannot open catalog file")
		}
		defer f.Close()
		c, digest, err := catalog.Read(f)
		if err != nil {
			return err
		}
		importCtx, importCancel := context.WithTimeout(ctx, 2*time.Minute)
		defer importCancel()
		if err = storage.Import(importCtx, c, digest, time.Now().UTC()); err != nil {
			if errors.Is(err, store.ErrConflict) {
				return errors.New("catalog conflicts with immutable metadata or release generation")
			}
			return errors.New("catalog import failed")
		}
		slog.Info("catalog imported", "packages", len(c.Plugins), "generation", c.Generation)
		return nil
	}
	// Schema readiness is required at startup; a catalog may be bootstrapped
	// separately before /readyz becomes ready, without losing the first deploy.
	var schema int
	var dirty bool
	if err = db.QueryRowContext(ctx, "SELECT version,dirty FROM schema_migrations LIMIT 1").Scan(&schema, &dirty); err != nil || schema != store.SchemaVersion || dirty {
		return errors.New("database migration is required")
	}
	var presigner *asset.Presigner
	if cfg.CosSecretID != "" {
		presigner = &asset.Presigner{SecretID: cfg.CosSecretID, SecretKey: cfg.CosSecretKey}
		slog.Info("download URLs are COS presigned for a private bucket")
	}
	h := (&api.API{Store: storage, Signer: cfg.Signer, AssetBaseURL: cfg.AssetBaseURL, Presigner: presigner, BuildSHA: buildSHA, Logger: slog.Default()}).Handler()
	srv := &http.Server{Addr: cfg.ListenAddr, Handler: h, ReadHeaderTimeout: 5 * time.Second, ReadTimeout: 10 * time.Second, WriteTimeout: 15 * time.Second, IdleTimeout: 60 * time.Second, MaxHeaderBytes: 16 << 10, ErrorLog: slog.NewLogLogger(slog.Default().Handler(), slog.LevelError)}
	done := make(chan error, 1)
	go func() { done <- srv.ListenAndServe() }()
	cleanupDone := make(chan struct{})
	go func() {
		defer close(cleanupDone)
		ticker := time.NewTicker(time.Hour)
		defer ticker.Stop()
		for {
			select {
			case <-ctx.Done():
				return
			case now := <-ticker.C:
				cleanCtx, cleanCancel := context.WithTimeout(ctx, 10*time.Second)
				err := storage.Cleanup(cleanCtx, now)
				cleanCancel()
				if err != nil {
					slog.Error("attempt cleanup failed")
				}
			}
		}
	}()
	slog.Info("plugin statistics listening", "buildSha", buildSHA)
	select {
	case err := <-done:
		if !errors.Is(err, http.ErrServerClosed) {
			return errors.New("HTTP listener failed")
		}
	case <-ctx.Done():
		shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer shutdownCancel()
		if srv.Shutdown(shutdownCtx) != nil {
			return errors.New("HTTP shutdown timed out")
		}
		<-cleanupDone
	}
	return nil
}
func migrateUp(db *sql.DB) error {
	src, err := iofs.New(migrations.Files, ".")
	if err != nil {
		return errors.New("cannot initialize embedded migrations")
	}
	driver, err := mysqlmigrate.WithInstance(db, &mysqlmigrate.Config{})
	if err != nil {
		return errors.New("cannot initialize migration driver")
	}
	m, err := migrate.NewWithInstance("iofs", src, "mysql", driver)
	if err != nil {
		return errors.New("cannot initialize migrations")
	}
	defer func() { _, _ = m.Close() }()
	m.LockTimeout = 30 * time.Second
	if err = m.Up(); err != nil && !errors.Is(err, migrate.ErrNoChange) {
		return errors.New("migration failed; inspect schema_migrations and backup before repair")
	}
	slog.Info("database migrations applied")
	return nil
}
