/**
 * @module session-blob
 *
 * Session blob storage — persistent binary data associated with sessions.
 *
 * Manages storage and retrieval of binary data (screenshots, file snapshots,
 * etc.) that are associated with agent sessions but stored separately from
 * the conversation history.
 */
import { createHash } from "node:crypto";
import { mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { OrangeError, newIOError } from "@orangecoding/core";

/** Matches a valid SHA-256 hex hash (64 lowercase hex chars). */
const VALID_HEX_HASH = /^[0-9a-f]{64}$/;

/**
 * BlobStore implements content-addressed storage using SHA-256 hashes as keys.
 * Each blob is stored as a file named by its hex-encoded SHA-256 hash.
 */
export class BlobStore {
  constructor(private readonly dir: string) {}

  /**
   * Put writes data to the store and returns its SHA-256 hex hash.
   * If a blob with the same content already exists, it is not written again.
   */
  async put(data: Uint8Array): Promise<string> {
    const hexHash = createHash("sha256").update(data).digest("hex");
    const path = join(this.dir, hexHash);

    // Check if already exists (idempotent)
    try {
      await stat(path);
      return hexHash;
    } catch {
      // File does not exist; proceed to write.
    }

    try {
      await mkdir(this.dir, { recursive: true });
    } catch (err) {
      throw newIOError(`blob store mkdir: ${(err as Error).message}`);
    }

    try {
      await writeFile(path, data);
    } catch (err) {
      throw newIOError(`blob store write: ${(err as Error).message}`);
    }

    return hexHash;
  }

  /**
   * Get reads a blob by its SHA-256 hex hash.
   * Throws OrangeError if the hash is invalid or the blob does not exist.
   */
  async get(hash: string): Promise<Uint8Array> {
    if (!VALID_HEX_HASH.test(hash)) {
      throw newIOError(`blob store: invalid hash format "${hash}"`);
    }
    const path = join(this.dir, hash);
    try {
      return await readFile(path);
    } catch (err) {
      throw newIOError(`blob store get ${hash}: ${(err as Error).message}`);
    }
  }

  /**
   * Has returns true if a blob with the given hash exists in the store.
   */
  async has(hash: string): Promise<boolean> {
    if (!VALID_HEX_HASH.test(hash)) {
      return false;
    }
    const path = join(this.dir, hash);
    try {
      await stat(path);
      return true;
    } catch {
      return false;
    }
  }
}
