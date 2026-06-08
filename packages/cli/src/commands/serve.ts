/**
 * Handles the `serve` command.
 * Starts the OrangeCoding control server for managing agent sessions.
 *
 * NOTE: The @orangecoding/worker and @orangecoding/control-server packages
 * are currently stubs. This command is wired up structurally but will
 * throw if the actual implementations are not yet available.
 */

import { ConfigManager } from "@orangecoding/config";
import { defaultConfigPath } from "./init.js";

/**
 * Run the serve command.
 * @param addr - Bind address override (e.g. ":3200"). Falls back to config control_port.
 */
export async function runServe(addr?: string): Promise<void> {
  const configPath = defaultConfigPath();

  // Load configuration
  const mgr = new ConfigManager();
  const cfg = mgr.load(configPath);

  // Determine bind address
  const bindAddr = addr || `:${cfg.control_port}`;

  // The control-server and worker packages are stubs at this point.
  // When they are implemented, the following wiring will be used:
  //
  //   import { ServerEvent } from "@orangecoding/control-protocol";
  //   import { WorkerRuntime } from "@orangecoding/worker";
  //   import { Server } from "@orangecoding/control-server";
  //
  //   const eventCh: AsyncIterable<ServerEvent> = ...;
  //   const runtime = new WorkerRuntime(eventCh);
  //   const server = new Server(runtime, bindAddr);
  //   await server.start();
  //
  //   console.log(`OrangeCoding control server listening on ${bindAddr}`);
  //   console.log("Press Ctrl+C to stop.");
  //
  //   // Block until interrupted
  //   await waitForSignal();
  //   console.log("\nShutting down...");
  //   await server.stop();

  console.log(
    `Serve command is not yet fully implemented. Would listen on ${bindAddr}.`,
  );
  console.log(
    "Requires @orangecoding/worker and @orangecoding/control-server implementations.",
  );
}
