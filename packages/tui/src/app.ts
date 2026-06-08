import { Model } from "./model.js";
import { update, type TuiMsg } from "./update.js";
import { view } from "./view.js";

/**
 * App wraps the TUI model and provides a simple entry point.
 * In Go this used Bubble Tea; in TS the actual terminal rendering
 * can use ink, blessed, etc.
 */
export class App {
  model: Model;

  constructor() {
    this.model = new Model();
  }

  /**
   * Send a message to the app (equivalent to Bubble Tea's Update).
   */
  send(msg: TuiMsg): void {
    this.model = update(this.model, msg);
  }

  /**
   * Render the current view (equivalent to Bubble Tea's View).
   */
  render(): string {
    return view(this.model);
  }

  /**
   * Run starts the TUI application loop.
   * In a real implementation, this would set up the terminal renderer
   * and event loop. For now it returns immediately.
   */
  async run(): Promise<void> {
    // Placeholder: actual terminal rendering would be set up here
    // using ink, blessed, or another TUI framework.
    return;
  }
}
