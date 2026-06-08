import { Role } from "./types.js";

// ---------------------------------------------------------------------------
// ToolCall
// ---------------------------------------------------------------------------

export interface ToolCall {
  id: string;
  function_name: string;
  arguments: unknown;
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

export interface MessageJSON {
  role: string;
  content?: string;
  name?: string;
  tool_calls?: ToolCall[];
  tool_call_id?: string;
  created_at: string;
}

export class Message {
  constructor(
    public readonly role: Role,
    public readonly content: string,
    public readonly createdAt: Date = new Date(),
    public readonly name?: string,
    public readonly toolCalls?: ToolCall[],
    public readonly toolCallID?: string,
  ) {}

  hasToolCalls(): boolean {
    return this.toolCalls != null && this.toolCalls.length > 0;
  }

  toJSON(): MessageJSON {
    const obj: MessageJSON = {
      role: this.role,
      content: this.content,
      created_at: this.createdAt.toISOString(),
    };
    if (this.name) obj.name = this.name;
    if (this.toolCalls && this.toolCalls.length > 0) obj.tool_calls = this.toolCalls;
    if (this.toolCallID) obj.tool_call_id = this.toolCallID;
    return obj;
  }
}

// ---------------------------------------------------------------------------
// Message factory functions
// ---------------------------------------------------------------------------

export function newSystemMessage(content: string): Message {
  return new Message(Role.System, content);
}

export function newUserMessage(content: string): Message {
  return new Message(Role.User, content);
}

export function newAssistantMessage(content: string): Message {
  return new Message(Role.Assistant, content);
}

export function newAssistantMessageWithToolCalls(content: string, toolCalls: ToolCall[]): Message {
  return new Message(Role.Assistant, content, new Date(), undefined, toolCalls);
}

export function newToolResultMessage(toolCallID: string, content: string, isError: boolean): Message {
  const prefix = isError ? "ERROR: " : "";
  return new Message(Role.Tool, `${prefix}${content}`, new Date(), undefined, undefined, toolCallID);
}

// ---------------------------------------------------------------------------
// ToolResult
// ---------------------------------------------------------------------------

export interface ToolResultJSON {
  tool_call_id: string;
  content: string;
  is_error: boolean;
}

export class ToolResult {
  constructor(
    public readonly toolCallID: string,
    public readonly content: string,
    public readonly isError: boolean,
  ) {}

  toMessage(): Message {
    return newToolResultMessage(this.toolCallID, this.content, this.isError);
  }

  toJSON(): ToolResultJSON {
    return {
      tool_call_id: this.toolCallID,
      content: this.content,
      is_error: this.isError,
    };
  }
}

export function newToolResultSuccess(toolCallID: string, content: string): ToolResult {
  return new ToolResult(toolCallID, content, false);
}

export function newToolResultError(toolCallID: string, content: string): ToolResult {
  return new ToolResult(toolCallID, content, true);
}

// ---------------------------------------------------------------------------
// Conversation
// ---------------------------------------------------------------------------

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

export class Conversation {
  private _messages: Message[] = [];

  static create(): Conversation {
    return new Conversation();
  }

  static createWithSystemPrompt(prompt: string): Conversation {
    const conv = new Conversation();
    conv.addMessage(newSystemMessage(prompt));
    return conv;
  }

  addMessage(msg: Message): void {
    this._messages.push(msg);
  }

  /** Returns a defensive copy of messages. */
  messages(): Message[] {
    return [...this._messages];
  }

  /** Returns the internal messages array without copying.
   *  Callers MUST NOT mutate the returned array. */
  messagesUnsafe(): readonly Message[] {
    return this._messages;
  }

  get length(): number {
    return this._messages.length;
  }

  isEmpty(): boolean {
    return this._messages.length === 0;
  }

  systemPrompt(): string | undefined {
    if (this._messages.length === 0) return undefined;
    const first = this._messages[0]!;
    if (first.role !== Role.System) return undefined;
    return first.content;
  }

  lastMessage(): Message | undefined {
    return this._messages.length > 0 ? this._messages[this._messages.length - 1] : undefined;
  }

  lastAssistantMessage(): Message | undefined {
    for (let i = this._messages.length - 1; i >= 0; i--) {
      const msg = this._messages[i]!;
      if (msg.role === Role.Assistant) return msg;
    }
    return undefined;
  }

  pendingToolCalls(): ToolCall[] | undefined {
    const last = this.lastAssistantMessage();
    if (last == null) return undefined;
    return last.toolCalls;
  }

  clear(): void {
    this._messages = [];
  }

  /** Replace the system prompt in-place without rebuilding the message array. */
  replaceSystemPrompt(newPrompt: string): void {
    if (this._messages.length > 0 && this._messages[0]!.role === Role.System) {
      this._messages[0] = newSystemMessage(newPrompt);
    } else {
      this._messages.unshift(newSystemMessage(newPrompt));
    }
  }

  tokenEstimate(): number {
    let cjkCount = 0;
    let nonCJKCount = 0;
    for (const msg of this._messages) {
      for (const ch of msg.content) {
        const code = ch.codePointAt(0)!;
        if (isCJK(code)) {
          cjkCount++;
        } else {
          nonCJKCount++;
        }
      }
    }
    return cjkCount * 2 + Math.floor(nonCJKCount / 4);
  }
}
