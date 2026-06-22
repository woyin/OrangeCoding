/**
 * @module cli-main
 *
 * OrangeCoding CLI entry point.
 *
 * Parses command-line arguments and dispatches to the appropriate
 * subcommand handler (launch, serve, tui-mode, sessions, etc.).
 */
#!/usr/bin/env node

/**
 * OrangeCoding CLI entry point.
 *
 * Command tree:
 *   orangecoding (root, default=launch)
 *     ├── launch   [--prompt/-p, --text]
 *     ├── init
 *     ├── config
 *     │   ├── get [key]
 *     │   └── set [key] [value]
 *     ├── status
 *     ├── serve    [--addr]
 *     └── version
 */

import { parseArgs } from "node:util";
import { runLaunch } from "./commands/launch.js";
import { runInit } from "./commands/init.js";
import { runConfigGet, runConfigSet } from "./commands/config.js";
import { runStatus } from "./commands/status.js";
import { runServe } from "./commands/serve.js";
import { runVersion } from "./commands/version.js";
import { runSkills } from "./commands/skills.js";
import { runResume } from "./commands/resume.js";
import { runAnalyze } from "./commands/analyze.js";
import { runSessions } from "./commands/sessions.js";

// ---------------------------------------------------------------------------
// CLI argument parsing and command dispatch
// ---------------------------------------------------------------------------
// Resolves argv[0] to a subcommand and forwards the remaining flags. The
// default (no subcommand) is `launch`. Unknown commands print usage and
// exit non-zero.

/** printUsage writes the top-level CLI help text (commands + options) to stdout. */
function printUsage(): void {
  console.log(`Usage: orangecoding [command] [options]

Commands:
  launch              Start AI agent (default when no command given)
  init                Initialize project config
  config get <key>    Get a configuration value
  config set <key> <value>  Set a configuration value
  sessions            List saved sessions
  skills              List available skills
  resume [run-id]     Resume an interrupted session
  analyze             Analyze sessions for self-improvement
  status              Show system status
  serve               Start control server
  version             Show version

Options:
  --prompt, -p <text>  Single-shot task prompt (launch only)
  --skill, -s <name>   Use a specific skill (launch only)
  --text               Text mode, no TUI (launch only)
  --resume, -r <id>    Resume a saved session (launch only)
  --addr <addr>        Bind address for serve (serve only)
  --log-level <level>  Log level: debug, info, warn, error
  --json-log           Enable JSON log format
  --help, -h           Show this help message`);
}

/** exitWithError prints a message to stderr and exits with code 1. */
function exitWithError(msg: string): never {
  console.error(`Error: ${msg}`);
  process.exit(1);
}

/**
 * main is the CLI entry point: parses argv[0] as a subcommand, dispatches to
 * the matching handler, and forwards remaining flags. No args (or a leading
 * flag) defaults to `launch`. Unknown commands print usage and exit non-zero.
 */
async function main(): Promise<void> {
  const args = process.argv.slice(2);

  // No args = default to launch
  if (args.length === 0) {
    await runLaunch();
    return;
  }

  const command = args[0]!;

  // Handle --help anywhere at the top level
  if (command === "--help" || command === "-h") {
    printUsage();
    return;
  }

  switch (command) {
    // launch: parse --prompt/-p, --text, --skill/-s, --resume/-r and start the agent.
    /** Subcommand: launch — starts a new agent session. */
    case "launch": {
      const parsed = parseArgs({
        args: args.slice(1),
        options: {
          prompt: { type: "string", short: "p" },
          skill: { type: "string", short: "s" },
          text: { type: "boolean", default: false },
          resume: { type: "string", short: "r" },
        },
        strict: false,
      });
      await runLaunch(
        typeof parsed.values.prompt === "string" ? parsed.values.prompt : undefined,
        !!parsed.values.text,
        typeof parsed.values.skill === "string" ? parsed.values.skill : undefined,
        typeof parsed.values.resume === "string" ? parsed.values.resume : undefined,
      );
      break;
    }

    // init: scaffold a project config file in the current directory.
    case "init": {
      runInit();
      break;
    }

    // config: route to the get/set subcommands (requires a subcommand argument).
    case "config": {
      const subArgs = args.slice(1);
      if (subArgs.length === 0) {
        exitWithError(
          "config requires a subcommand: get <key> or set <key> <value>",
        );
      }

      const subCommand = subArgs[0];
      switch (subCommand) {
        case "get": {
          if (subArgs.length < 2) {
            exitWithError("config get requires a key argument");
          }
          runConfigGet(subArgs[1]!);
          break;
        }
        case "set": {
          if (subArgs.length < 3) {
            exitWithError("config set requires key and value arguments");
          }
          runConfigSet(subArgs[1]!, subArgs[2]!);
          break;
        }
        default:
          exitWithError(`unknown config subcommand: ${subCommand}`);
      }
      break;
    }

    // status: print version, providers, API keys, tools, sessions, audit, harness.
    /** Subcommand: status — shows running agent status. */
    case "status": {
      runStatus();
      break;
    }

    // serve: start the HTTP/WebSocket control server bound to --addr or config port.
    /** Subcommand: serve — starts the HTTP control server. */
    case "serve": {
      const parsed = parseArgs({
        args: args.slice(1),
        options: {
          addr: { type: "string" },
        },
        strict: false,
      });
      await runServe(
        typeof parsed.values.addr === "string" ? parsed.values.addr : undefined,
      );
      break;
    }

    // sessions: list saved JSONL sessions under ~/.orangecoding/sessions.
    /** Subcommand: sessions — manages saved sessions. */
    case "sessions": {
      await runSessions();
      break;
    }

    // skills: list discovered skills and their descriptions.
    case "skills": {
      runSkills();
      break;
    }

    // resume: re-open a saved session by run-id for continuation.
    case "resume": {
      const runID = args.slice(1).find((a) => !a.startsWith("-"));
      await runResume(runID);
      break;
    }

    // analyze: run self-improvement analysis over saved sessions.
    case "analyze": {
      await runAnalyze();
      break;
    }

    // version: print the package version.
    case "version": {
      runVersion();
      break;
    }

    default:
      // If the first arg looks like a flag (starts with -), treat it as launch with flags
      if (command.startsWith("-")) {
        const parsed = parseArgs({
          args: args,
          options: {
            prompt: { type: "string", short: "p" },
            skill: { type: "string", short: "s" },
            text: { type: "boolean", default: false },
            resume: { type: "string", short: "r" },
          },
          strict: false,
        });
        await runLaunch(
          typeof parsed.values.prompt === "string" ? parsed.values.prompt : undefined,
          !!parsed.values.text,
          typeof parsed.values.skill === "string" ? parsed.values.skill : undefined,
          typeof parsed.values.resume === "string" ? parsed.values.resume : undefined,
        );
      } else {
        exitWithError(`unknown command: ${command}\nRun 'orangecoding --help' for usage.`);
      }
  }
}

main().catch((err) => {
  const msg = err instanceof Error ? err.message : String(err);
  console.error(`Error: ${msg}`);
  process.exit(1);
});
