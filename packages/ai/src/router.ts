// ---------------------------------------------------------------------------
// ModelRouter
// ---------------------------------------------------------------------------

/**
 * Classifies the type of AI task being performed.
 *
 * Standard categories cover common agent operations.
 * OmO-style categories (Quick, Deep, Visual, Ultrabrain) provide
 * intent-driven routing where the user picks a mode, not a model.
 */
export enum ModelCategory {
  // Standard categories
  Coding = "coding",
  Planning = "planning",
  Review = "review",
  Answer = "answer",
  Explore = "explore",
  Creative = "creative",
  Analysis = "analysis",
  General = "general",

  // OmO-style intent categories
  /** Single-file changes, typos, quick fixes — fast + cheap model */
  Quick = "quick",
  /** Autonomous research + execution — strong reasoning model */
  Deep = "deep",
  /** Frontend, UI/UX, design — vision-capable model */
  Visual = "visual",
  /** Hard logic, architecture decisions — strongest available model */
  Ultrabrain = "ultrabrain",
}

/**
 * Maps a ModelCategory to a specific provider and model.
 */
export interface RoutingRule {
  category: ModelCategory;
  provider: string;
  model: string;
}

/**
 * Routes model categories to specific provider+model combinations.
 *
 * Supports both standard categories and OmO-style intent categories.
 * The router picks the best model for the task type automatically.
 */
export class ModelRouter {
  private readonly rules: RoutingRule[];
  private readonly defaultRule: RoutingRule;

  /**
   * Creates a new ModelRouter with the given rules.
   * If no rule matches, the first rule is used as the default.
   * If no rules are provided, a built-in default is used.
   */
  constructor(rules: RoutingRule[]) {
    const defaultRule: RoutingRule = {
      category: ModelCategory.General,
      provider: "openai",
      model: "gpt-4",
    };
    if (rules.length > 0) {
      this.defaultRule = rules[0]!;
    } else {
      this.defaultRule = defaultRule;
    }
    this.rules = rules;
  }

  /**
   * Returns the provider and model for the given category.
   * If no exact match is found, the default rule is used.
   */
  route(category: ModelCategory): { provider: string; model: string } {
    for (const rule of this.rules) {
      if (rule.category === category) {
        return { provider: rule.provider, model: rule.model };
      }
    }
    return { provider: this.defaultRule.provider, model: this.defaultRule.model };
  }
}

/**
 * Create a ModelRouter with OmO-style default category mappings.
 * Users only pick a category (quick/deep/visual/ultrabrain), the router
 * selects the appropriate provider and model.
 */
export function createOmORouter(overrides?: Partial<Record<ModelCategory, { provider: string; model: string }>>): ModelRouter {
  const defaults: RoutingRule[] = [
    { category: ModelCategory.Quick, provider: "anthropic", model: "claude-sonnet-4-6" },
    { category: ModelCategory.Deep, provider: "openai", model: "gpt-5.1" },
    { category: ModelCategory.Visual, provider: "openai", model: "gpt-5.1" },
    { category: ModelCategory.Ultrabrain, provider: "anthropic", model: "claude-opus-4-7" },
    { category: ModelCategory.Coding, provider: "anthropic", model: "claude-opus-4-7" },
    { category: ModelCategory.Planning, provider: "anthropic", model: "claude-opus-4-7" },
    { category: ModelCategory.Review, provider: "openai", model: "gpt-5.1" },
    { category: ModelCategory.Explore, provider: "anthropic", model: "claude-sonnet-4-6" },
    { category: ModelCategory.Analysis, provider: "openai", model: "gpt-5.1" },
    { category: ModelCategory.Answer, provider: "anthropic", model: "claude-sonnet-4-6" },
    { category: ModelCategory.Creative, provider: "anthropic", model: "claude-opus-4-7" },
    { category: ModelCategory.General, provider: "anthropic", model: "claude-opus-4-7" },
  ];

  if (overrides) {
    for (const rule of defaults) {
      const ov = overrides[rule.category];
      if (ov) {
        rule.provider = ov.provider;
        rule.model = ov.model;
      }
    }
  }

  return new ModelRouter(defaults);
}
