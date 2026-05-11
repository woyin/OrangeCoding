package audit

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestVerifyChainEmpty(t *testing.T) {
	err := VerifyChain(nil)
	if err != nil {
		t.Fatalf("empty chain should be valid, got error: %v", err)
	}

	err = VerifyChain([]AuditEntry{})
	if err != nil {
		t.Fatalf("empty chain should be valid, got error: %v", err)
	}
}

func TestVerifyChainSingle(t *testing.T) {
	e := NewEntry("action", "agent-1", "details")
	err := VerifyChain([]AuditEntry{e})
	if err != nil {
		t.Fatalf("single entry chain should be valid, got error: %v", err)
	}
}

func TestAuditEntryHashChain(t *testing.T) {
	// Create two linked entries
	e1 := NewEntry("login", "agent-1", "user logged in")
	e2 := NewEntry("logout", "agent-1", "user logged out")
	e2.PrevHash = e1.Hash
	e2.ComputeHash()

	entries := []AuditEntry{e1, e2}

	// Verify chain should pass
	if err := VerifyChain(entries); err != nil {
		t.Fatalf("valid chain should pass, got error: %v", err)
	}

	// Tamper with first entry's hash to break the chain
	entries[0].Hash = []byte("tampered-hash-value-that-is-invalid")

	// Verify chain should fail
	err := VerifyChain(entries)
	if err == nil {
		t.Fatal("tampered chain should fail verification")
	}
}

func TestAuditLogAppendGet(t *testing.T) {
	// Create temp directory for bbolt DB
	dir := t.TempDir()

	log, err := NewAuditLog(dir)
	if err != nil {
		t.Fatalf("failed to create audit log: %v", err)
	}
	defer log.Close()

	// Append entries with slight time separation
	log.Append("create", "agent-1", "created resource")
	time.Sleep(2 * time.Millisecond)
	log.Append("update", "agent-1", "updated resource")
	time.Sleep(2 * time.Millisecond)
	log.Append("delete", "agent-1", "deleted resource")

	// Get all entries (zero time range)
	all := log.GetEntries(time.Time{}, time.Time{})
	if len(all) != 3 {
		t.Fatalf("expected 3 entries, got %d", len(all))
	}

	// Verify chronological order
	if all[0].Action != "create" {
		t.Errorf("first entry should be 'create', got %q", all[0].Action)
	}
	if all[1].Action != "update" {
		t.Errorf("second entry should be 'update', got %q", all[1].Action)
	}
	if all[2].Action != "delete" {
		t.Errorf("third entry should be 'delete', got %q", all[2].Action)
	}

	// Verify hash chain integrity
	if err := VerifyChain(all); err != nil {
		t.Fatalf("retrieved entries should form valid chain: %v", err)
	}

	// Test time range filtering: get only the middle entry
	from := all[0].Timestamp.Add(time.Millisecond)
	to := all[2].Timestamp.Add(-time.Millisecond)
	middle := log.GetEntries(from, to)
	if len(middle) != 1 {
		t.Fatalf("expected 1 entry in time range, got %d", len(middle))
	}
	if middle[0].Action != "update" {
		t.Errorf("filtered entry should be 'update', got %q", middle[0].Action)
	}

	// Test "from only" — get entries after the first
	afterFirst := log.GetEntries(all[0].Timestamp.Add(time.Millisecond), time.Time{})
	if len(afterFirst) != 2 {
		t.Fatalf("expected 2 entries after first, got %d", len(afterFirst))
	}

	// Test "to only" — get entries up to and including the second
	upToSecond := log.GetEntries(time.Time{}, all[1].Timestamp)
	if len(upToSecond) != 2 {
		t.Fatalf("expected 2 entries up to second, got %d", len(upToSecond))
	}

	// Verify the DB file was created
	dbPath := filepath.Join(dir, "audit.db")
	if _, err := os.Stat(dbPath); os.IsNotExist(err) {
		t.Error("expected audit.db to be created")
	}
}
