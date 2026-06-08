/**
 * TTSR (Regex-Triggered Streaming Rule Injection) checks content against a set
 * of regex patterns and returns matching rules.
 * Ported from modules/agent/ttsr.go.
 */

export interface Rule {
  pattern: RegExp;
  rule: string;
}

export class TTSR {
  private _rules: Rule[];

  constructor(rules: Rule[]) {
    this._rules = rules;
  }

  /** Check tests the content against all rules and returns the rule strings
   *  for every pattern that matches. */
  check(content: string): string[] {
    const matches: string[] = [];
    for (const r of this._rules) {
      if (r.pattern.test(content)) {
        matches.push(r.rule);
      }
    }
    return matches;
  }
}
