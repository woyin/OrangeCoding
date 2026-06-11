import { describe, it, expect, afterEach } from "@jest/globals";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import { FileWatcher } from "../file-watcher.js";
import type { FileChangeEvent } from "../file-watcher.js";

describe("FileWatcher", () => {
  const tmpDirs: string[] = [];

  afterEach(() => {
    for (const dir of tmpDirs) {
      try {
        fs.rmSync(dir, { recursive: true, force: true });
      } catch {
        // ignore cleanup errors
      }
    }
    tmpDirs.length = 0;
  });

  function createTmpDir(): string {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "fw-test-"));
    tmpDirs.push(dir);
    return dir;
  }

  it("starts and stops without errors", () => {
    const dir = createTmpDir();
    const watcher = new FileWatcher({ paths: [dir] });
    watcher.start(() => {});
    expect(watcher.isActive).toBe(true);
    watcher.stop();
    expect(watcher.isActive).toBe(false);
  });

  it("does not start twice", () => {
    const dir = createTmpDir();
    const watcher = new FileWatcher({ paths: [dir] });
    watcher.start(() => {});
    watcher.start(() => {}); // should be a no-op
    expect(watcher.isActive).toBe(true);
    watcher.stop();
  });

  it("handles non-existent paths gracefully", () => {
    const watcher = new FileWatcher({ paths: ["/nonexistent/path/abc123"] });
    watcher.start(() => {});
    expect(watcher.isActive).toBe(true);
    watcher.stop();
  });

  it("detects file changes", async () => {
    const dir = createTmpDir();
    const testFile = path.join(dir, "test.txt");
    fs.writeFileSync(testFile, "initial");

    const events: FileChangeEvent[] = [];
    const watcher = new FileWatcher({ paths: [dir], debounceMs: 50 });

    watcher.start((event) => {
      events.push(event);
    });

    // Wait a bit for watcher to be ready, then modify the file
    await sleep(100);
    fs.writeFileSync(testFile, "modified");

    // Wait for debounce + event
    await sleep(500);

    watcher.stop();

    expect(events.length).toBeGreaterThanOrEqual(1);
    expect(events.some((e) => e.path.includes("test.txt"))).toBe(true);
  });

  it("ignores files matching ignore patterns", async () => {
    const dir = createTmpDir();
    const nodeModules = path.join(dir, "node_modules");
    fs.mkdirSync(nodeModules);
    const ignored = path.join(nodeModules, "pkg.js");
    fs.writeFileSync(ignored, "ignored");

    const events: FileChangeEvent[] = [];
    const watcher = new FileWatcher({
      paths: [dir],
      debounceMs: 50,
      ignorePatterns: ["node_modules"],
    });

    watcher.start((event) => {
      events.push(event);
    });

    await sleep(100);
    fs.writeFileSync(ignored, "still ignored");
    await sleep(500);

    watcher.stop();

    // Events should not include the node_modules file
    const nmEvents = events.filter((e) => e.path.includes("node_modules"));
    expect(nmEvents.length).toBe(0);
  });

  it("watches individual files", () => {
    const dir = createTmpDir();
    const testFile = path.join(dir, "watched.txt");
    fs.writeFileSync(testFile, "content");

    const watcher = new FileWatcher({ paths: [testFile] });
    watcher.start(() => {});
    expect(watcher.isActive).toBe(true);
    watcher.stop();
  });

  it("stop cleans up all resources", () => {
    const dir = createTmpDir();
    const watcher = new FileWatcher({ paths: [dir] });
    watcher.start(() => {});
    watcher.stop();
    // Calling stop again should not throw
    watcher.stop();
    expect(watcher.isActive).toBe(false);
  });
});

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
