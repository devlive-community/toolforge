// Package asset builds download URLs for COS objects. A private bucket needs
// a presigned URL; public buckets keep the plain base URL untouched.
package asset

import (
	"crypto/hmac"
	"crypto/sha1"
	"encoding/hex"
	"net/url"
	"strconv"
	"strings"
	"time"
)

// Presigner signs COS object GET URLs with the XML API presign algorithm.
type Presigner struct {
	SecretID  string
	SecretKey string
	// TTL is the validity window of one URL; it defaults to ten minutes.
	TTL time.Duration
	Now func() time.Time
}

func (p *Presigner) now() time.Time {
	if p.Now != nil {
		return p.Now().UTC()
	}
	return time.Now().UTC()
}

func (p *Presigner) ttl() time.Duration {
	if p.TTL > 0 {
		return p.TTL
	}
	return 10 * time.Minute
}

// Presign returns a signed GET URL for key (given without a leading slash)
// served from host. The signature covers only the request path, so custom
// COS domains work as long as they resolve to the same bucket.
func (p *Presigner) Presign(host, key string) string {
	host = strings.TrimRight(host, "/")
	key = strings.TrimLeft(key, "/")
	now := p.now().Unix()
	keyTime := strconv.FormatInt(now, 10) + ";" + strconv.FormatInt(now+int64(p.ttl().Seconds()), 10)
	signKey := hex.EncodeToString(hmacSHA1([]byte(p.SecretKey), []byte(keyTime)))
	// Lowercase verb, URI path, empty query and header lists: exactly what a
	// plain GET from any HTTP client produces for this URL.
	httpString := "get\n/" + key + "\n\n\n"
	sum := sha1.Sum([]byte(httpString))
	stringToSign := "sha1\n" + keyTime + "\n" + hex.EncodeToString(sum[:]) + "\n"
	signature := hex.EncodeToString(hmacSHA1([]byte(signKey), []byte(stringToSign)))
	return host + "/" + key +
		"?q-sign-algorithm=sha1&q-ak=" + url.QueryEscape(p.SecretID) +
		"&q-sign-time=" + keyTime + "&q-key-time=" + keyTime +
		"&q-header-list=&q-url-param-list=&q-signature=" + signature
}

func hmacSHA1(key, message []byte) []byte {
	mac := hmac.New(sha1.New, key)
	mac.Write(message)
	return mac.Sum(nil)
}
