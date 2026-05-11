package agent

import "regexp"

// Rule associates a regex pattern with a rule string to inject when matched.
type Rule struct {
	Pattern *regexp.Regexp
	Rule    string
}

// TTSR (Regex-Triggered Streaming Rule Injection) checks content against a set
// of regex patterns and returns matching rules.
type TTSR struct {
	rules []Rule
}

// NewTTSR creates a new TTSR with the given rules.
func NewTTSR(rules []Rule) *TTSR {
	return &TTSR{rules: rules}
}

// Check tests the content against all rules and returns the rule strings
// for every pattern that matches.
func (t *TTSR) Check(content string) []string {
	var matches []string
	for _, r := range t.rules {
		if r.Pattern.MatchString(content) {
			matches = append(matches, r.Rule)
		}
	}
	return matches
}
