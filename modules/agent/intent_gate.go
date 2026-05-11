package agent

import "strings"

// IntentCategory classifies the type of user intent.
type IntentCategory string

const (
	IntentCoding   IntentCategory = "coding"
	IntentPlanning IntentCategory = "planning"
	IntentReview   IntentCategory = "review"
	IntentQuestion IntentCategory = "question"
	IntentExplore  IntentCategory = "explore"
	IntentGeneral  IntentCategory = "general"
)

// IntentGate classifies user input into an intent category using keyword matching.
type IntentGate struct{}

// NewIntentGate creates a new IntentGate.
func NewIntentGate() *IntentGate {
	return &IntentGate{}
}

// Classify returns the intent category for the given input string.
// Classification uses keyword matching in the following priority order:
//   - coding: code, file, function, implement, debug, fix, write, create, refactor
//   - planning: plan, design, architecture, roadmap, strategy
//   - review: review, check, inspect, audit, verify
//   - question: what, how, why, when, where, who, which
//   - explore: find, search, explore, locate, list, show
//   - general: default fallback
func (g *IntentGate) Classify(input string) IntentCategory {
	lower := strings.ToLower(input)

	// Check coding keywords
	codingKeywords := []string{"code", "file", "function", "implement", "debug", "fix", "write", "create", "refactor"}
	for _, kw := range codingKeywords {
		if strings.Contains(lower, kw) {
			return IntentCoding
		}
	}

	// Check planning keywords
	planningKeywords := []string{"plan", "design", "architecture", "roadmap", "strategy"}
	for _, kw := range planningKeywords {
		if strings.Contains(lower, kw) {
			return IntentPlanning
		}
	}

	// Check review keywords
	reviewKeywords := []string{"review", "check", "inspect", "audit", "verify"}
	for _, kw := range reviewKeywords {
		if strings.Contains(lower, kw) {
			return IntentReview
		}
	}

	// Check question keywords
	questionKeywords := []string{"what", "how", "why", "when", "where", "who", "which"}
	for _, kw := range questionKeywords {
		if strings.Contains(lower, kw) {
			return IntentQuestion
		}
	}

	// Check explore keywords
	exploreKeywords := []string{"find", "search", "explore", "locate", "list", "show"}
	for _, kw := range exploreKeywords {
		if strings.Contains(lower, kw) {
			return IntentExplore
		}
	}

	return IntentGeneral
}
