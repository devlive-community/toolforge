package receipt

import (
	"bytes"
	"strings"
	"testing"
	"time"
)

func testClaims() Claims {
	return Claims{Kid: "old", AttemptID: strings.Repeat("a", 32), PluginID: "json-formatter", Version: "1.0.0", SHA256: strings.Repeat("b", 64), Size: 42, IssuedAt: 1700000000, ExpiresAt: 1700000000 + int64(Lifetime/time.Second)}
}
func TestSigningRotationAndTampering(t *testing.T) {
	oldKey := bytes.Repeat([]byte{1}, 32)
	newKey := bytes.Repeat([]byte{2}, 32)
	old, err := New("old", map[string][]byte{"old": oldKey})
	if err != nil {
		t.Fatal(err)
	}
	c := testClaims()
	token, err := old.Sign(c)
	if err != nil {
		t.Fatal(err)
	}
	rotating, err := New("new", map[string][]byte{"old": oldKey, "new": newKey})
	if err != nil {
		t.Fatal(err)
	}
	now := time.Unix(c.IssuedAt+1, 0)
	got, err := rotating.Verify(token, now)
	if err != nil || got != c {
		t.Fatalf("rotation lost old receipt: %v", err)
	}
	tests := []string{token + "x", "x" + token, strings.Repeat("x", 4096), "", token + ".extra"}
	for i, token := range tests {
		if _, err := rotating.Verify(token, now); err == nil {
			t.Fatalf("accepted tampered token %d", i)
		}
	}
	if _, err := rotating.Verify(token, time.Unix(c.ExpiresAt, 0)); err != ErrExpired {
		t.Fatalf("expiry: %v", err)
	}
	retired, _ := New("new", map[string][]byte{"new": newKey})
	if _, err := retired.Verify(token, now); err != ErrInvalid {
		t.Fatalf("retired key: %v", err)
	}
}
func TestClaimsBounds(t *testing.T) {
	s, _ := New("old", map[string][]byte{"old": bytes.Repeat([]byte{1}, 32)})
	c := testClaims()
	c.ExpiresAt++
	if _, err := s.Sign(c); err == nil {
		t.Fatal("accepted excessive lifetime")
	}
	if _, err := New("bad", map[string][]byte{"bad": []byte("short")}); err == nil {
		t.Fatal("accepted short key")
	}
}
