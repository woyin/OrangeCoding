import { describe, it, expect } from "@jest/globals";
import { App } from "../app.js";
import { Model } from "../model.js";
import type { TuiMsg } from "../update.js";

describe("App", () => {
  it("creates a default model", () => {
    const app = new App();
    expect(app.model).toBeInstanceOf(Model);
    expect(app.model.quitting).toBe(false);
    expect(app.model.messages).toEqual([]);
  });

  it("send() updates the model via the update function", () => {
    const app = new App();
    const msg: TuiMsg = { type: "status", status: "testing" };
    app.send(msg);
    expect(app.model.status).toBe("testing");
  });

  it("render() returns the view string for the current model", () => {
    const app = new App();
    const output = app.render();
    // The view includes the status bar with mode=normal
    expect(output).toContain("mode=normal");
    expect(typeof output).toBe("string");
  });

  it("render() returns goodbye when quitting", () => {
    const app = new App();
    app.send({ type: "key", key: "ctrl+c" });
    expect(app.model.quitting).toBe(true);
    const output = app.render();
    expect(output).toContain("Goodbye");
  });

  it("handles typed characters via key messages", () => {
    const app = new App();
    app.send({ type: "key", key: "backspace", runes: undefined });
    app.send({ type: "key", key: "unknown", runes: "hello" });
    expect(app.model.input).toBe("hello");
  });

  it("handles enter to submit input as user message", () => {
    const app = new App();
    app.send({ type: "key", key: "unknown", runes: "test prompt" });
    app.send({ type: "key", key: "enter" });
    expect(app.model.input).toBe("");
    // Should have added a user message
    expect(app.model.messages.length).toBe(1);
    expect(app.model.messages[0]!.role).toBe("user");
    expect(app.model.messages[0]!.content).toBe("test prompt");
  });

  it("handles slash commands", () => {
    const app = new App();
    app.send({ type: "key", key: "unknown", runes: "/help" });
    app.send({ type: "key", key: "enter" });
    // /help adds a system message
    expect(app.model.messages.length).toBe(1);
    expect(app.model.messages[0]!.content).toContain("Available commands");
  });

  it("handles core_message to add assistant messages", () => {
    const app = new App();
    app.send({
      type: "core_message",
      msg: {
        role: "assistant" as any,
        content: "Hello from AI",
        timestamp: new Date(),
      } as any,
    });
    expect(app.model.messages.length).toBe(1);
    expect(app.model.messages[0]!.content).toBe("Hello from AI");
  });

  it("handles window_size changes", () => {
    const app = new App();
    app.send({ type: "window_size", width: 120, height: 40 });
    expect(app.model.width).toBe(120);
    expect(app.model.height).toBe(40);
  });

  it("tab toggles sidebar", () => {
    const app = new App();
    expect(app.model.sidebar).toBe(false);
    app.send({ type: "key", key: "tab" });
    expect(app.model.sidebar).toBe(true);
    app.send({ type: "key", key: "tab" });
    expect(app.model.sidebar).toBe(false);
  });

  it("onSubmit callback is called when user submits input", () => {
    const app = new App();
    let submittedText = "";
    app.onSubmit = (text: string) => {
      submittedText = text;
    };
    app.send({ type: "key", key: "unknown", runes: "do something" });
    app.send({ type: "key", key: "enter" });
    expect(submittedText).toBe("do something");
  });

  it("streaming text accumulates in currentStream", () => {
    const app = new App();
    app.appendStream("Hello ");
    app.appendStream("World");
    expect(app.currentStream).toBe("Hello World");
  });

  it("clearStream resets the streaming buffer", () => {
    const app = new App();
    app.appendStream("some text");
    app.clearStream();
    expect(app.currentStream).toBe("");
  });

  it("setToolStatus updates the tool status display", () => {
    const app = new App();
    app.setToolStatus("bash", true);
    expect(app.model.status).toContain("bash");
  });
});
