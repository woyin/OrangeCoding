/**
 * HarnessContextBuilder builds stable, ordered context blocks.
 * Ported from modules/agent/harness_context.go.
 */

import type { Conversation } from "@orangecoding/core";
import { Role } from "@orangecoding/core";
import type { HarnessMemoryManager } from "./harness-memory.js";
import type { TieredMemoryManager } from "./tiered-memory.js";
import type { ContextBlock, ContextBlockKind } from "./harness-state.js";

// ---------------------------------------------------------------------------
// HarnessContextConfig
// ---------------------------------------------------------------------------

export interface HarnessContextConfig {
  maxTokens: number;
  recentMessages: number;
  memoryMaxBlocks: number;
}

// ---------------------------------------------------------------------------
// HarnessContextInput
// ---------------------------------------------------------------------------

export interface HarnessContextInput {
  systemPrompt: string;
  task: string;
  conversation: Conversation | undefined;
  memoryManager: HarnessMemoryManager | undefined;
  tieredMemory?: TieredMemoryManager;
}

// ---------------------------------------------------------------------------
// HarnessContextBuilder
// ---------------------------------------------------------------------------

export class HarnessContextBuilder {
  private _config: HarnessContextConfig;

  constructor(config?: Partial<HarnessContextConfig>) {
    this._config = {
      maxTokens: config?.maxTokens ?? 24000,
      recentMessages: config?.recentMessages ?? 8,
      memoryMaxBlocks: config?.memoryMaxBlocks ?? 6,
    };
    if (this._config.maxTokens <= 0) this._config.maxTokens = 24000;
    if (this._config.recentMessages <= 0) this._config.recentMessages = 8;
    if (this._config.memoryMaxBlocks <= 0) this._config.memoryMaxBlocks = 6;
  }

  /** Build assembles stable system/task/memory blocks followed by recent dynamic blocks. */
  async build(_signal: AbortSignal | undefined, input: HarnessContextInput): Promise<ContextBlock[]> {
    const blocks: ContextBlock[] = [
      newContextBlock("system", input.systemPrompt, true, 100),
      newContextBlock("task", `Task: ${input.task}`, true, 90),
    ];

    if (input.tieredMemory) {
      const memoryBlocks = await input.tieredMemory.recall(input.task);
      blocks.push(...memoryBlocks);
    } else if (input.memoryManager) {
      let memoryBlocks = await input.memoryManager.recall(undefined, input.task);
      if (memoryBlocks.length > this._config.memoryMaxBlocks) {
        memoryBlocks = memoryBlocks.slice(0, this._config.memoryMaxBlocks);
      }
      blocks.push(...memoryBlocks);
    }

    if (input.conversation) {
      const msgs = input.conversation.messagesUnsafe();
      const start = Math.max(0, msgs.length - this._config.recentMessages);
      for (let i = start; i < msgs.length; i++) {
        const msg = msgs[i]!;
        blocks.push(
          newContextBlock(
            "conversation",
            `${msg.role}: ${msg.content}`,
            false,
            20,
          ),
        );
      }
    }

    return fitContextBlocks(blocks, this._config.maxTokens);
  }
}

function newContextBlock(kind: ContextBlockKind, content: string, stable: boolean, priority: number): ContextBlock {
  return {
    kind,
    content,
    stable,
    priority,
    tokenEstimate: estimateTextTokens(content),
  };
}

function fitContextBlocks(blocks: ContextBlock[], maxTokens: number): ContextBlock[] {
  if (maxTokens <= 0 || totalBlockTokens(blocks) <= maxTokens) {
    return blocks;
  }

  const kept = [...blocks];
  while (totalBlockTokens(kept) > maxTokens) {
    let dropIdx = -1;
    let lowestPriority = Number.MAX_SAFE_INTEGER;
    for (let i = 0; i < kept.length; i++) {
      if (kept[i]!.stable) continue;
      if (kept[i]!.priority < lowestPriority) {
        lowestPriority = kept[i]!.priority;
        dropIdx = i;
      }
    }
    if (dropIdx === -1) break;
    kept.splice(dropIdx, 1);
  }
  return kept;
}

function totalBlockTokens(blocks: ContextBlock[]): number {
  let total = 0;
  for (const block of blocks) {
    total += block.tokenEstimate;
  }
  return total;
}

function estimateTextTokens(text: string): number {
  if (!text) return 0;
  const tokens = Math.floor(text.length / 4);
  return tokens === 0 ? 1 : tokens;
}

// ---------------------------------------------------------------------------
// Helper functions exported for testing
// ---------------------------------------------------------------------------

export function containsBlockKind(blocks: ContextBlock[], kind: ContextBlockKind): boolean {
  return blocks.some((b) => b.kind === kind);
}

export function containsBlockText(blocks: ContextBlock[], text: string): boolean {
  return blocks.some((b) => b.content.includes(text));
}
