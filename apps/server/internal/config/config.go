package config

import (
	"encoding/base64"
	"encoding/json"
	"errors"
	"net"
	"net/url"
	"os"
	"strings"
	"time"

	"github.com/go-sql-driver/mysql"
	"toolforge.dev/plugin-stats/internal/receipt"
)

type Config struct {
	ListenAddr, AssetBaseURL, DSN string
	Signer                        *receipt.Signer
}

func env(key, fallback string) string {
	if value, ok := os.LookupEnv(key); ok {
		return value
	}
	return fallback
}

// Read never includes environment values in an error. The same binary uses
// independently protected runtime, migration, and catalog environment files.
func Read(serving bool) (Config, error) {
	var c Config
	rawDSN := os.Getenv("MYSQL_DSN")
	var db *mysql.Config
	var err error
	if rawDSN != "" {
		db, err = mysql.ParseDSN(rawDSN)
		if err != nil {
			return c, errors.New("invalid MYSQL_DSN")
		}
	} else {
		db = mysql.NewConfig()
		db.User = os.Getenv("MYSQL_USER")
		db.Passwd = os.Getenv("MYSQL_PASSWORD")
		db.DBName = env("MYSQL_DATABASE", "toolforge_stats")
		db.Net = "unix"
		db.Addr = env("MYSQL_SOCKET", "/var/run/mysqld/mysqld.sock")
	}
	if db.User == "" || db.DBName == "" || db.User == "root" || db.AllowAllFiles || db.AllowCleartextPasswords || db.AllowNativePasswords == false {
		return c, errors.New("invalid or unsafe MySQL account configuration")
	}
	if db.Net != "unix" && (db.Net != "tcp" || !loopbackHostPort(db.Addr)) {
		return c, errors.New("MySQL must use a local socket or loopback address")
	}
	db.ParseTime = true
	db.Loc = time.UTC
	db.Timeout = 5 * time.Second
	db.ReadTimeout = 5 * time.Second
	db.WriteTimeout = 5 * time.Second
	db.MultiStatements = false
	db.InterpolateParams = false
	if db.Params == nil {
		db.Params = map[string]string{}
	}
	db.Params["time_zone"] = "'+00:00'"
	c.DSN = db.FormatDSN()
	if !serving {
		return c, nil
	}
	c.ListenAddr = env("STATS_LISTEN_ADDR", "127.0.0.1:8088")
	if !loopbackHostPort(c.ListenAddr) {
		return c, errors.New("STATS_LISTEN_ADDR must bind a loopback address")
	}
	c.AssetBaseURL = strings.TrimRight(env("PLUGIN_ASSET_BASE_URL", "https://plugins.toolforge.devlive.org"), "/")
	asset, err := url.Parse(c.AssetBaseURL)
	if err != nil || asset.Scheme != "https" || asset.Host == "" || asset.User != nil || asset.RawQuery != "" || asset.Fragment != "" || asset.Path != "" || asset.Opaque != "" {
		return c, errors.New("PLUGIN_ASSET_BASE_URL requires an HTTPS origin")
	}
	var encoded map[string]string
	if err := json.Unmarshal([]byte(os.Getenv("RECEIPT_KEYS_JSON")), &encoded); err != nil {
		return c, errors.New("invalid RECEIPT_KEYS_JSON")
	}
	keys := make(map[string][]byte, len(encoded))
	for kid, value := range encoded {
		key, err := base64.StdEncoding.DecodeString(value)
		if err != nil {
			return c, errors.New("receipt key must be standard base64")
		}
		keys[kid] = key
	}
	c.Signer, err = receipt.New(os.Getenv("RECEIPT_ACTIVE_KID"), keys)
	if err != nil {
		return c, errors.New("invalid receipt keyring")
	}
	return c, nil
}
func loopbackHostPort(addr string) bool {
	host, port, err := net.SplitHostPort(addr)
	if err != nil || port == "" || port == "0" {
		return false
	}
	ip := net.ParseIP(host)
	return ip != nil && ip.IsLoopback()
}
