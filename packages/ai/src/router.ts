// ---------------------------------------------------------------------------
// ModelRouter
// ---------------------------------------------------------------------------

/**
 * Classifies the type of AI task being performed.
 */
export enum ModelCategory {
  Coding = "coding",
  Planning = "planning",
  Review = "review",
  Answer = "answer",
  Explore = "explore",
  Creative = "creative",
  Analysis = "analysis",
  General = "general",
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
