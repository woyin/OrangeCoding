// Types
export type {
  OrangeConfig,
  ProviderConfig,
  HooksConfig,
  PermissionsConfig,
  HarnessConfig,
  MultiplexerConfig,
  SkillsConfig,
  SkillDefinition,
} from "./types.js";
export { validateConfig } from "./types.js";

// JSONC parser
export { parseJSONC } from "./jsonc.js";

// Crypto
export { encrypt, decrypt } from "./crypto.js";

// Config management
export { ConfigManager, defaultConfig } from "./config.js";
