package agent

import (
	"context"
	"fmt"
	"regexp"
	"strings"
)

var factLinePattern = regexp.MustCompile(`(?m)^\s*FACT:\s*(.+?)\s*$`)

// HarnessMemoryManager recalls and learns stable facts for context assembly.
type HarnessMemoryManager struct {
	store *MemoryStore
}

// NewHarnessMemoryManager creates a memory manager backed by MemoryStore.
func NewHarnessMemoryManager(store *MemoryStore) *HarnessMemoryManager {
	return &HarnessMemoryManager{store: store}
}

// Recall returns memory blocks that match the task query.
func (m *HarnessMemoryManager) Recall(ctx context.Context, query string) ([]ContextBlock, error) {
	if m == nil || m.store == nil {
		return nil, nil
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}

	keys, err := m.store.List()
	if err != nil {
		return nil, err
	}
	queryTerms := splitRecallTerms(query)
	var blocks []ContextBlock
	for _, key := range keys {
		value, err := m.store.Read(key)
		if err != nil {
			return nil, err
		}
		if memoryMatches(key, value, queryTerms) {
			blocks = append(blocks, newContextBlock(
				ContextBlockMemory,
				fmt.Sprintf("Memory[%s]: %s", key, value),
				true,
				80,
			))
		}
	}
	return blocks, nil
}

// LearnObservation extracts FACT lines from observations and stores them.
func (m *HarnessMemoryManager) LearnObservation(ctx context.Context, observation string) error {
	if m == nil || m.store == nil {
		return nil
	}
	if err := ctx.Err(); err != nil {
		return err
	}

	matches := factLinePattern.FindAllStringSubmatch(observation, -1)
	for _, match := range matches {
		fact := strings.TrimSpace(match[1])
		if fact == "" {
			continue
		}
		key := memoryKeyForFact(fact)
		if err := m.store.Write(key, fact); err != nil {
			return err
		}
	}
	return nil
}

func splitRecallTerms(query string) []string {
	fields := strings.FieldsFunc(strings.ToLower(query), func(r rune) bool {
		return r == ' ' || r == '\t' || r == '\n' || r == ',' || r == '，' || r == ':' || r == '：'
	})
	var terms []string
	for _, field := range fields {
		field = strings.TrimSpace(field)
		if len([]rune(field)) >= 2 {
			terms = append(terms, field)
		}
	}
	return terms
}

func memoryMatches(key, value string, terms []string) bool {
	if len(terms) == 0 {
		return true
	}
	text := strings.ToLower(key + "\n" + value)
	for _, term := range terms {
		if strings.Contains(text, term) {
			return true
		}
	}
	return false
}

func memoryKeyForFact(fact string) string {
	sanitized := strings.ToLower(fact)
	replacer := strings.NewReplacer(" ", "-", "\t", "-", "\n", "-", "/", "-", "\\", "-", ":", "-", "：", "-", "，", "-", ",", "-")
	sanitized = replacer.Replace(sanitized)
	runes := []rune(sanitized)
	if len(runes) > 32 {
		runes = runes[:32]
	}
	return "fact-" + strings.Trim(string(runes), "-")
}
