/**
 * Compactor reduces conversation size by removing old messages when the token
 * estimate exceeds a configured limit.
 * Ported from modules/agent/compaction.go.
 *
 * Performance: Uses binary search O(n log n) instead of linear scan O(n^2)
 * to find the optimal cutoff point.
 */

import { Role, Conversation, Message, newSystemMessage } from "@orangecoding/core";

function isCJK(code: number): boolean {
  return (
    (code >= 0x4e00 && code <= 0x9fff) ||
    (code >= 0x3400 && code <= 0x4dbf) ||
    (code >= 0x3000 && code <= 0x303f) ||
    (code >= 0xff00 && code <= 0xffef) ||
    (code >= 0x3040 && code <= 0x309f) ||
    (code >= 0x30a0 && code <= 0x30ff)
  );
}

export class Compactor {
  private _maxTokens: number;

  constructor(maxTokens: number) {
    this._maxTokens = maxTokens;
  }

  /** Compact removes old non-system messages from the conversation until the token
   *  estimate is under the configured limit. */
  compact(conv: Conversation): void {
    const estimate = conv.tokenEstimate();
    if (estimate <= this._maxTokens) {
      return;
    }

    const msgs = conv.messagesUnsafe();
    if (msgs.length <= 6) {
      return;
    }

    // Identify system prompt extent
    let systemEnd = 0;
    for (let i = 0; i < msgs.length; i++) {
      if (msgs[i]!.role === Role.System) {
        systemEnd = i + 1;
      } else {
        break;
      }
    }

    let keepFrom = msgs.length - 5;
    if (keepFrom < systemEnd) {
      keepFrom = systemEnd;
    }

    const systemMsgs = msgs.slice(0, systemEnd);
    const tailMsgs = msgs.slice(keepFrom);
    const middle = msgs.slice(systemEnd, keepFrom);

    // Pre-compute token contributions for system + tail (fixed overhead)
    const systemTokens = tokenEstimateFor(systemMsgs);
    const tailTokens = tokenEstimateFor(tailMsgs);
    const fixedTokens = systemTokens + tailTokens;

    // If even without middle messages we're over budget, just keep system + tail
    if (fixedTokens > this._maxTokens) {
      conv.clear();
      for (const m of systemMsgs) conv.addMessage(m);
      for (const m of tailMsgs) conv.addMessage(m);
      return;
    }

    // Pre-compute cumulative token estimates from each middle start position
    // Use binary search to find how many middle messages we can keep
    const budget = this._maxTokens - fixedTokens;

    // Compute cumulative tokens from the end of middle backwards
    const suffixTokens: number[] = new Array(middle.length + 1);
    suffixTokens[middle.length] = 0;
    for (let i = middle.length - 1; i >= 0; i--) {
      const msgTokens = tokenEstimateForMsg(middle[i]!);
      suffixTokens[i] = (suffixTokens[i + 1] ?? 0) + msgTokens;
    }

    // Binary search: find the smallest middleStart where suffixTokens[middleStart] <= budget
    let lo = 0;
    let hi = middle.length;
    while (lo < hi) {
      const mid = (lo + hi) >>> 1;
      if ((suffixTokens[mid] ?? 0) <= budget) {
        hi = mid;
      } else {
        lo = mid + 1;
      }
    }

    // Rebuild conversation: system + middle[lo..] + tail
    conv.clear();
    for (const m of systemMsgs) conv.addMessage(m);
    for (let i = lo; i < middle.length; i++) {
      conv.addMessage(middle[i]!);
    }
    for (const m of tailMsgs) conv.addMessage(m);
  }
}

function tokenEstimateForMsg(msg: Message): number {
  let cjkCount = 0;
  let nonCJKCount = 0;

  // Estimate content tokens
  for (const ch of msg.content) {
    const code = ch.codePointAt(0)!;
    if (isCJK(code)) {
      cjkCount++;
    } else {
      nonCJKCount++;
    }
  }

  // Estimate tool call tokens (function name + arguments)
  if (msg.toolCalls) {
    for (const tc of msg.toolCalls) {
      for (const ch of tc.function_name) {
        const code = ch.codePointAt(0)!;
        if (isCJK(code)) {
          cjkCount++;
        } else {
          nonCJKCount++;
        }
      }
      const argsStr = typeof tc.arguments === "string" ? tc.arguments : JSON.stringify(tc.arguments);
      for (const ch of argsStr) {
        const code = ch.codePointAt(0)!;
        if (isCJK(code)) {
          cjkCount++;
        } else {
          nonCJKCount++;
        }
      }
    }
  }

  return cjkCount * 2 + Math.floor(nonCJKCount / 4);
}

function tokenEstimateFor(msgs: readonly Message[]): number {
  let total = 0;
  for (const m of msgs) {
    total += tokenEstimateForMsg(m);
  }
  return total;
}
