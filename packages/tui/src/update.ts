import { newUserMessage, newSystemMessage, type Message } from "@orangecoding/core";
import { Model } from "./model.js";

// --- Message types (equivalent to tea.Msg in Bubble Tea) ---

/** Carries a core.Message into the model. */
export interface CoreMessageMsg {
  type: "core_message";
  msg: Message;
}

/** Updates the status bar text. */
export interface StatusMsg {
  type: "status";
  status: string;
}

/** Window resize event. */
export interface WindowSizeMsg {
  type: "window_size";
  width: number;
  height: number;
}

/** Key press event. */
export interface KeyMsg {
  type: "key";
  key: string;        // "enter", "escape", "tab", "backspace", "ctrl+c"
  runes?: string;     // typed characters for printable keys
}

export type TuiMsg = CoreMessageMsg | StatusMsg | WindowSizeMsg | KeyMsg;

// --- Known slash commands ---

const KNOWN_SLASH_COMMANDS = new Set([
  "/help",
  "/quit",
  "/clear",
  "/model",
  "/mode",
  "/think",
  "/plan",
]);

/**
 * update is the central update function.
 * Takes the current model and a message, returns the updated model.
 */
export function update(m: Model, msg: TuiMsg): Model {
  switch (msg.type) {
    case "window_size":
      return new Model({ ...m, width: msg.width, height: msg.height });

    case "core_message":
      return new Model({ ...m, messages: [...m.messages, msg.msg] });

    case "status":
      return new Model({ ...m, status: msg.status });

    case "key":
      return handleKey(m, msg);
  }
}

function handleKey(m: Model, msg: KeyMsg): Model {
  switch (msg.key) {
    case "ctrl+c":
    case "escape":
      return new Model({ ...m, quitting: true });

    case "tab":
      return new Model({ ...m, sidebar: !m.sidebar });

    case "enter":
      return handleInput(m);

    case "backspace": {
      const input = m.input.length > 0 ? m.input.slice(0, -1) : "";
      return new Model({ ...m, input });
    }

    default:
      if (msg.runes) {
        return new Model({ ...m, input: m.input + msg.runes });
      }
      return m;
  }
}

function handleInput(m: Model): Model {
  const text = m.input.trim();
  const base = new Model({ ...m, input: "" });

  if (text === "") return base;

  // Slash commands
  if (text.startsWith("/")) {
    return handleSlashCommand(base, text);
  }

  // Regular user message
  return new Model({ ...base, messages: [...base.messages, newUserMessage(text)] });
}

function handleSlashCommand(m: Model, text: string): Model {
  const parts = text.split(/\s+/);
  const cmd = parts[0]!;

  switch (cmd) {
    case "/quit":
      return new Model({ ...m, quitting: true });

    case "/clear":
      return new Model({ ...m, messages: [] });

    case "/help": {
      const helpText = `Available commands:
  /help   - Show this help message
  /quit   - Quit the application
  /clear  - Clear conversation history
  /mode   - Switch mode (normal, plan, goal, ultra)
  /model  - Switch model
  /think  - Toggle thinking mode
  /plan   - Enter plan mode`;
      return new Model({ ...m, messages: [...m.messages, newSystemMessage(helpText)] });
    }

    case "/mode": {
      const newMode = parts[1];
      if (newMode) {
        switch (newMode) {
          case "normal":
          case "plan":
          case "goal":
          case "ultra":
            return new Model({ ...m, mode: newMode, status: `mode=${newMode}` });
          default:
            return new Model({ ...m, messages: [...m.messages, newSystemMessage("unknown mode: " + newMode)] });
        }
      }
      return new Model({ ...m, messages: [...m.messages, newSystemMessage("usage: /mode <normal|plan|goal|ultra>")] });
    }

    case "/model": {
      if (parts[1]) {
        return new Model({ ...m, status: `model=${parts[1]}` });
      }
      return new Model({ ...m, messages: [...m.messages, newSystemMessage("usage: /model <name>")] });
    }

    case "/think":
      return new Model({ ...m, status: "thinking enabled" });

    case "/plan":
      return new Model({ ...m, mode: "plan", status: "mode=plan" });

    default:
      if (KNOWN_SLASH_COMMANDS.has(cmd)) {
        return new Model({ ...m, messages: [...m.messages, newSystemMessage("command not yet implemented: " + cmd)] });
      }
      return new Model({ ...m, messages: [...m.messages, newSystemMessage("unknown command: " + cmd)] });
  }
}
