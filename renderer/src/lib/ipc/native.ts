import { open } from "@tauri-apps/plugin-dialog";
import { readText } from "@tauri-apps/plugin-clipboard-manager";

/** Open the native folder picker. Returns null when the user cancels. */
export async function chooseDirectory(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false });
  return typeof selected === "string" ? selected : null;
}

/** Open the native file picker. Returns null when the user cancels. */
export async function chooseFile(
  filters?: Array<{ name: string; extensions: string[] }>,
): Promise<string | null> {
  const selected = await open({ directory: false, multiple: false, filters });
  return typeof selected === "string" ? selected : null;
}

/** Read the clipboard through the clipboard-manager plugin. */
export function readClipboard(): Promise<string> {
  return readText();
}
