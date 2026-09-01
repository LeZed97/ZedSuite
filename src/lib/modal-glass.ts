import type { CSSProperties } from "react";

/**
 * Style verre partagé par toutes les fenêtres modales de l'app
 * (thème settings + glassmorphism). Le backdrop reste #000000a2 + blur ;
 * le conteneur porte la surface verre + bordure hairline.
 *
 * Note : backdrop-filter fait du conteneur le containing block de tout
 * descendant en position:fixed — les dropdowns StyledSelect sont portalés
 * vers document.body, donc sans risque ici.
 */
export const MODAL_GLASS: CSSProperties = {
  backgroundColor: "rgba(22, 25, 34, 0.55)",
  borderColor: "rgba(255, 255, 255, 0.08)",
  backdropFilter: "blur(18px) saturate(140%)",
  WebkitBackdropFilter: "blur(18px) saturate(140%)",
};

/** Variante claire de la surface modale (menus affichés en thème clair) */
export const MODAL_GLASS_LIGHT: CSSProperties = {
  backgroundColor: "rgba(255, 255, 255, 0.88)",
  borderColor: "rgba(0, 0, 0, 0.10)",
  backdropFilter: "blur(18px) saturate(140%)",
  WebkitBackdropFilter: "blur(18px) saturate(140%)",
};

/**
 * Style verre des pastilles de notification (toasts) en bas de l'écran —
 * même langage que les modales, un peu plus opaque pour rester lisible
 * au-dessus de n'importe quel contenu. L'accent de statut est porté par
 * le disque d'icône teinté (vert/orange/rouge), pas par le fond.
 */
export const TOAST_GLASS: CSSProperties = {
  backgroundColor: "rgba(22, 25, 34, 0.72)",
  border: "1px solid rgba(255, 255, 255, 0.1)",
  backdropFilter: "blur(18px) saturate(140%)",
  WebkitBackdropFilter: "blur(18px) saturate(140%)",
  boxShadow: "0 8px 24px rgba(0, 0, 0, 0.35)",
};

/** Variante claire des pastilles de notification (thème clair) */
export const TOAST_GLASS_LIGHT: CSSProperties = {
  backgroundColor: "rgba(255, 255, 255, 0.9)",
  border: "1px solid rgba(0, 0, 0, 0.12)",
  backdropFilter: "blur(18px) saturate(140%)",
  WebkitBackdropFilter: "blur(18px) saturate(140%)",
  boxShadow: "0 8px 24px rgba(0, 0, 0, 0.15)",
};

/** Variante thémée pour les modales qui suivent le thème de l'éditeur. */
export function getModalGlassStyle(
  theme: "default" | "light" | "oled" = "default"
): CSSProperties {
  if (theme === "light") {
    return {
      backgroundColor: "rgba(255, 255, 255, 0.78)",
      borderColor: "rgba(0, 0, 0, 0.1)",
      backdropFilter: "blur(18px) saturate(140%)",
      WebkitBackdropFilter: "blur(18px) saturate(140%)",
    };
  }
  if (theme === "oled") {
    // OLED : verre quasi opaque + blur réduit, cohérent avec le chrome éditeur
    return {
      backgroundColor: "rgba(20, 20, 23, 0.85)",
      borderColor: "rgba(255, 255, 255, 0.06)",
      backdropFilter: "blur(6px)",
      WebkitBackdropFilter: "blur(6px)",
    };
  }
  return MODAL_GLASS;
}
