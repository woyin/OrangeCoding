package mesh

import (
	"sync"

	"github.com/woyin/OrangeCoding/modules/core"
)

// AgentInfo holds metadata about a registered agent.
type AgentInfo struct {
	ID           core.AgentId
	Role         core.AgentRole
	Capabilities []core.AgentCapability
	Status       core.AgentStatus
}

// AgentRegistry maintains a thread-safe mapping of agent IDs to their info.
type AgentRegistry struct {
	mu     sync.RWMutex
	agents map[core.AgentId]AgentInfo
}

// NewAgentRegistry creates a new empty AgentRegistry.
func NewAgentRegistry() *AgentRegistry {
	return &AgentRegistry{
		agents: make(map[core.AgentId]AgentInfo),
	}
}

// Register adds or replaces the entry for the given agent.
func (r *AgentRegistry) Register(info AgentInfo) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.agents[info.ID] = info
}

// Unregister removes the agent with the given ID. If the agent does not exist,
// Unregister is a no-op.
func (r *AgentRegistry) Unregister(id core.AgentId) {
	r.mu.Lock()
	defer r.mu.Unlock()
	delete(r.agents, id)
}

// Get returns the AgentInfo for the given ID and true if found, or a zero
// AgentInfo and false otherwise.
func (r *AgentRegistry) Get(id core.AgentId) (AgentInfo, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	info, ok := r.agents[id]
	return info, ok
}

// FindByRole returns all agents whose Role matches the given role.
func (r *AgentRegistry) FindByRole(role core.AgentRole) []AgentInfo {
	r.mu.RLock()
	defer r.mu.RUnlock()

	var result []AgentInfo
	for _, info := range r.agents {
		if info.Role == role {
			result = append(result, info)
		}
	}
	return result
}

// FindByCapability returns all agents that have a capability with the given
// name.
func (r *AgentRegistry) FindByCapability(cap string) []AgentInfo {
	r.mu.RLock()
	defer r.mu.RUnlock()

	var result []AgentInfo
	for _, info := range r.agents {
		for _, c := range info.Capabilities {
			if c.Name == cap {
				result = append(result, info)
				break
			}
		}
	}
	return result
}
