package audit

import (
	"crypto/sha256"
	"encoding/json"
	"errors"
	"fmt"
	"path/filepath"
	"time"

	bolt "go.etcd.io/bbolt"
)

var bucketName = []byte("audit")

// AuditEntry represents a single entry in the tamper-proof audit log.
type AuditEntry struct {
	Timestamp time.Time `json:"timestamp"`
	Action    string    `json:"action"`
	AgentID   string    `json:"agent_id"`
	Details   string    `json:"details"`
	PrevHash  []byte    `json:"prev_hash"`
	Hash      []byte    `json:"hash"`
}

// NewEntry creates a new AuditEntry with the current UTC timestamp and computed hash.
func NewEntry(action, agentID, details string) AuditEntry {
	e := AuditEntry{
		Timestamp: time.Now().UTC(),
		Action:    action,
		AgentID:   agentID,
		Details:   details,
	}
	e.ComputeHash()
	return e
}

// ComputeHash sets Hash to SHA256(PrevHash + Action + Timestamp.RFC3339Nano + Details).
func (e *AuditEntry) ComputeHash() {
	h := sha256.New()
	h.Write(e.PrevHash)
	h.Write([]byte(e.Action))
	h.Write([]byte(e.Timestamp.Format(time.RFC3339Nano)))
	h.Write([]byte(e.Details))
	e.Hash = h.Sum(nil)
}

// VerifyChain checks that each entry's PrevHash matches the previous entry's Hash.
// An empty or nil chain is considered valid.
func VerifyChain(entries []AuditEntry) error {
	if len(entries) == 0 {
		return nil
	}
	for i := 1; i < len(entries); i++ {
		prev := entries[i-1]
		cur := entries[i]
		if !equalHash(prev.Hash, cur.PrevHash) {
			return fmt.Errorf("chain broken at entry %d: prev hash mismatch", i)
		}
	}
	return nil
}

func equalHash(a, b []byte) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

// AuditLog is a tamper-proof audit log backed by bbolt.
type AuditLog struct {
	db *bolt.DB
}

// NewAuditLog opens or creates a bbolt database in the given directory
// and ensures the "audit" bucket exists.
func NewAuditLog(dir string) (*AuditLog, error) {
	path := filepath.Join(dir, "audit.db")
	db, err := bolt.Open(path, 0600, nil)
	if err != nil {
		return nil, fmt.Errorf("open bbolt: %w", err)
	}
	err = db.Update(func(tx *bolt.Tx) error {
		_, err := tx.CreateBucketIfNotExists(bucketName)
		return err
	})
	if err != nil {
		db.Close()
		return nil, fmt.Errorf("create bucket: %w", err)
	}
	return &AuditLog{db: db}, nil
}

// Append creates a new audit entry linked to the last entry in the log,
// and saves it to bbolt keyed by the entry's timestamp in RFC3339Nano format.
func (l *AuditLog) Append(action, agentID, details string) error {
	entry := NewEntry(action, agentID, details)

	return l.db.Update(func(tx *bolt.Tx) error {
		b := tx.Bucket(bucketName)
		if b == nil {
			return errors.New("audit bucket not found")
		}

		// Get last entry to link hash chain
		c := b.Cursor()
		if k, v := c.Last(); k != nil {
			var last AuditEntry
			if err := json.Unmarshal(v, &last); err != nil {
				return fmt.Errorf("unmarshal last entry: %w", err)
			}
			entry.PrevHash = last.Hash
			entry.ComputeHash()
		}

		data, err := json.Marshal(entry)
		if err != nil {
			return fmt.Errorf("marshal entry: %w", err)
		}

		key := []byte(entry.Timestamp.Format(time.RFC3339Nano))
		return b.Put(key, data)
	})
}

// GetEntries retrieves audit entries within the given time range [from, to].
// If from is zero, it starts from the beginning.
// If to is zero, it includes all entries after from.
// Entries are returned in chronological order.
func (l *AuditLog) GetEntries(from, to time.Time) []AuditEntry {
	var entries []AuditEntry

	l.db.View(func(tx *bolt.Tx) error {
		b := tx.Bucket(bucketName)
		if b == nil {
			return nil
		}

		c := b.Cursor()

		var k []byte
		var v []byte

		if from.IsZero() {
			k, v = c.First()
		} else {
			fromKey := []byte(from.Format(time.RFC3339Nano))
			k, v = c.Seek(fromKey)
		}

		for ; k != nil; k, v = c.Next() {
			if !to.IsZero() {
				ts, err := time.Parse(time.RFC3339Nano, string(k))
				if err != nil {
					continue
				}
				if ts.After(to) {
					break
				}
			}

			var entry AuditEntry
			if err := json.Unmarshal(v, &entry); err != nil {
				continue
			}
			entries = append(entries, entry)
		}
		return nil
	})

	return entries
}

// Close closes the underlying bbolt database.
func (l *AuditLog) Close() error {
	return l.db.Close()
}
