/**
 * FileWatcher monitors files/directories for changes and emits events.
 * Uses Node.js fs.watch() for efficiency — no external dependencies.
 */

import * as fs from "node:fs";
import * as path from "node:path";

/** FileChangeEvent describes a single filesystem change observed by FileWatcher. */
export interface FileChangeEvent {
  type: "add" | "change" | "remove";
  path: string;
  timestamp: Date;
}

export type FileChangeHandler = (event: FileChangeEvent) => void;

/** FileWatcherOptions configures which paths to watch, what to ignore, and debounce behavior. */
export interface FileWatcherOptions {
  /** Directories or files to watch */
  paths: string[];
  /** Glob patterns to ignore (simple pattern matching) */
  ignorePatterns?: string[];
  /** Debounce interval in ms (default: 300) */
  debounceMs?: number;
  /** Recursive watching (default: true) */
  recursive?: boolean;
}

export class FileWatcher {
  private watchers: fs.FSWatcher[] = [];
  private handler: FileChangeHandler | undefined;
  private debounceTimers = new Map<string, ReturnType<typeof setTimeout>>();
  private readonly opts: Required<FileWatcherOptions>;
  private running = false;

  /**
   * Constructs a watcher merging the given options with sensible defaults
   * (common ignore patterns, 300ms debounce, recursive watching).
   */
  constructor(opts: FileWatcherOptions) {
    this.opts = {
      paths: opts.paths,
      ignorePatterns: opts.ignorePatterns ?? [
        "node_modules",
        ".git",
        "dist",
        ".next",
        "__pycache__",
        "*.pyc",
      ],
      debounceMs: opts.debounceMs ?? 300,
      recursive: opts.recursive ?? true,
    };
  }

  /**
   * Start watching and register a change handler.
   */
  start(handler: FileChangeHandler): void {
    if (this.running) return;
    this.handler = handler;
    this.running = true;

    for (const watchPath of this.opts.paths) {
      try {
        const resolved = path.resolve(watchPath);
        const stat = fs.statSync(resolved);

        if (stat.isDirectory()) {
          const watcher = fs.watch(
            resolved,
            { recursive: this.opts.recursive },
            (eventType, filename) => {
              if (filename) {
                this.handleFsEvent(eventType, path.join(resolved, filename));
              }
            },
          );
          this.watchers.push(watcher);
        } else if (stat.isFile()) {
          const watcher = fs.watch(resolved, (eventType) => {
            this.handleFsEvent(eventType, resolved);
          });
          this.watchers.push(watcher);
        }
      } catch {
        // Skip paths that don't exist or can't be watched
      }
    }
  }

  /**
   * Stop all watchers and clean up.
   */
  stop(): void {
    this.running = false;
    for (const w of this.watchers) {
      try {
        w.close();
      } catch {
        // ignore
      }
    }
    this.watchers = [];
    for (const timer of this.debounceTimers.values()) {
      clearTimeout(timer);
    }
    this.debounceTimers.clear();
  }

  /**
   * Check if the watcher is currently active.
   */
  get isActive(): boolean {
    return this.running;
  }

  /**
   * handleFsEvent is the raw fs.watch callback: it ignores matched patterns,
   * classifies add/change/remove via an accessSync probe, and emits debounced events.
   */
  private handleFsEvent(eventType: string, filePath: string): void {
    // Check ignore patterns
    if (this.shouldIgnore(filePath)) return;

    const type: FileChangeEvent["type"] =
      eventType === "rename" ? "add" : "change";

    // Check if file was actually removed
    try {
      fs.accessSync(filePath);
    } catch {
      // File doesn't exist — it was removed
      this.emitDebounced({
        type: "remove",
        path: filePath,
        timestamp: new Date(),
      });
      return;
    }

    this.emitDebounced({
      type,
      path: filePath,
      timestamp: new Date(),
    });
  }

  private emitDebounced(event: FileChangeEvent): void {
    const key = event.path;
    const existing = this.debounceTimers.get(key);
    if (existing) {
      clearTimeout(existing);
    }

    this.debounceTimers.set(
      key,
      setTimeout(() => {
        this.debounceTimers.delete(key);
        if (this.handler && this.running) {
          this.handler(event);
        }
      }, this.opts.debounceMs),
    );
  }

  /**
   * shouldIgnore tests a path against the configured ignore patterns,
   * supporting both extension globs (*.ext) and substring matches.
   */
  private shouldIgnore(filePath: string): boolean {
    const relative = filePath;
    for (const pattern of this.opts.ignorePatterns) {
      // Simple pattern matching for common cases
      if (pattern.startsWith("*.")) {
        const ext = pattern.slice(1);
        if (relative.endsWith(ext)) return true;
      } else {
        if (relative.includes(pattern)) return true;
      }
    }
    return false;
  }
}
