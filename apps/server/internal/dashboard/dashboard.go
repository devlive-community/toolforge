// Package dashboard serves the public statistics dashboard at the site root.
// The page is static and embedded in the binary; data comes from /v1/dashboard.
package dashboard

import (
	"embed"
	"io/fs"
	"net/http"
	"path"
)

//go:embed static
var files embed.FS

// Only the embedded page, script and stylesheet can load; no inline code,
// external hosts, frames or form submissions.
const contentSecurityPolicy = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"

var contentTypes = map[string]string{
	".html": "text/html; charset=utf-8",
	".css":  "text/css; charset=utf-8",
	".js":   "text/javascript; charset=utf-8",
}

func serve(w http.ResponseWriter, name, cache string) {
	data, err := fs.ReadFile(files, "static/"+name)
	contentType, known := contentTypes[path.Ext(name)]
	if err != nil || !known {
		http.NotFound(w, nil)
		return
	}
	w.Header().Set("Content-Type", contentType)
	w.Header().Set("Content-Security-Policy", contentSecurityPolicy)
	w.Header().Set("Cache-Control", cache)
	_, _ = w.Write(data)
}

// Index serves the dashboard page; it is revalidated so a deploy shows at once.
func Index(w http.ResponseWriter, _ *http.Request) {
	serve(w, "index.html", "no-cache")
}

// Asset serves the dashboard script and stylesheet.
func Asset(w http.ResponseWriter, r *http.Request) {
	name := r.PathValue("file")
	if name != "dashboard.js" && name != "dashboard.css" {
		http.NotFound(w, r)
		return
	}
	serve(w, name, "public, max-age=300")
}
