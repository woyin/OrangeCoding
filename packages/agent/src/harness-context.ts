/**
 * HarnessContextBuilder builds stable, ordered context blocks.
 * Ported from modules/agent/harness_context.go.
 *
 * Enhanced with OmO-style AGENTS.md auto-injection:
 * Automatically loads AGENTS.md, .omo/rules/**, and README.md
 * from the project directory into the agent context.
 */

import type { Conversation } from "@orangecoding/core";
import { Role } from "@orangecoding/core";
import type { HarnessMemoryManager } from "./harness-memory.js";
import type { TieredMemoryManager } from "./tiered-memory.js";
import type { ContextBlock, ContextBlockKind } from "./harness-state.js";

// ---------------------------------------------------------------------------
// HarnessContextConfig
// ---------------------------------------------------------------------------

/**
 * Tunable knobs for {@link HarnessContextBuilder}. Token caps and message
 * windows keep the assembled context within a model's input budget; the
 * file-loading flags opt AGENTS.md / README.md injection in/out.
 */

export interface HarnessContextConfig {
  maxTokens: number;
  recentMessages: number;
  memoryMaxBlocks: number;
  /** Enable AGENTS.md auto-injection (default: true) */
  loadAgentsMd: boolean;
  /** Enable README.md injection (default: false) */
  loadReadme: boolean;
  /** Working directory for finding AGENTS.md files */
  workDir: string;
}

// ---------------------------------------------------------------------------
// HarnessContextInput
// ---------------------------------------------------------------------------

/**
 * Per-turn inputs to the context builder: the static system prompt and task
 * plus the dynamic conversation and memory managers. All optional fields
 * degrade gracefully — the builder emits at least system + task blocks.
 */

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

/**
 * Assembles the ordered list of {@link ContextBlock}s fed to the model each
 * turn. Stable blocks (system, task, AGENTS.md, memory) come first so they
 * occupy predictable prompt positions; recent conversation messages are
 * appended last as low-priority dynamic blocks. Final trimming via
 * {@link fitContextBlocks} drops the lowest-priority non-stable blocks until
 * the total fits the token budget.
 */

export class HarnessContextBuilder {
  private _config: HarnessContextConfig;

  constructor(config?: Partial<HarnessContextConfig>) {
    this._config = {
      maxTokens: config?.maxTokens ?? 24000,
      recentMessages: config?.recentMessages ?? 8,
      memoryMaxBlocks: config?.memoryMaxBlocks ?? 6,
      loadAgentsMd: config?.loadAgentsMd ?? true,
      loadReadme: config?.loadReadme ?? false,
      workDir: config?.workDir ?? process.cwd(),
    };
    if (this._config.maxTokens <= 0) this._config.maxTokens = 24000;
    if (this._config.recentMessages <= 0) this._config.recentMessages = 8;
    if (this._config.memoryMaxBlocks <= 0) this._config.memoryMaxBlocks = 6;
  }

