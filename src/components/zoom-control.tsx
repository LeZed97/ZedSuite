"use client";

import { useState } from "react";
import { Minus, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useTheme } from "@/contexts/theme-context";
import { APP_MAX_ZOOM_PERCENT, APP_MIN_ZOOM_PERCENT, APP_ZOOM_STEP } from "@/lib/webview-zoom";

interface ZoomControlProps {
  /** Valeur affichée, en pourcent */
  percent: number;
  /** Bouton − / + : appelé avec −5 ou +5 */
  onStep: (delta: number) => void;
  /** Valeur saisie au clavier (déjà bornée par l'appelant) */
  onSet: (percent: number) => void;
  /** Marges de placement, propres à chaque écran */
  className?: string;
}

/**
 * Pastille de zoom « − valeur + », partagée par la barre d'outils de
 * l'éditeur et l'en-tête du dashboard : un seul rendu pour les deux écrans,
 * sinon ils divergent au premier ajustement (le dashboard avait perdu la
 * bordure et gagné 4 px de hauteur en étant recopié à la main).
 *
 * Le pas des boutons vaut 5 %, la valeur se saisit en cliquant dessus, et les
 * bornes 50–100 % viennent de lib/webview-zoom.
 */
export function ZoomControl({ percent, onStep, onSet, className = "" }: ZoomControlProps) {
  const { theme } = useTheme();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");

  const borderColor =
    theme === "light"
      ? "rgba(15,20,35,0.1)"
      : theme === "oled"
        ? "rgba(255,255,255,0.06)"
        : "rgba(255,255,255,0.08)";
  const background =
    theme === "light"
      ? "rgba(255,255,255,0.6)"
      : theme === "oled"
        ? "rgba(20,20,23,0.85)"
        : "rgba(22,25,34,0.55)";
  const textColor = theme === "light" ? "#000000" : "rgba(255, 255, 255, 0.7)";
  const hoverClass = theme === "light" ? "hover:bg-black/10" : "hover:bg-white/10";
  const range = `${APP_MIN_ZOOM_PERCENT} – ${APP_MAX_ZOOM_PERCENT} %`;

  const commit = () => {
    const v = parseInt(draft, 10);
    if (Number.isFinite(v)) onSet(v);
    setEditing(false);
  };

  return (
    <div
      className={`flex items-center rounded-lg px-1 flex-shrink-0 ${className}`}
      style={{ background, border: `1px solid ${borderColor}` }}
    >
      <Button
        variant="ghost"
        size="sm"
        className={`h-7 w-7 p-0 ${hoverClass}`}
        onClick={() => onStep(-APP_ZOOM_STEP)}
        title="Zoom −"
        style={{ color: textColor }}
      >
        <Minus className="w-3.5 h-3.5" />
      </Button>
      {editing ? (
        <input
          autoFocus
          type="text"
          inputMode="numeric"
          value={draft}
          onChange={(e) => setDraft(e.target.value.replace(/[^0-9]/g, "").slice(0, 3))}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
            else if (e.key === "Escape") setEditing(false);
            e.stopPropagation();
          }}
          className="text-xs tabular-nums w-9 text-center bg-transparent outline-none rounded border"
          style={{ color: textColor, borderColor }}
        />
      ) : (
        <button
          type="button"
          className="text-xs tabular-nums w-9 text-center select-none rounded hover:opacity-80"
          style={{ color: textColor }}
          title={range}
          onClick={() => {
            setDraft(String(percent));
            setEditing(true);
          }}
        >
          {percent}%
        </button>
      )}
      <Button
        variant="ghost"
        size="sm"
        className={`h-7 w-7 p-0 ${hoverClass}`}
        onClick={() => onStep(APP_ZOOM_STEP)}
        title="Zoom +"
        style={{ color: textColor }}
      >
        <Plus className="w-3.5 h-3.5" />
      </Button>
    </div>
  );
}
