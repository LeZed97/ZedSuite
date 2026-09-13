// Logo personnalisé des feuilles de banc (PDF du dyno virtuel) : l'image
// choisie par l'utilisateur dans les Paramètres remplace le logo « ZedSuite »
// de l'en-tête. Même mécanique que le fond d'écran personnalisé
// (lib/custom-wallpaper) : image réduite côté client, stockée en data URL
// dans le localStorage, événement fenêtre pour les pages ouvertes.
//
// Ré-encodée en PNG (et non JPEG) pour garder la transparence : un logo
// détouré doit rester propre sur la page sombre comme sur la page claire.

const KEY = "zedsuiteReportLogo";

export const REPORT_LOGO_EVENT = "zedsuite-report-logo";

/** Bornes de l'image stockée : largement suffisant pour ~70 × 11 mm à 300 dpi. */
const MAX_W = 1200;
const MAX_H = 400;

export function getReportLogo(): string | null {
  try {
    return localStorage.getItem(KEY);
  } catch {
    return null;
  }
}

export function clearReportLogo(): void {
  try {
    localStorage.removeItem(KEY);
  } catch {
    // stockage indisponible : rien à faire
  }
  window.dispatchEvent(new CustomEvent(REPORT_LOGO_EVENT));
}

/**
 * Lit le fichier image, le réduit à 1200 × 400 px max (ratio conservé) et le
 * stocke en PNG. Rejette si le fichier n'est pas une image lisible ou si le
 * stockage échoue (quota).
 */
export function setReportLogoFromFile(file: File): Promise<void> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      URL.revokeObjectURL(url);
      try {
        const scale = Math.min(1, MAX_W / img.width, MAX_H / img.height);
        const w = Math.max(1, Math.round(img.width * scale));
        const h = Math.max(1, Math.round(img.height * scale));
        const canvas = document.createElement("canvas");
        canvas.width = w;
        canvas.height = h;
        const ctx = canvas.getContext("2d");
        if (!ctx) throw new Error("canvas 2d context unavailable");
        ctx.drawImage(img, 0, 0, w, h);
        localStorage.setItem(KEY, canvas.toDataURL("image/png"));
        window.dispatchEvent(new CustomEvent(REPORT_LOGO_EVENT));
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

/** Valeur courante + abonnement aux changements (pour un état React). */
export function subscribeReportLogo(onChange: (dataUrl: string | null) => void): () => void {
  const handler = () => onChange(getReportLogo());
  window.addEventListener(REPORT_LOGO_EVENT, handler);
  return () => window.removeEventListener(REPORT_LOGO_EVENT, handler);
}
