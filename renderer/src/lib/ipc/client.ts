import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import { type CommandMap } from "./commands";

/**
 * The only place `invoke` is called (plan R4/R18). The command name is a key
 * of `CommandMap`, so a command that the Rust side does not register cannot
 * be called from here.
 */
export async function call<K extends keyof CommandMap>(
  command: K,
  args?: CommandMap[K]["args"],
): Promise<CommandMap[K]["result"]> {
  return invoke<CommandMap[K]["result"]>(
    command,
    args as InvokeArgs | undefined,
  );
}
