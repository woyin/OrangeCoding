import { AgentRegistry } from "../registry.js";
import { AgentId, AgentRole, AgentStatus } from "@orangecoding/core";
import type { AgentInfo } from "../registry.js";

describe("AgentRegistry", () => {
  let registry: AgentRegistry;

  beforeEach(() => {
    registry = new AgentRegistry();
  });

  function makeAgentInfo(overrides: Partial<AgentInfo> = {}): AgentInfo {
    return {
      id: AgentId.create(),
      role: AgentRole.Executor,
      capabilities: [],
      status: AgentStatus.Idle,
      ...overrides,
    };
  }

  describe("register and get", () => {
    it("registers and retrieves an agent by ID", () => {
      const info = makeAgentInfo();
      registry.register(info);
      expect(registry.get(info.id)).toEqual(info);
    });

    it("returns undefined for unknown ID", () => {
      const unknown = AgentId.create();
      expect(registry.get(unknown)).toBeUndefined();
    });

    it("accepts string IDs for lookup", () => {
      const info = makeAgentInfo();
      registry.register(info);
      expect(registry.get(info.id.toString())).toEqual(info);
    });

    it("overwrites an existing registration with the same ID", () => {
      const id = AgentId.create();
      const info1 = makeAgentInfo({ id, status: AgentStatus.Idle });
      const info2 = makeAgentInfo({ id, status: AgentStatus.Running });
      registry.register(info1);
      registry.register(info2);
      expect(registry.get(id)!.status).toBe(AgentStatus.Running);
    });
  });

  describe("unregister", () => {
    it("removes a registered agent", () => {
      const info = makeAgentInfo();
      registry.register(info);
      registry.unregister(info.id);
      expect(registry.get(info.id)).toBeUndefined();
    });

    it("is a no-op for non-existent agent", () => {
      expect(() => registry.unregister(AgentId.create())).not.toThrow();
    });
  });

  describe("findByRole", () => {
    it("returns agents matching the given role", () => {
      const coder = makeAgentInfo({ role: AgentRole.Coder });
      const executor = makeAgentInfo({ role: AgentRole.Executor });
      registry.register(coder);
      registry.register(executor);

      const coders = registry.findByRole(AgentRole.Coder);
      expect(coders).toHaveLength(1);
      expect(coders[0]!.id.toString()).toBe(coder.id.toString());
    });

    it("returns empty array when no agents match", () => {
      registry.register(makeAgentInfo({ role: AgentRole.Executor }));
      expect(registry.findByRole(AgentRole.Planner)).toHaveLength(0);
    });
  });

  describe("findByCapability", () => {
    it("returns agents that have the given capability", () => {
      const info = makeAgentInfo({
        capabilities: [
          { name: "code", description: "coding", supportedTools: [] },
          { name: "debug", description: "debugging", supportedTools: [] },
        ],
      });
      registry.register(info);

      const result = registry.findByCapability("code");
      expect(result).toHaveLength(1);
      expect(result[0]!.id.toString()).toBe(info.id.toString());
    });

    it("returns empty array when no agents have the capability", () => {
      registry.register(makeAgentInfo());
      expect(registry.findByCapability("nonexistent")).toHaveLength(0);
    });
  });
});
