package ai

import (
	"context"
	"sync"
	"time"
)

// ---------------------------------------------------------------------------
// FallbackChain
// ---------------------------------------------------------------------------

// providerEntry holds a provider and its cooldown state.
type providerEntry struct {
	provider   AiProvider
	coolUntil  time.Time
	lastErr    error
}

// FallbackChain tries multiple providers in order, falling back on failure.
// Providers that recently failed are put on cooldown and skipped.
type FallbackChain struct {
	mu       sync.Mutex
	entries  []*providerEntry
	cooldown time.Duration
}

// NewFallbackChain creates a new FallbackChain with the given providers and
// cooldown duration. Providers are tried in slice order.
func NewFallbackChain(providers []AiProvider, cooldown time.Duration) *FallbackChain {
	entries := make([]*providerEntry, len(providers))
	for i, p := range providers {
		entries[i] = &providerEntry{provider: p}
	}
	return &FallbackChain{
		entries:  entries,
		cooldown: cooldown,
	}
}

// ChatCompletion tries providers in order until one succeeds.
// Providers on cooldown are skipped. On failure, the provider is put on cooldown.
func (c *FallbackChain) ChatCompletion(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (*AiResponse, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	var lastErr error
	for _, entry := range c.entries {
		if time.Now().Before(entry.coolUntil) {
			// Carry forward the last error from the provider that is cooling down
			if entry.lastErr != nil {
				lastErr = entry.lastErr
			}
			continue
		}

		resp, err := entry.provider.ChatCompletion(ctx, messages, tools, opts)
		if err == nil {
			return resp, nil
		}

		// Set cooldown on failure
		entry.coolUntil = time.Now().Add(c.cooldown)
		entry.lastErr = err
		lastErr = err
	}

	return nil, lastErr
}

// ChatCompletionStream tries providers in order until one succeeds.
// Providers on cooldown are skipped. On failure, the provider is put on cooldown.
func (c *FallbackChain) ChatCompletionStream(ctx context.Context, messages []ChatMessage, tools []ToolDefinition, opts ChatOptions) (<-chan StreamEvent, error) {
	c.mu.Lock()
	defer c.mu.Unlock()

	var lastErr error
	for _, entry := range c.entries {
		if time.Now().Before(entry.coolUntil) {
			if entry.lastErr != nil {
				lastErr = entry.lastErr
			}
			continue
		}

		ch, err := entry.provider.ChatCompletionStream(ctx, messages, tools, opts)
		if err == nil {
			return ch, nil
		}

		// Set cooldown on failure
		entry.coolUntil = time.Now().Add(c.cooldown)
		entry.lastErr = err
		lastErr = err
	}

	return nil, lastErr
}

// Providers returns a copy of the provider list (for testing/inspection).
func (c *FallbackChain) Providers() []AiProvider {
	c.mu.Lock()
	defer c.mu.Unlock()

	result := make([]AiProvider, len(c.entries))
	for i, entry := range c.entries {
		result[i] = entry.provider
	}
	return result
}

// IsCoolingDown returns true if the provider at the given index is on cooldown.
func (c *FallbackChain) IsCoolingDown(index int) bool {
	c.mu.Lock()
	defer c.mu.Unlock()

	if index < 0 || index >= len(c.entries) {
		return false
	}
	return time.Now().Before(c.entries[index].coolUntil)
}
