import { BudgetGuard } from "../budget.js";
import { AgentId } from "@orangecoding/core";

describe("BudgetGuard", () => {
  let guard: BudgetGuard;
  let agentId: AgentId;

  beforeEach(() => {
    guard = new BudgetGuard();
    agentId = AgentId.create();
  });

  describe("setBudget", () => {
    it("sets budget for an agent", () => {
      guard.setBudget(agentId, { maxCalls: 10, maxTokens: 1000, maxWallTimeMs: 60000 });
      expect(guard).toBeDefined();
    });
  });

  describe("check", () => {
    it("returns [true, ''] when no budget is set", () => {
      const [ok, reason] = guard.check(agentId);
      expect(ok).toBe(true);
      expect(reason).toBe("");
    });

    it("returns [true, ''] when within budget", () => {
      guard.setBudget(agentId, { maxCalls: 5, maxTokens: 1000, maxWallTimeMs: 60000 });
      const [ok, reason] = guard.check(agentId);
      expect(ok).toBe(true);
      expect(reason).toBe("");
    });

    it("allows exactly maxCalls successful calls, then denies", () => {
      guard.setBudget(agentId, { maxCalls: 3, maxTokens: 1000, maxWallTimeMs: 60000 });

      // check() increments AFTER passing, so calls 0,1,2 pass; call 3 fails
      expect(guard.check(agentId)[0]).toBe(true);  // calls=0 -> pass, calls=1
      expect(guard.check(agentId)[0]).toBe(true);  // calls=1 -> pass, calls=2
      expect(guard.check(agentId)[0]).toBe(true);  // calls=2 -> pass, calls=3
      expect(guard.check(agentId)[0]).toBe(false); // calls=3 >= 3 -> deny
    });

    it("returns [false, reason] when maxCalls exceeded", () => {
      guard.setBudget(agentId, { maxCalls: 1, maxTokens: 0, maxWallTimeMs: 0 });

      guard.check(agentId); // call 0 -> pass, calls=1
      const [ok, reason] = guard.check(agentId); // call 1 >= 1 -> deny
      expect(ok).toBe(false);
      expect(reason).toContain("call budget exceeded");
      expect(reason).toContain("1/1");
    });

    it("handles unlimited calls when maxCalls is 0", () => {
      guard.setBudget(agentId, { maxCalls: 0, maxTokens: 0, maxWallTimeMs: 0 });

      for (let i = 0; i < 100; i++) {
        const [ok] = guard.check(agentId);
        expect(ok).toBe(true);
      }
    });

    it("tracks budgets independently per agent", () => {
      const agent1 = AgentId.create();
      const agent2 = AgentId.create();

      guard.setBudget(agent1, { maxCalls: 1, maxTokens: 0, maxWallTimeMs: 0 });
      guard.setBudget(agent2, { maxCalls: 5, maxTokens: 0, maxWallTimeMs: 0 });

      guard.check(agent1); // agent1 call 0 -> pass
      expect(guard.check(agent1)[0]).toBe(false); // agent1 call 1 >= 1 -> deny

      expect(guard.check(agent2)[0]).toBe(true); // agent2 call 0 -> pass
    });
  });
});
