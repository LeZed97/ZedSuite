"use client";

import { useEffect, useState } from "react";
import FloatingLines from "@/components/FloatingLines";
import { getCustomWallpaper, subscribeCustomWallpaper } from "@/lib/custom-wallpaper";
import type { UserSettings } from "@/contexts/settings-context";
import type { Theme } from "@/contexts/theme-context";

// Fond d'écran du dashboard, partagé avec la page Paramètres pour que les
// deux écrans aient exactement le même rendu (traits animés, halos, image
// personnalisée…) quel que soit le choix fait dans les Paramètres.

export type DashboardWallpaper = "lines" | "lines-light" | "editor" | "white" | "black" | "custom";

const FILM_GRAIN = `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='140' height='140'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2'/%3E%3C/filter%3E%3Crect width='140' height='140' filter='url(%23n)' opacity='0.55'/%3E%3C/svg%3E")`;

/**
 * Résout le fond effectif du dashboard à partir du réglage utilisateur et du
 * thème. Choix manuel dans les Paramètres, sinon automatique selon le thème
 * (défaut → traits animés, clair → blanc, OLED → noir).
 * Contraintes : thème clair → fond blanc uniquement ; OLED → jamais blanc.
 * Le fond PERSONNALISÉ (image utilisateur) est utilisable avec TOUS les
 * thèmes ; sans image enregistrée il retombe sur l'automatique.
 * L'image personnalisée (lib/custom-wallpaper) est suivie en direct pour que
 * le choix fait dans les Paramètres s'applique sans recharger la page.
 */
export function useDashboardWallpaper(
  stored: UserSettings["dashboardWallpaper"],
  theme: Theme,
): { wallpaper: DashboardWallpaper; isLight: boolean; pageBg: string; customWallpaper: string | null } {
  const [customWallpaper, setCustomWallpaper] = useState<string | null>(null);
  useEffect(() => {
    setCustomWallpaper(getCustomWallpaper("dashboard"));
    return subscribeCustomWallpaper("dashboard", setCustomWallpaper);
  }, []);

  let wallpaper: string =
    stored === "custom" && customWallpaper
      ? "custom"
      : theme === "light"
        ? (stored === "lines-light" ? "lines-light" : "white")
        : stored && stored !== "auto"
          ? stored
          : theme === "oled"
            ? "black"
            : "lines";
  // Le blanc et les traits clairs n'existent que sur le thème clair
  if (wallpaper !== "custom") {
    if (theme !== "light" && (wallpaper === "white" || wallpaper === "lines-light")) {
      wallpaper = theme === "oled" ? "black" : "lines";
    }
    if (!["lines", "lines-light", "editor", "white", "black"].includes(wallpaper)) {
      wallpaper = theme === "oled" ? "black" : "lines";
    }
  }
  // Les textes s'adaptent à la luminosité du fond effectif (pas au thème seul)
  // — sur une image personnalisée, on suit le thème choisi.
  const isLight = wallpaper === "custom" ? theme === "light" : (wallpaper === "white" || wallpaper === "lines-light");
  const pageBg = wallpaper === "black" ? "#000000" : isLight ? "#eef1f6" : "#0a0b0f";
  return { wallpaper: wallpaper as DashboardWallpaper, isLight, pageBg, customWallpaper };
}

/** Couches fixes (z-0) du fond : à placer en premier enfant du conteneur de page. */
export function DashboardBackground({
  wallpaper,
  theme,
  isLight,
  customWallpaper,
}: {
  wallpaper: DashboardWallpaper;
  theme: Theme;
  isLight: boolean;
  customWallpaper: string | null;
}) {
  return (
    <>
      {/* Fond d'écran du thème par défaut : les traits animés rouge/blanc */}
      {wallpaper === "lines" && (
        <>
          <div className="fixed inset-0 z-0" style={{ opacity: 0.45 }}>
            <FloatingLines
              linesGradient={['#9a3412', '#7f1d1d', '#312e81']}
              enabledWaves={['top', 'middle', 'bottom']}
              lineCount={[6, 8, 6]}
              lineDistance={[5, 5, 6]}
              animationSpeed={0.8}
              interactive={false}
              parallax={false}
              parallaxStrength={0.15}
            />
          </div>
          {/* Grain de film par-dessus les traits (texture d'origine) */}
          <div aria-hidden className="fixed inset-0 z-0 pointer-events-none" style={{ opacity: 0.05, backgroundImage: FILM_GRAIN }} />
        </>
      )}
      {/* Traits animés, déclinaison claire : canvas inversé en CSS, couleurs
          pré-inversées pour retomber sur orange/rouge/indigo pastel */}
      {wallpaper === "lines-light" && (
        <>
          <div className="fixed inset-0 z-0" style={{ opacity: 0.5, filter: 'invert(1)' }}>
            <FloatingLines
              linesGradient={['#046dc3', '#078e8e', '#7e7307']}
              enabledWaves={['top', 'middle', 'bottom']}
              lineCount={[6, 8, 6]}
              lineDistance={[5, 5, 6]}
              animationSpeed={0.8}
              interactive={false}
              parallax={false}
              parallaxStrength={0.15}
            />
          </div>
          <div aria-hidden className="fixed inset-0 z-0 pointer-events-none" style={{ opacity: 0.03, backgroundImage: FILM_GRAIN }} />
        </>
      )}
      {/* Image personnalisée de l'utilisateur : le rendu de la zone de
          travail de l'éditeur — image nette en cover, voile du fond puis le
          même fond translucide que le workspace, selon le thème. Voiles
          réduits de moitié par rapport à l'éditeur (l'image restait trop
          assombrie sur le dashboard). */}
      {wallpaper === "custom" && customWallpaper && (
        <>
          <div
            aria-hidden
            className="fixed inset-0 z-0 pointer-events-none"
            style={{
              backgroundImage: `url(${customWallpaper})`,
              backgroundSize: "cover",
              backgroundPosition: "center",
            }}
          />
          <div
            aria-hidden
            className="fixed inset-0 z-0 pointer-events-none"
            style={{ backgroundColor: isLight ? "rgba(255,255,255,0.11)" : "rgba(0,0,0,0.18)" }}
          />
          <div
            aria-hidden
            className="fixed inset-0 z-0 pointer-events-none"
            style={{
              backgroundColor:
                theme === "light"
                  ? "rgba(233,236,241,0.28)"
                  : theme === "oled"
                    ? "rgba(0,0,0,0.22)"
                    : "rgba(18,21,29,0.22)",
            }}
          />
        </>
      )}
      {/* Fond d'écran par défaut de l'éditeur : trois halos flous + grain */}
      {wallpaper === "editor" && (
        <>
          <div aria-hidden className="fixed inset-0 z-0 pointer-events-none overflow-hidden" style={{ opacity: 0.65 }}>
            <div className="absolute rounded-full" style={{ width: 520, height: 520, left: -120, top: -140, filter: 'blur(90px)', background: 'radial-gradient(circle, #ef444488, transparent 70%)' }} />
            <div className="absolute rounded-full" style={{ width: 620, height: 620, right: -160, top: 120, filter: 'blur(90px)', background: 'radial-gradient(circle, #2563eb77, transparent 70%)' }} />
            <div className="absolute rounded-full" style={{ width: 480, height: 480, left: '32%', bottom: -200, filter: 'blur(90px)', background: 'radial-gradient(circle, #7c3aed66, transparent 70%)' }} />
          </div>
          <div aria-hidden className="fixed inset-0 z-0 pointer-events-none" style={{ opacity: 0.05, backgroundImage: FILM_GRAIN }} />
        </>
      )}
    </>
  );
}
