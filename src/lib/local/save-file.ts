// Native "Save as" flow — replaces browser blob downloads, which are inert
// inside the Tauri webview. The Rust command opens the OS save dialog and
// writes the bytes itself. Returns false when the user cancels the dialog.

import { invoke } from "@tauri-apps/api/core";
import { bytesToBase64 } from "./detector";

export async function saveBytesToFile(
  bytes: Uint8Array,
  defaultName: string
): Promise<boolean> {
  return invoke<boolean>("save_binary_file", {
    defaultName,
    dataBase64: bytesToBase64(bytes),
  });
}
