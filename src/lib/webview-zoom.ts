// Zoom natif de la webview (équivalent Ctrl+molette) — contrairement au
// zoom CSS, toutes les unités (100vh compris) suivent, sans trou en bas de
// page ni décalage des dropdowns portalés. Chaque écran pose son zoom au
// montage et le rend à 100 % en partant.
//
// Nécessite la permission "core:webview:allow-set-webview-zoom"
// (src-tauri/capabilities/default.json).

import { getCurrentWebview } from "@tauri-apps/api/webview";

// Les demandes de zoom sont SÉRIALISÉES et la dernière l'emporte : en
// passant du dashboard à l'éditeur (nouveau projet), le « retour à 100 % »
// du dashboard qui se démonte et le zoom de l'éditeur qui se monte
// partaient en même temps vers la webview, et il arrivait que le 100 %
// arrive en dernier — l'écran restait à 100 % alors que la barre affichait
// 70 % (bug rare signalé).
let zoomChain: Promise<void> = Promise.resolve();
let latestZoomRequest = 0;
let lastAppliedZoom: number | null = null;

export function setAppZoom(factor: number): Promise<void> {
  const request = ++latestZoomRequest;
  zoomChain = zoomChain.then(async () => {
    // Une demande plus récente existe : celle-ci est obsolète
    if (request !== latestZoomRequest) return;
    if (lastAppliedZoom === factor) return;
    try {
      await getCurrentWebview().setZoom(factor);
      lastAppliedZoom = factor;
    } catch {
      // hors Tauri (navigateur de dev) : silencieux
    }
  });
  return zoomChain;
}

/** Ré-applique le dernier zoom demandé si la webview l'a perdu (navigation) */
export function reapplyAppZoom(factor: number): Promise<void> {
  lastAppliedZoom = null;
  return setAppZoom(factor);
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
// Largeur minimale demandée pendant que la fenêtre était maximisée / en
// plein écran : appliquée au retour en mode fenêtré (voir plus bas).
let pendingMinWidth: number | null = null;
let resizeListenerInstalled = false;

/** Largeur CSS minimale de la barre d'outils de l'éditeur (+60 px depuis
 *  le bouton HiLo/LoHi de la pastille 8b/16b : en dessous, la barre
 *  recouvrait les contrôles de fenêtre à largeur minimale ; −70 px le 07/09,
 *  le bouton Compare est parti dans la liste des maps). */
export const EDITOR_TOOLBAR_MIN_CSS_WIDTH = 1025;
/** Largeur par défaut de la liste des maps de l'éditeur. */
export const EDITOR_SIDEBAR_DEFAULT_WIDTH = 335;

/** Bornes communes aux deux écrans : 50 % en bas, 100 % en haut, pas de 5 %
 *  sur les boutons − / +, valeur libre à la saisie. */
export const APP_MIN_ZOOM_PERCENT = 50;
export const APP_MAX_ZOOM_PERCENT = 100;
export const APP_ZOOM_STEP = 5;

/** Zoom d'ouverture du dashboard, réglable puis mémorisé. */
export const DASHBOARD_DEFAULT_ZOOM_PERCENT = 90;

function storedZoomPercent(key: string, fallback: number): number {
  if (typeof window === "undefined") return fallback;
  try {
    const saved = parseInt(localStorage.getItem(key) || "", 10);
    return Number.isFinite(saved)
      && saved >= APP_MIN_ZOOM_PERCENT
      && saved <= APP_MAX_ZOOM_PERCENT
      ? saved
      : fallback;
  } catch {
    return fallback;
  }
}

/** Zoom de l'éditeur mémorisé (en %), 100 hors plage ou hors navigateur. */
export function storedEditorZoomPercent(): number {
  return storedZoomPercent("zedsuite-editor-zoom", 100);
}

/** Zoom du dashboard mémorisé (en %), 90 par défaut — la valeur que l'écran
 *  a toujours eue, réglable depuis sa barre de titre depuis la 1.1.9. */
export function storedDashboardZoomPercent(): number {
  return storedZoomPercent("zedsuite-dashboard-zoom", DASHBOARD_DEFAULT_ZOOM_PERCENT);
}

/** Largeur logique minimale de la fenêtre telle que l'éditeur la demande
 *  (barre d'outils + liste des maps, à l'échelle du zoom). Le dashboard
 *  utilise la même valeur pour que la taille minimale ne change pas d'une
 *  page à l'autre. */
export function editorMinLogicalWidth(
  sidebarWidth = EDITOR_SIDEBAR_DEFAULT_WIDTH,
  zoomPercent = storedEditorZoomPercent(),
): number {
  return Math.round((EDITOR_TOOLBAR_MIN_CSS_WIDTH + sidebarWidth) * (zoomPercent / 100));
}

/** Largeur logique de l'écran courant (null hors Tauri). */
export async function monitorLogicalWidth(): Promise<number | null> {
  try {
    const { currentMonitor } = await import("@tauri-apps/api/window");
    const monitor = await currentMonitor();
    if (!monitor) return null;
    return monitor.size.toLogical(monitor.scaleFactor).width;
  } catch {
    return null;
  }
}

/** Marge gardée entre la fenêtre et le bord de l'écran (px logiques). */
const SCREEN_MARGIN = 16;

/** Zoom le plus bas que l'éditeur accepte (50 %). */
export const EDITOR_MIN_ZOOM = APP_MIN_ZOOM_PERCENT / 100;

/** Largeur logique minimale de la fenêtre : la barre d'outils + la liste
 *  des maps au zoom le plus bas. En dessous, le zoom automatique ne peut
 *  plus faire tenir la barre. */
export function editorFloorLogicalWidth(sidebarWidth = EDITOR_SIDEBAR_DEFAULT_WIDTH): number {
  return Math.round((EDITOR_TOOLBAR_MIN_CSS_WIDTH + sidebarWidth) * EDITOR_MIN_ZOOM);
}

/** Largeur logique intérieure de la fenêtre (null hors Tauri). */
export async function windowLogicalWidth(): Promise<number | null> {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    const factor = await win.scaleFactor();
    return (await win.innerSize()).toLogical(factor).width;
  } catch {
    return null;
  }
}

