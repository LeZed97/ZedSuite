// Presse-papiers du SYSTÈME (celui d'Excel, d'EDC Suite…), lu et écrit par la
// coque Tauri (plugin clipboard-manager) : pas de permission navigateur à
// demander, même comportement sous Windows, macOS et Linux. Hors Tauri (dev
// dans un navigateur), repli sur navigator.clipboard quand il est disponible.

import { isTauri } from "./update";

export async function writeSystemClipboardText(text: string): Promise<boolean> {
  try {
    if (isTauri()) {
      const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
      await writeText(text);
      return true;
    }
    if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // presse-papiers indisponible : la copie interne reste utilisable
  }
  return false;
}

/** Texte du presse-papiers système, ou null s'il est vide / illisible. */
export async function readSystemClipboardText(): Promise<string | null> {
  try {
    if (isTauri()) {
      const { readText } = await import("@tauri-apps/plugin-clipboard-manager");
      const t = await readText();
      return t ? t : null;
    }
    if (typeof navigator !== "undefined" && navigator.clipboard?.readText) {
      const t = await navigator.clipboard.readText();
      return t ? t : null;
    }
  } catch {
    // pas de texte (image, contenu binaire) ou lecture refusée
  }
  return null;
}
