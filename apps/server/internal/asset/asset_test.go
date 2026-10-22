package asset

import (
	"crypto/sha1"
	"encoding/hex"
	"net/url"
	"strings"
	"testing"
	"time"
)

func TestPresignProducesSignedCOSURL(t *testing.T) {
	now := time.Unix(1767225600, 0)
	p := &Presigner{SecretID: "AKIDexample", SecretKey: "secret-key", Now: func() time.Time { return now }}
	got := p.Presign("https://plugins.example.com/", "packages/x/1.0.0/abc.tfp")

	if !strings.HasPrefix(got, "https://plugins.example.com/packages/x/1.0.0/abc.tfp?") {
		t.Fatalf("URL must target the object key: %s", got)
	}
	// 签名有效期 10 分钟：1767225600 到 1767226200
	if !strings.Contains(got, "q-sign-time=1767225600;1767226200") || !strings.Contains(got, "q-key-time=1767225600;1767226200") {
		t.Fatalf("missing key time window: %s", got)
	}
	if !strings.Contains(got, "q-ak=AKIDexample") || !strings.Contains(got, "q-header-list=&q-url-param-list=&q-signature=") {
		t.Fatalf("missing ak or signature parts: %s", got)
	}
	// 独立复算签名：SignKey = HMAC(SecretKey, KeyTime)；Signature = HMAC(SignKey, StringToSign)
	keyTime := "1767225600;1767226200"
	signKey := hex.EncodeToString(hmacSHA1([]byte("secret-key"), []byte(keyTime)))
	sum := sha1.Sum([]byte("get\n/packages/x/1.0.0/abc.tfp\n\n\n"))
	stringToSign := "sha1\n" + keyTime + "\n" + hex.EncodeToString(sum[:]) + "\n"
	want := hex.EncodeToString(hmacSHA1([]byte(signKey), []byte(stringToSign)))
	if !strings.HasSuffix(got, "q-signature="+want) {
		t.Fatalf("signature mismatch:\n got %s\nwant suffix %s", got, want)
	}
}

func TestPresignEscapesAndTrims(t *testing.T) {
	p := &Presigner{SecretID: "AKID needs/escape", SecretKey: "k", TTL: time.Minute, Now: func() time.Time { return time.Unix(0, 0) }}
	got := p.Presign("https://host.example//", "/leading/slash/key")
	if strings.HasPrefix(got, "https://host.example//") {
		t.Fatalf("double slash must be trimmed: %s", got)
	}
	if !strings.Contains(got, "q-ak="+url.QueryEscape("AKID needs/escape")) {
		t.Fatalf("secret id must be query escaped: %s", got)
	}
	if !strings.Contains(got, "q-sign-time=0;60") {
		t.Fatalf("one minute TTL not applied: %s", got)
	}
}
