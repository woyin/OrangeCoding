package ai

// ---------------------------------------------------------------------------
// ModelRouter
// ---------------------------------------------------------------------------

// ModelCategory classifies the type of AI task being performed.
type ModelCategory int

const (
	CategoryCoding   ModelCategory = iota // code generation/editing
	CategoryPlanning                       // task planning and decomposition
	CategoryReview                         // code review and analysis
	CategoryAnswer                         // question answering
	CategoryExplore                        // exploration and research
	CategoryCreative                       // creative writing
	CategoryAnalysis                       // data analysis
	CategoryGeneral                        // general-purpose tasks
)

// String returns the human-readable name of the model category.
func (c ModelCategory) String() string {
	switch c {
	case CategoryCoding:
		return "coding"
	case CategoryPlanning:
		return "planning"
	case CategoryReview:
		return "review"
	case CategoryAnswer:
		return "answer"
	case CategoryExplore:
		return "explore"
	case CategoryCreative:
		return "creative"
	case CategoryAnalysis:
		return "analysis"
	case CategoryGeneral:
		return "general"
	default:
		return "unknown"
	}
}

// RoutingRule maps a ModelCategory to a specific provider and model.
type RoutingRule struct {
	Category ModelCategory
	Provider string
	Model    string
}

// ModelRouter routes model categories to specific provider+model combinations.
type ModelRouter struct {
	rules   []RoutingRule
	defaultRule RoutingRule
}

// NewModelRouter creates a new ModelRouter with the given rules.
// If no rule matches, the first rule is used as the default.
// If no rules are provided, a built-in default is used.
func NewModelRouter(rules []RoutingRule) *ModelRouter {
	defaultRule := RoutingRule{
		Category: CategoryGeneral,
		Provider: "openai",
		Model:    "gpt-4",
	}
	if len(rules) > 0 {
		defaultRule = rules[0]
	}
	return &ModelRouter{
		rules:       rules,
		defaultRule: defaultRule,
	}
}

// Route returns the provider and model for the given category.
// If no exact match is found, the default rule is used.
func (r *ModelRouter) Route(category ModelCategory) (string, string) {
	for _, rule := range r.rules {
		if rule.Category == category {
			return rule.Provider, rule.Model
		}
	}
	return r.defaultRule.Provider, r.defaultRule.Model
}
