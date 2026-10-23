package config

import (
	"strings"
	"testing"
)

func baseEnv(t *testing.T) {
	t.Helper()
	t.Setenv("MYSQL_DSN", "")
	t.Setenv("MYSQL_USER", "toolforge_runtime")
	t.Setenv("MYSQL_PASSWORD", "test-password")
	t.Setenv("MYSQL_DATABASE", "toolforge_stats")
	t.Setenv("MYSQL_SOCKET", "/tmp/mysql.sock")
	t.Setenv("STATS_LISTEN_ADDR", "127.0.0.1:8088")
	t.Setenv("PLUGIN_ASSET_BASE_URL", "https://plugins.toolforge.devlive.org")
	t.Setenv("RECEIPT_ACTIVE_KID", "k1")
	t.Setenv("RECEIPT_KEYS_JSON", `{"k1":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="}`)
}
func TestSafeConfiguration(t *testing.T) {
	baseEnv(t)
	c, err := Read(true)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(c.DSN, "parseTime=true") || c.Signer == nil {
		t.Fatal("missing configuration")
	}
	t.Setenv("PLUGIN_REGISTRY_URL", "")
	c, err = Read(true)
	if err != nil || c.RegistryURL != "" {
		t.Fatal("explicitly empty registry URL must disable dashboard names")
	}
	t.Setenv("RECEIPT_KEYS_JSON", "not-secret-json")
	if _, err := Read(false); err != nil {
		t.Fatal("administrative CLI required receipt keys")
	}
}
func TestUnsafeConfigurationRejected(t *testing.T) {
	for _, tc := range []struct{ key, value string }{{"STATS_LISTEN_ADDR", "0.0.0.0:8088"}, {"PLUGIN_ASSET_BASE_URL", "http://insecure.invalid"}, {"PLUGIN_ASSET_BASE_URL", "https://user:password@host.invalid"}, {"MYSQL_USER", "root"}, {"PLUGIN_REGISTRY_URL", "http://plugins.invalid/registry.json"}, {"PLUGIN_REGISTRY_URL", "https://user:password@plugins.invalid/registry.json"}, {"PLUGIN_REGISTRY_URL", "https://plugins.invalid/registry.json#frag"}, {"MYSQL_DSN", "user:secret@tcp(remote.invalid:3306)/db"}, {"RECEIPT_KEYS_JSON", `{"k1":"c2hvcnQ="}`}} {
		t.Run(tc.key+tc.value, func(t *testing.T) {
			baseEnv(t)
			t.Setenv(tc.key, tc.value)
			if _, err := Read(true); err == nil {
				t.Fatal("unsafe configuration accepted")
			} else if strings.Contains(err.Error(), tc.value) {
				t.Fatal("configuration value leaked in error")
			}
		})
	}
}
