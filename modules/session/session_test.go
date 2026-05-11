package session

import (
	"os"
	"path/filepath"
	"sort"
	"testing"
	"time"

	"github.com/woyin/OrangeCoding/modules/core"
)

// --- Session ---

func TestSessionCreate(t *testing.T) {
	dir := t.TempDir()
	mgr := NewSessionManager(dir)
	s := mgr.Create()

	if s.ID == (core.SessionId{}) {
		t.Fatal("expected non-zero SessionId")
	}
	if s.CreatedAt.IsZero() {
		t.Fatal("expected non-zero CreatedAt")
	}
	if s.UpdatedAt.IsZero() {
		t.Fatal("expected non-zero UpdatedAt")
	}
	if len(s.Messages) != 0 {
		t.Fatalf("expected empty messages, got %d", len(s.Messages))
	}
	if s.ParentID != nil {
		t.Fatal("expected nil ParentID for new session")
	}
	if s.Metadata == nil {
		t.Fatal("expected non-nil Metadata map")
	}
}

// --- SessionManager CRUD ---

func TestSessionManagerCRUD(t *testing.T) {
	dir := t.TempDir()
	mgr := NewSessionManager(dir)

	// Create
	s := mgr.Create()
	id := s.ID

	// Save to disk
	if err := mgr.Update(s); err != nil {
		t.Fatalf("Update error: %v", err)
	}

	// Get
	got, err := mgr.Get(id)
	if err != nil {
		t.Fatalf("Get error: %v", err)
	}
	if got.ID != id {
		t.Fatalf("Get ID mismatch: got %v, want %v", got.ID, id)
	}

	// Update: add a message
	msg := core.NewUserMessage("hello")
	got.Messages = append(got.Messages, msg)
	originalUpdatedAt := got.UpdatedAt
	time.Sleep(10 * time.Millisecond) // ensure UpdatedAt changes
	if err := mgr.Update(got); err != nil {
		t.Fatalf("Update with message error: %v", err)
	}

	// Verify message persisted
	got2, err := mgr.Get(id)
	if err != nil {
		t.Fatalf("Get after update error: %v", err)
	}
	if len(got2.Messages) != 1 {
		t.Fatalf("expected 1 message, got %d", len(got2.Messages))
	}
	if got2.Messages[0].Content != "hello" {
		t.Fatalf("message content: got %q, want %q", got2.Messages[0].Content, "hello")
	}
	if !got2.UpdatedAt.After(originalUpdatedAt) {
		t.Fatalf("UpdatedAt should advance: before=%v, after=%v", originalUpdatedAt, got2.UpdatedAt)
	}

	// Delete
	if err := mgr.Delete(id); err != nil {
		t.Fatalf("Delete error: %v", err)
	}

	// Verify deleted
	_, err = mgr.Get(id)
	if err == nil {
		t.Fatal("expected error after Delete, got nil")
	}
}

// --- SessionManager List ---

func TestSessionManagerList(t *testing.T) {
	dir := t.TempDir()
	mgr := NewSessionManager(dir)

	// Create 3 sessions with staggered updates
	s1 := mgr.Create()
	mgr.Update(s1)

	time.Sleep(10 * time.Millisecond)
	s2 := mgr.Create()
	mgr.Update(s2)

	time.Sleep(10 * time.Millisecond)
	s3 := mgr.Create()
	mgr.Update(s3)

	sessions, err := mgr.List()
	if err != nil {
		t.Fatalf("List error: %v", err)
	}
	if len(sessions) != 3 {
		t.Fatalf("expected 3 sessions, got %d", len(sessions))
	}

	// Should be sorted by UpdatedAt descending (most recent first)
	if !sort.SliceIsSorted(sessions, func(i, j int) bool {
		return sessions[i].UpdatedAt.After(sessions[j].UpdatedAt)
	}) {
		t.Fatal("sessions not sorted by UpdatedAt descending")
	}
}

// --- JSONL Storage ---

func TestJSONLStorage(t *testing.T) {
	dir := t.TempDir()

	s := &Session{
		ID:        core.NewSessionId(),
		Messages:  []core.Message{},
		Metadata:  map[string]string{},
		TokenUsage: core.TokenUsage{},
		CreatedAt: time.Now().UTC(),
		UpdatedAt: time.Now().UTC(),
	}

	// Add messages
	s.Messages = append(s.Messages, core.NewSystemMessage("system prompt"))
	s.Messages = append(s.Messages, core.NewUserMessage("user input"))
	s.Messages = append(s.Messages, core.NewAssistantMessage("assistant reply"))

	// Write
	if err := WriteSession(dir, s); err != nil {
		t.Fatalf("WriteSession error: %v", err)
	}

	// Read back
	got, err := ReadSession(dir, s.ID)
	if err != nil {
		t.Fatalf("ReadSession error: %v", err)
	}

	if got.ID != s.ID {
		t.Fatalf("ID mismatch: got %v, want %v", got.ID, s.ID)
	}
	if len(got.Messages) != 3 {
		t.Fatalf("message count: got %d, want 3", len(got.Messages))
	}
	for i, msg := range got.Messages {
		if msg.Content != s.Messages[i].Content {
			t.Errorf("message[%d] content: got %q, want %q", i, msg.Content, s.Messages[i].Content)
		}
	}
}

