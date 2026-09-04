// Zoom natif de la webview (équivalent Ctrl+molette) — contrairement au
// zoom CSS, toutes les unités (100vh compris) suivent, sans trou en bas de
// page ni décalage des dropdowns portalés. Chaque écran pose son zoom au
// montage et le rend à 100 % en partant.
//
// Nécessite la permission "core:webview:allow-set-webview-zoom"
// (src-tauri/capabilities/default.json).

import { getCurrentWebview } from "@tauri-apps/api/webview";

export async function setAppZoom(factor: number): Promise<void> {
  try {
    await getCurrentWebview().setZoom(factor);
  } catch {
    // hors Tauri (navigateur de dev) : silencieux
  }
}

/**
 * Largeur minimale de la fenêtre, exprimée en pixels CSS puis convertie
 * selon le zoom courant (au zoom z, 1 px CSS occupe z px à l'écran).
 *
 * Dézoomer doit donc permettre de rétrécir davantage la fenêtre : le contenu
 * occupe moins de place réelle. Le calcul ne dépend QUE du zoom — aucune
 * mesure du DOM, aucun observateur de taille : c'est ce qui évite la boucle
 * « la fenêtre change → on remesure → la fenêtre change » qui bloquait le
 * redimensionnement.
 */
export async function setAppMinWidth(cssWidth: number, zoom = 1): Promise<void> {
  try {
    const { getCurrentWindow, LogicalSize } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    const width = Math.max(600, Math.round(cssWidth * zoom));
    await win.setMinSize(new LogicalSize(width, 700));

    // La contrainte seule ne suffit pas : Windows ne l'applique qu'au
    // prochain redimensionnement manuel, une fenêtre déjà plus étroite reste
    // telle quelle. On l'élargit donc explicitement — c'est ce qui permet
    // d'élargir la liste des maps sans repousser la barre d'outils hors du
    // cadre.
    // Jamais de redimensionnement explicite en plein écran ou maximisé : sur
    // Windows, un setSize dans ces états fait SORTIR du plein écran et décale
    // la fenêtre (constaté en élargissant la liste des maps). La contrainte
    // de taille minimale reste posée et s'appliquera au retour en fenêtré.
    const [fullscreen, maximized] = await Promise.all([win.isFullscreen(), win.isMaximized()]);
    if (fullscreen || maximized) return;
    const factor = await win.scaleFactor();
    const current = (await win.innerSize()).toLogical(factor);
    if (current.width < width) {
      await win.setSize(new LogicalSize(width, current.height));
    }
  } catch {
    // hors Tauri : silencieux
  }
}

