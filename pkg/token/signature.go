package token

import (
	"crypto/sha256"
	"encoding/base64"
	"fmt"
)

const (
	// SignatureSize is size of token signature
	SignatureSize = sha256.Size

	// SignatureBase64Size is size of Signature in Base64 format(with padding)
	SignatureBase64Size = (SignatureSize + 2) / 3 * 4
)

// Signature is only signature of token
type Signature [SignatureSize]byte

// Equal checks equality of two signatures
func (s Signature) Equal(s2 Signature) bool {
	return s == s2
}

// Base64 encodes signature to base64 format
func (s Signature) Base64() (b64 [SignatureBase64Size]byte) {
	base64.StdEncoding.Encode(b64[:], s[:])
	return
}

// SignatureFromBase64 parses base64 signature and returns Signature
func SignatureFromBase64(b64 [SignatureBase64Size]byte) (s Signature, err error) {
	n, err := base64.StdEncoding.Decode(s[:], b64[:])
	if err != nil {
		return Signature{}, err
	}
	if n != SignatureSize {
		return Signature{}, fmt.Errorf("invalid signature size: %d", n)
	}
	return s, nil
}