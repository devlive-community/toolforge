// Package receipt implements bounded HMAC signed download receipts. Receipts are
// bearer credentials and must never appear in URLs or logs.
package receipt

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"strings"
	"time"

	"toolforge.dev/plugin-stats/internal/catalog"
)

var ErrInvalid = errors.New("invalid receipt")
var ErrExpired = errors.New("expired receipt")

const Lifetime = 7 * 24 * time.Hour

type Claims struct {
	Kid       string `json:"kid"`
	AttemptID string `json:"attemptId"`
	PluginID  string `json:"pluginId"`
	Version   string `json:"version"`
	SHA256    string `json:"sha256"`
	Size      uint64 `json:"size"`
	IssuedAt  int64  `json:"iat"`
	ExpiresAt int64  `json:"exp"`
}
type Signer struct {
	active string
	keys   map[string][]byte
}

func New(active string, keys map[string][]byte) (*Signer, error) {
	if len(active) == 0 || len(active) > 32 || len(keys) == 0 || len(keys) > 8 {
		return nil, ErrInvalid
	}
	copied := make(map[string][]byte, len(keys))
	for kid, key := range keys {
		if len(kid) == 0 || len(kid) > 32 || len(key) < 32 {
			return nil, ErrInvalid
		}
		copied[kid] = append([]byte(nil), key...)
	}
	if _, ok := copied[active]; !ok {
		return nil, ErrInvalid
	}
	return &Signer{active: active, keys: copied}, nil
}
func (s *Signer) ActiveKid() string { return s.active }
func (s *Signer) Sign(c Claims) (string, error) {
	key, ok := s.keys[c.Kid]
	if !ok || !valid(c) {
		return "", ErrInvalid
	}
	payload, err := json.Marshal(c)
	if err != nil {
		return "", ErrInvalid
	}
	body := base64.RawURLEncoding.EncodeToString(payload)
	mac := hmac.New(sha256.New, key)
	_, _ = mac.Write([]byte(body))
	return body + "." + base64.RawURLEncoding.EncodeToString(mac.Sum(nil)), nil
}
func (s *Signer) Verify(token string, now time.Time) (Claims, error) {
	var c Claims
	if len(token) > 2048 {
		return c, ErrInvalid
	}
	parts := strings.Split(token, ".")
	if len(parts) != 2 {
		return c, ErrInvalid
	}
	raw, err := base64.RawURLEncoding.DecodeString(parts[0])
	if err != nil {
		return c, ErrInvalid
	}
	dec := json.NewDecoder(strings.NewReader(string(raw)))
	dec.DisallowUnknownFields()
	if dec.Decode(&c) != nil || dec.Decode(new(any)) != io.EOF || !valid(c) {
		return Claims{}, ErrInvalid
	}
	key, ok := s.keys[c.Kid]
	if !ok {
		return Claims{}, ErrInvalid
	}
	sig, err := base64.RawURLEncoding.DecodeString(parts[1])
	if err != nil {
		return Claims{}, ErrInvalid
	}
	mac := hmac.New(sha256.New, key)
	_, _ = mac.Write([]byte(parts[0]))
	if !hmac.Equal(sig, mac.Sum(nil)) || c.IssuedAt > now.Unix()+60 {
		return Claims{}, ErrInvalid
	}
	if c.ExpiresAt <= now.Unix() {
		return Claims{}, ErrExpired
	}
	return c, nil
}
func valid(c Claims) bool {
	return len(c.Kid) > 0 && len(c.Kid) <= 32 && catalog.AttemptPattern.MatchString(c.AttemptID) && catalog.IDPattern.MatchString(c.PluginID) && catalog.VersionPattern.MatchString(c.Version) && catalog.SHAPattern.MatchString(c.SHA256) && c.Size > 0 && c.Size <= catalog.MaxPackageSize && c.IssuedAt > 0 && c.ExpiresAt > c.IssuedAt && c.ExpiresAt-c.IssuedAt <= int64(Lifetime/time.Second)
}
