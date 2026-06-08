import { newConfigError } from "@orangecoding/core";

const NONCE_SIZE = 12;

/**
 * Convert a Uint8Array to a BufferSource that is compatible with the Web
 * Crypto API's strict type requirements for ArrayBuffer (not SharedArrayBuffer).
 */
function toBufferSource(data: Uint8Array): ArrayBuffer {
  // Create a plain ArrayBuffer copy to satisfy the Web Crypto API's
  // strict type requirement (ArrayBuffer, not SharedArrayBuffer).
  const buf = new ArrayBuffer(data.byteLength);
  new Uint8Array(buf).set(data);
  return buf;
}

/**
 * Encrypt encrypts plaintext using AES-256-GCM with the given 32-byte key.
 * The 12-byte nonce is prepended to the ciphertext.
 */
export async function encrypt(
  key: Uint8Array,
  plaintext: Uint8Array,
): Promise<Uint8Array> {
  if (key.length !== 32) {
    throw newConfigError(`key must be 32 bytes, got ${key.length}`);
  }

  const nonce = crypto.getRandomValues(new Uint8Array(NONCE_SIZE));
  const cryptoKey = await crypto.subtle.importKey(
    "raw",
    toBufferSource(key),
    { name: "AES-GCM" },
    false,
    ["encrypt"],
  );

  const encrypted = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv: nonce, tagLength: 128 },
    cryptoKey,
    toBufferSource(plaintext),
  );

  // Prepend nonce to ciphertext (nonce | ciphertext+tag)
  const result = new Uint8Array(NONCE_SIZE + encrypted.byteLength);
  result.set(nonce, 0);
  result.set(new Uint8Array(encrypted), NONCE_SIZE);

  return result;
}

/**
 * Decrypt decrypts ciphertext produced by encrypt using the given 32-byte key.
 * It reads the nonce from the first 12 bytes of ciphertext.
 */
export async function decrypt(
  key: Uint8Array,
  ciphertext: Uint8Array,
): Promise<Uint8Array> {
  if (key.length !== 32) {
    throw newConfigError(`key must be 32 bytes, got ${key.length}`);
  }

  if (ciphertext.length < NONCE_SIZE + 1) {
    throw newConfigError("ciphertext too short");
  }

  const nonce = ciphertext.slice(0, NONCE_SIZE);
  const data = ciphertext.slice(NONCE_SIZE);

  const cryptoKey = await crypto.subtle.importKey(
    "raw",
    toBufferSource(key),
    { name: "AES-GCM" },
    false,
    ["decrypt"],
  );

  try {
    const plaintext = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv: nonce, tagLength: 128 },
      cryptoKey,
      toBufferSource(data),
    );
    return new Uint8Array(plaintext);
  } catch (err) {
    throw newConfigError(
      `decrypt: ${err instanceof Error ? err.message : String(err)}`,
    );
  }
}