func TestJSONLStorageFilePath(t *testing.T) {
	dir := t.TempDir()
	id := core.NewSessionId()
	s := &Session{
		ID:        id,
		Messages:  []core.Message{},
		Metadata:  map[string]string{},
		CreatedAt: time.Now().UTC(),
		UpdatedAt: time.Now().UTC(),
	}
	if err := WriteSession(dir, s); err != nil {
		t.Fatalf("WriteSession error: %v", err)
	}

	expected := filepath.Join(dir, id.String()+".jsonl")
	if _, err := os.Stat(expected); os.IsNotExist(err) {
		t.Fatalf("expected file %q to exist", expected)
	}
}

// --- SessionTree ---

func TestSessionTreeFork(t *testing.T) {
	tree := NewSessionTree()
	parentID := core.NewSessionId()
	childID := core.NewSessionId()

	tree.Fork(parentID, childID)

	// GetChildren
	children := tree.GetChildren(parentID)
	if len(children) != 1 {
		t.Fatalf("expected 1 child, got %d", len(children))
	}
	if children[0] != childID {
		t.Fatalf("child ID: got %v, want %v", children[0], childID)
	}

	// GetParent
	p, ok := tree.GetParent(childID)
	if !ok {
		t.Fatal("expected to find parent")
	}
	if p != parentID {
		t.Fatalf("parent ID: got %v, want %v", p, parentID)
	}

	// No parent for root
	_, ok = tree.GetParent(parentID)
	if ok {
		t.Fatal("root should have no parent")
	}

	// No children for leaf
	leafChildren := tree.GetChildren(childID)
	if len(leafChildren) != 0 {
		t.Fatalf("leaf should have no children, got %d", len(leafChildren))
	}
}

func TestSessionTreeMultipleChildren(t *testing.T) {
	tree := NewSessionTree()
	parentID := core.NewSessionId()
	child1 := core.NewSessionId()
	child2 := core.NewSessionId()

	tree.Fork(parentID, child1)
	tree.Fork(parentID, child2)

	children := tree.GetChildren(parentID)
	if len(children) != 2 {
		t.Fatalf("expected 2 children, got %d", len(children))
	}
}

// --- BlobStore ---

func TestBlobStorePutGet(t *testing.T) {
	dir := t.TempDir()
	store := NewBlobStore(dir)

	data := []byte("hello blob store")
	hash, err := store.Put(data)
	if err != nil {
		t.Fatalf("Put error: %v", err)
	}
	if len(hash) != 64 { // SHA-256 hex = 64 chars
		t.Fatalf("hash length: got %d, want 64", len(hash))
	}

	got, err := store.Get(hash)
	if err != nil {
		t.Fatalf("Get error: %v", err)
	}
	if string(got) != string(data) {
		t.Fatalf("Get content: got %q, want %q", string(got), string(data))
	}
}

func TestBlobStorePutIdempotent(t *testing.T) {
	dir := t.TempDir()
	store := NewBlobStore(dir)

	data := []byte("idempotent test")
	hash1, err := store.Put(data)
	if err != nil {
		t.Fatalf("Put 1 error: %v", err)
	}
	hash2, err := store.Put(data)
	if err != nil {
		t.Fatalf("Put 2 error: %v", err)
	}

	if hash1 != hash2 {
		t.Fatalf("hashes differ: %s vs %s", hash1, hash2)
	}

	// Count files in dir
	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatalf("ReadDir error: %v", err)
	}
	if len(entries) != 1 {
		t.Fatalf("expected 1 file, got %d", len(entries))
	}
}

func TestBlobStoreGetNotFound(t *testing.T) {
	dir := t.TempDir()
	store := NewBlobStore(dir)

	_, err := store.Get("nonexistenthash000000000000000000000000000000000000000000000000")
	if err == nil {
		t.Fatal("expected error for non-existent hash, got nil")
	}
}

func TestBlobStoreHas(t *testing.T) {
	dir := t.TempDir()
	store := NewBlobStore(dir)

	data := []byte("check existence")
	hash, _ := store.Put(data)

	if !store.Has(hash) {
		t.Fatal("expected Has to return true for existing blob")
	}
	if store.Has("nonexistenthash00000000000000000000000000000000000000000000000") {
		t.Fatal("expected Has to return false for missing blob")
	}
}
