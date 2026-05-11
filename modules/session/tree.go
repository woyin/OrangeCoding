package session

import "github.com/woyin/OrangeCoding/modules/core"

// SessionTree tracks parent-child relationships between sessions,
// enabling session branching (forking).
type SessionTree struct {
	parentOf  map[core.SessionId]core.SessionId   // child -> parent
	childrenOf map[core.SessionId][]core.SessionId // parent -> children
}

// NewSessionTree creates an empty session tree.
func NewSessionTree() *SessionTree {
	return &SessionTree{
		parentOf:   make(map[core.SessionId]core.SessionId),
		childrenOf: make(map[core.SessionId][]core.SessionId),
	}
}

// Fork registers a parent-child relationship between two sessions.
func (t *SessionTree) Fork(parentID, childID core.SessionId) {
	t.parentOf[childID] = parentID
	t.childrenOf[parentID] = append(t.childrenOf[parentID], childID)
}

// GetChildren returns all child session IDs for the given parent.
// Returns an empty (non-nil) slice if the session has no children.
func (t *SessionTree) GetChildren(id core.SessionId) []core.SessionId {
	children, ok := t.childrenOf[id]
	if !ok {
		return []core.SessionId{}
	}
	return children
}

// GetParent returns the parent session ID and true if the given session has a parent.
// Returns a zero SessionId and false if the session is a root (no parent).
func (t *SessionTree) GetParent(id core.SessionId) (core.SessionId, bool) {
	parent, ok := t.parentOf[id]
	return parent, ok
}
