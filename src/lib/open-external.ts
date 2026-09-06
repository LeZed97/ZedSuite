// Ouvre une page GitHub de ZedSuite dans le navigateur par défaut (la webview
// n'a pas d'onglets) : commande Rust limitée au dépôt ZedSuite.
import { invoke } from "@tauri-apps/api/core";

export const ZEDSUITE_RELEASES_URL = "https://github.com/LeZed97/ZedSuite/releases";
export const ZEDSUITE_REPO_URL = "https://github.com/LeZed97/ZedSuite";

export async function openExternal(url: string): Promise<void> {
  try {
    await invoke("open_external_url", { url });
  } catch {
    // hors Tauri (navigateur de dev) : ouvrir dans un onglet
    try {
      window.open(url, "_blank", "noopener");
    } catch {
      /* silencieux */
    }
  }
}
