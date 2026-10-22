package catalog

import (
	"encoding/json"
	"strings"
	"testing"
)

func fixture() Catalog {
	sha := strings.Repeat("a", 64)
	return Catalog{SchemaVersion: 1, SourceCommit: strings.Repeat("b", 40), Generation: 1, Plugins: []Package{{ID: "json-formatter", Version: "1.0.0", SHA256: sha, Size: 42, ObjectKey: "packages/json-formatter/1.0.0/" + sha + ".tfp", Status: "active"}}}
}
func TestCatalogCanonicalDigestAndValidation(t *testing.T) {
	c := fixture()
	raw, _ := json.Marshal(c)
	_, digest, err := Read(strings.NewReader(string(raw)))
	if err != nil {
		t.Fatal(err)
	}
	_, digest2, err := Read(strings.NewReader(" \n" + string(raw) + "\n"))
	if err != nil || digest != digest2 {
		t.Fatal("whitespace changed digest")
	}
	cases := []struct {
		name   string
		mutate func(*Catalog)
	}{
		{"traversal", func(c *Catalog) { c.Plugins[0].ObjectKey = "packages/../secret" }},
		{"external", func(c *Catalog) { c.Plugins[0].ObjectKey = "https://attacker.invalid/file" }},
		{"duplicate", func(c *Catalog) { c.Plugins = append(c.Plugins, c.Plugins[0]) }},
		{"generation", func(c *Catalog) { c.Generation = 0 }},
		{"sha", func(c *Catalog) { c.Plugins[0].SHA256 = "bad" }},
		{"status", func(c *Catalog) { c.Plugins[0].Status = "deleted" }},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			c := fixture()
			tc.mutate(&c)
			if c.Validate() == nil {
				t.Fatal("accepted invalid catalog")
			}
		})
	}
	if _, _, err := Read(strings.NewReader(string(raw) + "{}")); err == nil {
		t.Fatal("accepted trailing JSON")
	}
	if _, _, err := Read(strings.NewReader(`{"schemaVersion":1,"unknown":true}`)); err == nil {
		t.Fatal("accepted unknown field")
	}
}
