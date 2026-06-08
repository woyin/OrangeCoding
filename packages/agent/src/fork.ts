/**
 * ForkAgent creates a child agent that runs with a restricted tool subset
 * on a cloned context.
 * Ported from modules/agent/fork.go.
 */

import { AgentId, SessionId, Conversation, newSystemMessage, newUserMessage } from "@orangecoding/core";
import type { AiProvider, ChatOptions, ToolDefinition } from "@orangecoding/ai";
import type { ToolExecutor } from "./executor.js";
import { filteredRegistry } from "./executor.js";
import { AgentLoop, defaultLoopConfig, type AgentLoopResult } from "./loop.js";
import type { AgentContext } from "./context.js";
import { buildToolDefinitions } from "./tool-defs.js";

export class ForkAgent {
  private _parent: AgentLoop;
  private _allowedTools: string[];

  constructor(parent: AgentLoop, allowedTools: string[]) {
    this._parent = parent;
    this._allowedTools = allowedTools;
  }

  /** Run clones the parent context, creates a filtered tool registry, and runs a
   *  new AgentLoop for the given task. */
  async run(task: string): Promise<AgentLoopResult> {
    // Clone the parent's context by copying the conversation
    const parentCtx = this._parent.context;
    const clonedConv = Conversation.create();
    for (const m of parentCtx.conversation.messages()) {
      clonedConv.addMessage(m);
    }

    const forkCtx = new (await import("./context.js")).AgentContext(
      parentCtx.sessionID,
      parentCtx.workDir,
    );
    // Replace the default conversation with the cloned one
    // We need to directly build the context properly
    const AgentContextModule = await import("./context.js");

    // Set task as user message by adding messages to cloned conv
    clonedConv.addMessage(newUserMessage(task));

    // Create filtered tool registry
    const fRegistry = filteredRegistry(this._parent.executor.registry, this._allowedTools);
    const { ToolExecutor } = await import("./executor.js");
    const forkExecutor = new ToolExecutor(fRegistry);

    // Filter tool definitions
    const filteredDefs: ToolDefinition[] = [];
    for (const td of this._parent.toolDefs) {
      for (const name of this._allowedTools) {
        if (td.function.name === name) {
          filteredDefs.push(td);
          break;
        }
      }
    }

    // Build the fork context properly
    const forkAgentCtx = new AgentContextModule.AgentContext(parentCtx.sessionID, parentCtx.workDir);
    // Copy messages from cloned conv
    for (const m of clonedConv.messages()) {
      forkAgentCtx.conversation.addMessage(m);
    }

    const forkID = AgentId.create();
    const loop = new AgentLoop(forkID, this._parent.provider, forkExecutor, forkAgentCtx, defaultLoopConfig(), filteredDefs);

    const result = await loop.run({}, null);
    return result;
  }
}
