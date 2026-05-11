package session

import (
	"crypto/sha256"
	"fmt"
	"os"
	"path/filepath"
)

// BlobStore implements content-addressed storage using SHA-256 hashes as keys.
// Each blob is stored as a file named by its hex-encoded SHA-256 hash.
type BlobStore struct {
	dir string
}

// NewBlobStore creates a BlobStore that stores blobs in the given directory.
// The directory is created if it does not exist.
func NewBlobStore(dir string) *BlobStore {
	return &BlobStore{dir: dir}
}

// Put writes data to the store and returns its SHA-256 hex hash.
// If a blob with the same content already exists, it is not written again.
func (b *BlobStore) Put(data []byte) (string, error) {
	hash := sha256.Sum256(data)
	hexHash := fmt.Sprintf("%x", hash)

	path := filepath.Join(b.dir, hexHash)

	// Check if already exists (idempotent)
	if _, err := os.Stat(path); err == nil {
		return hexHash, nil
	}

	if err := os.MkdirAll(b.dir, 0o755); err != nil {
		return "", fmt.Errorf("blob store mkdir: %w", err)
	}

	if err := os.WriteFile(path, data, 0o644); err != nil {
		return "", fmt.Errorf("blob store write: %w", err)
	}

	return hexHash, nil
}

// Get reads a blob by its SHA-256 hex hash.
// Returns an error if the blob does not exist.
func (b *BlobStore) Get(hash string) ([]byte, error) {
	path := filepath.Join(b.dir, hash)
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("blob store get %s: %w", hash, err)
	}
	return data, nil
}

// Has returns true if a blob with the given hash exists in the store.
func (b *BlobStore) Has(hash string) bool {
	path := filepath.Join(b.dir, hash)
	_, err := os.Stat(path)
	return err == nil
}