  /**
   * Assemble the full context block list for one turn. Ordering is:
   * system → task → AGENTS.md rules → README → memory tiers → recent
   * conversation. The result is trimmed to {@link maxTokens} before return.
   */
  async build(_signal: AbortSignal | undefined, input: HarnessContextInput): Promise<ContextBlock[]> {
    const blocks: ContextBlock[] = [
      newContextBlock("system", input.systemPrompt, true, 100),
      newContextBlock("task", `Task: ${input.task}`, true, 90),
    ];

    // AGENTS.md auto-injection (OmO-style)
    if (this._config.loadAgentsMd) {
      const agentsMdBlocks = await loadAgentsMdFiles(this._config.workDir);
      blocks.push(...agentsMdBlocks);
    }

    // README.md injection
    if (this._config.loadReadme) {
      const readmeBlock = await loadReadmeFile(this._config.workDir);
      if (readmeBlock) blocks.push(readmeBlock);
    }

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

// ---------------------------------------------------------------------------
// AGENTS.md auto-loading (OmO-style project memory)
// ---------------------------------------------------------------------------
// These helpers implement OmO-style project context injection: they walk
// the working directory for AGENTS.md files and rule directories
// (.omo/rules, .opencode/rules), turning each into a stable harness block.
// All file I/O is best-effort: any error short-circuits to an empty block
// list so a missing/permission-denied file never breaks the agent.

/**
 * Load AGENTS.md files from the project directory tree.
 * Walks from workDir up to the nearest git root, loading:
 *   - AGENTS.md at project root
 *   - AGENTS.md in src/ and subdirectories
 *   - .omo/rules/*.md files (project rules)
 */
async function loadAgentsMdFiles(workDir: string): Promise<ContextBlock[]> {
  const blocks: ContextBlock[] = [];

  try {
    const { existsSync } = await import("node:fs");
    const { readFile } = await import("node:fs/promises");
    const { join, resolve } = await import("node:path");

    const rootDir = resolve(workDir);

    // Load root AGENTS.md
    const rootAgentsMd = join(rootDir, "AGENTS.md");
    if (existsSync(rootAgentsMd)) {
      const content = await readFile(rootAgentsMd, "utf-8");
      if (content.trim()) {
        blocks.push(newContextBlock("harness", `[Project AGENTS.md]\n${content}`, true, 85));
      }
    }

    // Load src/AGENTS.md if it exists
    const srcAgentsMd = join(rootDir, "src", "AGENTS.md");
    if (existsSync(srcAgentsMd)) {
      const content = await readFile(srcAgentsMd, "utf-8");
      if (content.trim()) {
        blocks.push(newContextBlock("harness", `[src/AGENTS.md]\n${content}`, true, 80));
      }
    }

    // Load .omo/rules/*.md (project rules)
    const rulesDir = join(rootDir, ".omo", "rules");
    if (existsSync(rulesDir)) {
      try {
        const { readdir } = await import("node:fs/promises");
        const files = await readdir(rulesDir);
        const mdFiles = files.filter((f) => f.endsWith(".md")).sort();
        for (const file of mdFiles) {
          const content = await readFile(join(rulesDir, file), "utf-8");
          if (content.trim()) {
            blocks.push(newContextBlock("harness", `[Rule: ${file}]\n${content}`, true, 78));
          }
        }
      } catch {
        // .omo/rules/ directory read failed — ignore
      }
    }

    // Load .opencode/rules/*.md (OpenCode-compatible rules)
    const opencodeRulesDir = join(rootDir, ".opencode", "rules");
    if (existsSync(opencodeRulesDir)) {
      try {
        const { readdir } = await import("node:fs/promises");
        const files = await readdir(opencodeRulesDir);
        const mdFiles = files.filter((f) => f.endsWith(".md")).sort();
        for (const file of mdFiles) {
          const content = await readFile(join(opencodeRulesDir, file), "utf-8");
          if (content.trim()) {
            blocks.push(newContextBlock("harness", `[Rule: ${file}]\n${content}`, true, 78));
          }
        }
      } catch {
        // ignore
      }
    }
  } catch {
    // File system errors are non-fatal — context injection is best-effort
  }

  return blocks;
}

/**
 * Load README.md from the project root.
 */
async function loadReadmeFile(workDir: string): Promise<ContextBlock | null> {
  try {
    const { existsSync } = await import("node:fs");
    const { readFile } = await import("node:fs/promises");
    const { join, resolve } = await import("node:path");

    const readmePath = join(resolve(workDir), "README.md");
    if (existsSync(readmePath)) {
      const content = await readFile(readmePath, "utf-8");
      if (content.trim()) {
        // Truncate long READMEs to 3000 chars
        const truncated = content.length > 3000 ? content.slice(0, 3000) + "\n... (truncated)" : content;
        return newContextBlock("harness", `[README.md]\n${truncated}`, true, 75);
      }
    }
  } catch {
    // ignore
  }
  return null;
}

/** Construct a ContextBlock with a precomputed token estimate. */
function newContextBlock(kind: ContextBlockKind, content: string, stable: boolean, priority: number): ContextBlock {
  return {
    kind,
    content,
    stable,
    priority,
    tokenEstimate: estimateTextTokens(content),
  };
}

/**
 * Trim `blocks` to fit within `maxTokens` by repeatedly dropping the
 * lowest-priority non-stable block. Stable blocks (system, task, AGENTS.md)
 * are never dropped, which guarantees the prompt always carries identity and
 * rules. Runs in O(n²) worst case but n is small (tens of blocks).
 */
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

/** Sum the precomputed token estimates across all blocks (single pass). */
function totalBlockTokens(blocks: ContextBlock[]): number {
  let total = 0;
  for (const block of blocks) {
    total += block.tokenEstimate;
  }
  return total;
}

/** Cheap ~4 chars/token heuristic used to size context blocks for trimming. */
function estimateTextTokens(text: string): number {
  if (!text) return 0;
  const tokens = Math.floor(text.length / 4);
  return tokens === 0 ? 1 : tokens;
}

// ---------------------------------------------------------------------------
// Helper functions exported for testing
// ---------------------------------------------------------------------------

/** Test helper: does the block list contain at least one block of `kind`? */
export function containsBlockKind(blocks: ContextBlock[], kind: ContextBlockKind): boolean {
  return blocks.some((b) => b.kind === kind);
}

/** Test helper: does any block's content include `text` (substring match)? */
export function containsBlockText(blocks: ContextBlock[], text: string): boolean {
  return blocks.some((b) => b.content.includes(text));
}
