package agent

import (
	"context"
	"fmt"
	"strings"

	"github.com/woyin/OrangeCoding/modules/core"
)

// ContextBlockKind identifies the role of a block in the model context.
type ContextBlockKind string

const (
	ContextBlockSystem       ContextBlockKind = "system"
	ContextBlockHarness      ContextBlockKind = "harness"
	ContextBlockTask         ContextBlockKind = "task"
	ContextBlockMemory       ContextBlockKind = "memory"
	ContextBlockConversation ContextBlockKind = "conversation"
	ContextBlockToolResult   ContextBlockKind = "tool_result"
)

// ContextBlock is an independently budgeted chunk of model context.
type ContextBlock struct {
	Kind          ContextBlockKind `json:"kind"`
	Content       string           `json:"content"`
	Stable        bool             `json:"stable"`
	Priority      int              `json:"priority"`
	TokenEstimate int              `json:"token_estimate"`
}

// HarnessContextConfig controls context assembly.
type HarnessContextConfig struct {
	MaxTokens       int
	RecentMessages  int
	MemoryMaxBlocks int
}

// HarnessContextInput provides the data needed to build model context.
type HarnessContextInput struct {
	SystemPrompt  string
	Task          string
	Conversation  *core.Conversation
	MemoryManager *HarnessMemoryManager
}

// HarnessContextBuilder builds stable, ordered context blocks.
type HarnessContextBuilder struct {
	config HarnessContextConfig
}

// NewHarnessContextBuilder creates a context builder with defaults.
func NewHarnessContextBuilder(config HarnessContextConfig) *HarnessContextBuilder {
	if config.MaxTokens <= 0 {
		config.MaxTokens = 24000
	}
	if config.RecentMessages <= 0 {
		config.RecentMessages = 8
	}
	if config.MemoryMaxBlocks <= 0 {
		config.MemoryMaxBlocks = 6
	}
	return &HarnessContextBuilder{config: config}
}

// Build assembles stable system/task/memory blocks followed by recent dynamic blocks.
func (b *HarnessContextBuilder) Build(ctx context.Context, input HarnessContextInput) ([]ContextBlock, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}

	blocks := []ContextBlock{
		newContextBlock(ContextBlockSystem, input.SystemPrompt, true, 100),
		newContextBlock(ContextBlockTask, fmt.Sprintf("Task: %s", input.Task), true, 90),
	}

	if input.MemoryManager != nil {
		memoryBlocks, err := input.MemoryManager.Recall(ctx, input.Task)
		if err != nil {
			return nil, err
		}
		if len(memoryBlocks) > b.config.MemoryMaxBlocks {
			memoryBlocks = memoryBlocks[:b.config.MemoryMaxBlocks]
		}
		blocks = append(blocks, memoryBlocks...)
	}

	if input.Conversation != nil {
		msgs := input.Conversation.Messages()
		start := len(msgs) - b.config.RecentMessages
		if start < 0 {
			start = 0
		}
		for _, msg := range msgs[start:] {
			blocks = append(blocks, newContextBlock(
				ContextBlockConversation,
				fmt.Sprintf("%s: %s", msg.Role.String(), msg.Content),
				false,
				20,
			))
		}
	}

	return fitContextBlocks(blocks, b.config.MaxTokens), nil
}

func newContextBlock(kind ContextBlockKind, content string, stable bool, priority int) ContextBlock {
	return ContextBlock{
		Kind:          kind,
		Content:       content,
		Stable:        stable,
		Priority:      priority,
		TokenEstimate: estimateTextTokens(content),
	}
}

func fitContextBlocks(blocks []ContextBlock, maxTokens int) []ContextBlock {
	if maxTokens <= 0 || totalBlockTokens(blocks) <= maxTokens {
		return blocks
	}

	kept := append([]ContextBlock(nil), blocks...)
	for totalBlockTokens(kept) > maxTokens {
		dropIdx := -1
		lowestPriority := int(^uint(0) >> 1)
		for i, block := range kept {
			if block.Stable {
				continue
			}
			if block.Priority < lowestPriority {
				lowestPriority = block.Priority
				dropIdx = i
			}
		}
		if dropIdx == -1 {
			break
		}
		kept = append(kept[:dropIdx], kept[dropIdx+1:]...)
	}
	return kept
}

func totalBlockTokens(blocks []ContextBlock) int {
	total := 0
	for _, block := range blocks {
		total += block.TokenEstimate
	}
	return total
}

func estimateTextTokens(text string) int {
	if text == "" {
		return 0
	}
	tokens := len([]rune(text)) / 4
	if tokens == 0 {
		return 1
	}
	return tokens
}

func containsBlockKind(blocks []ContextBlock, kind ContextBlockKind) bool {
	for _, block := range blocks {
		if block.Kind == kind {
			return true
		}
	}
	return false
}

func containsBlockText(blocks []ContextBlock, text string) bool {
	for _, block := range blocks {
		if strings.Contains(block.Content, text) {
			return true
		}
	}
	return false
}
