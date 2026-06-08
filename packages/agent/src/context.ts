/**
 * AgentContext holds the conversation state, environment, and metadata for an agent session.
 * Ported from modules/agent/context.go.
 */

import {
  Role,
  SessionId,
  Conversation,
  Message,
  newSystemMessage,
  newUserMessage,
  newAssistantMessage,
  newToolResultMessage,
} from "@orangecoding/core";
import type { ToolResult, ToolCall } from "@orangecoding/core";
import type { HarnessProfile } from "./harness-profile.js";

export class AgentContext {
  private _sessionID: SessionId;
  private _conversation: Conversation;
  private _workDir: string;
  private _env: Map<string, string>;
  private _metadata: Map<string, string>;

  constructor(sessionID: SessionId, workDir: string) {
    this._sessionID = sessionID;
    this._conversation = Conversation.create();
    this._workDir = workDir;
    this._env = new Map();
    this._metadata = new Map();
  }

  /** SetSystemPrompt sets the system prompt as the first message in the conversation. */
  setSystemPrompt(prompt: string): void {
    this._conversation.replaceSystemPrompt(prompt);
  }

  /** ApplyHarnessProfile appends harness guidance to the system prompt once. */
  applyHarnessProfile(profile: HarnessProfile): void {
    const addendum = profile.systemPromptAddendum();
    const current = this._conversation.systemPrompt();
    if (current === undefined) {
      this.setSystemPrompt(addendum.trim());
      return;
    }
    if (current.includes("[OrangeCoding Harness]")) {
      return;
    }
    this.setSystemPrompt(current + addendum);
  }

  /** AddUserMessage appends a user message to the conversation. */
  addUserMessage(content: string): void {
    this._conversation.addMessage(newUserMessage(content));
  }

  /** AddAssistantMessage appends an assistant message to the conversation. */
  addAssistantMessage(content: string): void {
    this._conversation.addMessage(newAssistantMessage(content));
  }

  /** AddToolResult appends a tool result message to the conversation. */
  addToolResult(result: ToolResult): void {
    this._conversation.addMessage(result.toMessage());
  }

  /** Conversation returns the underlying conversation. */
  get conversation(): Conversation {
    return this._conversation;
  }

  /** SessionID returns the session ID. */
  get sessionID(): SessionId {
    return this._sessionID;
  }

  /** WorkDir returns the working directory. */
  get workDir(): string {
    return this._workDir;
  }
}