/** Appelle `cb` avec la largeur logique à chaque redimensionnement de la
 *  fenêtre (ancrage Windows, plein écran, poignée). Renvoie la fonction de
 *  désabonnement ; ne fait rien hors Tauri. */
export async function onWindowResized(cb: (logicalWidth: number) => void): Promise<() => void> {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    return await win.onResized(async ({ payload }) => {
      try {
        const factor = await win.scaleFactor();
        cb(payload.toLogical(factor).width);
      } catch {
        // fenêtre fermée entre-temps
      }
    });
  } catch {
    return () => {};
  }
}

/** Zoom effectif pour que `cssWidth` px CSS tiennent dans l'écran : le zoom
 *  demandé est réduit sur les écrans étroits (portables 1920x1200 à 150 %,
 *  1366x768…) au lieu de laisser la fenêtre déborder de l'écran, barre
 *  d'outils et boutons de fenêtre inaccessibles (issue #5). */
export async function fitZoomToScreen(cssWidth: number, zoom: number): Promise<number> {
  const screen = await monitorLogicalWidth();
  if (!screen || cssWidth <= 0) return zoom;
  const max = (screen - SCREEN_MARGIN) / cssWidth;
  return Math.max(0.5, Math.min(zoom, max));
}

export async function setAppMinWidth(cssWidth: number, zoom = 1): Promise<void> {
  try {
    const { getCurrentWindow, LogicalSize } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    let width = Math.max(600, Math.round(cssWidth * zoom));
    // Jamais plus large que l'écran : une taille minimale supérieure à
    // l'écran forçait une fenêtre qui débordait (issue #5).
    const screen = await monitorLogicalWidth();
    if (screen) width = Math.min(width, Math.max(600, screen - SCREEN_MARGIN));
    // Jamais de setMinSize ni de setSize en plein écran ou maximisé : sur
    // Windows, changer la contrainte de taille d'une fenêtre maximisée la
    // fait SORTIR de cet état et la décale (constaté en élargissant la liste
    // des maps). On mémorise la largeur voulue et on l'applique au retour en
    // mode fenêtré, via l'événement de redimensionnement.
    const [fullscreen, maximized] = await Promise.all([win.isFullscreen(), win.isMaximized()]);
    if (fullscreen || maximized) {
      pendingMinWidth = width;
      if (!resizeListenerInstalled) {
        resizeListenerInstalled = true;
        await win.onResized(async () => {
          if (pendingMinWidth === null) return;
          const [fs, max] = await Promise.all([win.isFullscreen(), win.isMaximized()]);
          if (fs || max) return;
          const w = pendingMinWidth;
          pendingMinWidth = null;
          await setAppMinWidth(w, 1);
        });
      }
      return;
    }
    pendingMinWidth = null;
    await win.setMinSize(new LogicalSize(width, 700));
    // La contrainte seule ne suffit pas : Windows ne l'applique qu'au
    // prochain redimensionnement manuel, une fenêtre déjà plus étroite reste
    // telle quelle. On l'élargit donc explicitement — c'est ce qui permet
    // d'élargir la liste des maps sans repousser la barre d'outils hors du
    // cadre.
    const factor = await win.scaleFactor();
    const current = (await win.innerSize()).toLogical(factor);
    if (current.width < width) {
      await win.setSize(new LogicalSize(width, current.height));
    }
  } catch {
    // hors Tauri : silencieux
  }
}
