// Fond d'écran personnalisé (image choisie par l'utilisateur) pour le
// dashboard et l'éditeur. L'image est redimensionnée côté client (max
// 2560 px de large) et ré-encodée en JPEG pour tenir sans risque dans le
// localStorage, puis servie en data URL. Un événement fenêtre notifie les
// pages ouvertes pour une mise à jour immédiate.

export type WallpaperScope = "dashboard" | "editor";

const KEYS: Record<WallpaperScope, string> = {
  dashboard: "zedsuiteWallpaperDashboard",
  editor: "zedsuiteWallpaperEditor",
};

export const CUSTOM_WALLPAPER_EVENT = "zedsuite-custom-wallpaper";

export function getCustomWallpaper(scope: WallpaperScope): string | null {
  try {
    return localStorage.getItem(KEYS[scope]);
  } catch {
    return null;
  }
}

export function clearCustomWallpaper(scope: WallpaperScope): void {
  try {
    localStorage.removeItem(KEYS[scope]);
  } catch {
    // stockage indisponible : rien à faire
  }
  window.dispatchEvent(new CustomEvent(CUSTOM_WALLPAPER_EVENT, { detail: { scope } }));
}

/**
 * Lit le fichier image, le réduit à 2560 px max (ratio conservé) et le
 * stocke en JPEG (qualité 0.85). Rejette si le fichier n'est pas une image
 * lisible ou si le stockage échoue (quota).
 */
export function setCustomWallpaperFromFile(
  scope: WallpaperScope,
  file: File
): Promise<void> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      URL.revokeObjectURL(url);
      try {
        const maxSide = 2560;
        const scale = Math.min(1, maxSide / Math.max(img.width, img.height));
        const w = Math.max(1, Math.round(img.width * scale));
        const h = Math.max(1, Math.round(img.height * scale));
        const canvas = document.createElement("canvas");
        canvas.width = w;
        canvas.height = h;
        const ctx = canvas.getContext("2d");
        if (!ctx) throw new Error("canvas 2d context unavailable");
        ctx.drawImage(img, 0, 0, w, h);
        const dataUrl = canvas.toDataURL("image/jpeg", 0.85);
        localStorage.setItem(KEYS[scope], dataUrl);
        window.dispatchEvent(
          new CustomEvent(CUSTOM_WALLPAPER_EVENT, { detail: { scope } })
        );
        resolve();
      } catch (e) {
        reject(e);
      }
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("unreadable image"));
    };
    img.src = url;
  });
}

/** Hook-friendly reader: valeur courante + abonnement aux changements. */
export function subscribeCustomWallpaper(
  scope: WallpaperScope,
  onChange: (dataUrl: string | null) => void
): () => void {
  const handler = (e: Event) => {
    const detail = (e as CustomEvent).detail as { scope?: WallpaperScope } | undefined;
    if (!detail || detail.scope === scope) {
      onChange(getCustomWallpaper(scope));
    }
  };
  window.addEventListener(CUSTOM_WALLPAPER_EVENT, handler);
  return () => window.removeEventListener(CUSTOM_WALLPAPER_EVENT, handler);
}
