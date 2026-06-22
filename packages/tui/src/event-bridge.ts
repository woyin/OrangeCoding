/**
 * TuiEventBridge converts AgentEvents from the agent loop into TUI messages.
 * This is the glue between the agent system and the TUI display.
 */

import type { AgentEvent } from "@orangecoding/core";
import {
  StreamChunkEvent,
  CompletedEvent,
  ToolCallRequestedEvent,
  ToolCallCompletedEvent,
  ErrorEvent,
  GuardrailDecisionEvent,
  MessageReceivedEvent,
  newAssistantMessage,
} from "@orangecoding/core";
import type { App } from "./app.js";

/**
 * TuiEventBridge is the glue between the agent event stream and the TUI: it
 * translates AgentEvents (stream chunks, tool calls, completion, errors,
 * guardrail decisions) into TUI model messages that drive rendering.
 */
export class TuiEventBridge {
  private readonly app: App;

  constructor(app: App) {
    this.app = app;
  }

  /**
   * Handle an AgentEvent and update the TUI accordingly.
   */
  handleEvent(event: AgentEvent): void {
    if (event instanceof StreamChunkEvent) {
      this.app.appendStream(event.content);
    } else if (event instanceof MessageReceivedEvent) {
      // Finalize the stream as a message
      if (this.app.currentStream) {
        this.app.send({
          type: "core_message",
          msg: newAssistantMessage(this.app.currentStream),
        });
        this.app.clearStream();
      }
    } else if (event instanceof CompletedEvent) {
      // Finalize any remaining stream
      if (this.app.currentStream) {
        this.app.send({
          type: "core_message",
          msg: newAssistantMessage(this.app.currentStream),
        });
        this.app.clearStream();
      }
      this.app.send({
        type: "status",
        status: `done | ${event.summary}`,
      });
    } else if (event instanceof ToolCallRequestedEvent) {
      this.app.send({
        type: "status",
        status: `🔧 ${event.toolCall.function_name}...`,
      });
    } else if (event instanceof ToolCallCompletedEvent) {
      const icon = event.success ? "✅" : "❌";
      this.app.send({
        type: "status",
        status: `${icon} ${event.toolName} ${event.durationMs}ms`,
      });
    } else if (event instanceof ErrorEvent) {
      this.app.send({
        type: "status",
        status: `❌ ${event.errorMessage}`,
      });
    } else if (event instanceof GuardrailDecisionEvent) {
      if (event.decision === "deny") {
        this.app.send({
          type: "status",
          status: `🛡️ blocked: ${event.reason}`,
        });
      }
    }
  }

  /**
   * Returns the event handler function bound to this bridge.
   */
  getHandler(): (event: AgentEvent) => void {
    return (event: AgentEvent) => this.handleEvent(event);
  }
}
