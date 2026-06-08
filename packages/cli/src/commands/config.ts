import { ConfigManager } from "@orangecoding/config";
import { defaultConfigPath } from "./init.js";

/**
 * Handles the `config get <key>` command.
 * Loads the config file and prints the value for the given key.
 */
export function runConfigGet(key: string): void {
  const configPath = defaultConfigPath();
  const val = runConfigGetAtPath(configPath, key);
  console.log(val);
}

/**
 * Loads the config at path and returns the value for key.
 */
export function runConfigGetAtPath(filePath: string, key: string): unknown {
  const mgr = new ConfigManager();
  return mgr.get(filePath, key);
}

/**
 * Handles the `config set <key> <value>` command.
 * Loads the config, sets the key to the coerced value, and saves.
 */
export function runConfigSet(key: string, value: string): void {
  const configPath = defaultConfigPath();
  runConfigSetAtPath(configPath, key, value);
  console.log(`Set ${key}`);
}

/**
 * Loads the config at path, sets key to value (with type coercion), and saves.
 */
export function runConfigSetAtPath(
  filePath: string,
  key: string,
  value: string,
): void {
  const mgr = new ConfigManager();

  // Determine the target type from the existing config field
  // and coerce the string value appropriately.
  let coerced: unknown = value;

  const current = mgr.get(filePath, key);

  if (typeof current === "number" && Number.isInteger(current)) {
    const n = parseInt(value, 10);
    if (isNaN(n)) {
      throw new Error(`cannot convert "${value}" to int`);
    }
    coerced = n;
  } else if (typeof current === "number") {
    const f = parseFloat(value);
    if (isNaN(f)) {
      throw new Error(`cannot convert "${value}" to float`);
    }
    coerced = f;
  } else if (typeof current === "boolean") {
    if (value === "true") {
      coerced = true;
    } else if (value === "false") {
      coerced = false;
    } else {
      throw new Error(
        `cannot convert "${value}" to bool (use "true" or "false")`,
      );
    }
  }

  mgr.set(filePath, key, coerced);
}
