"use client";

// Custom window controls (minimize / maximize / close) for the frameless
// window — the native Windows title bar is disabled (decorations: false in
// tauri.conf.json), so every page header embeds these instead.
// Theme-aware: follows the ambient ThemeProvider (dashboard or editor).

import { useEffect, useState } from "react";
import { Minus, Square, Copy, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useThemeOptional } from "@/contexts/theme-context";

// strong : icônes en noir (thème clair sur image personnalisée, sans pastille)
export function WindowControls({ strong = false }: { strong?: boolean } = {}) {
  const [isMaximized, setIsMaximized] = useState(false);
  const L = (useThemeOptional()?.theme ?? "default") === "light";

  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    let disposed = false;

    win.isMaximized().then(setIsMaximized).catch(() => {});
    win
      .onResized(async () => {
        try {
          setIsMaximized(await win.isMaximized());
        } catch {
          // window closing
        }
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {});

    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  }, []);

  const win = () => getCurrentWindow();

  // flex-shrink-0 : les trois boutons gardent leur largeur quelle que soit
  // la place restante — ils ne doivent jamais être rognés ni poussés hors
  // de l'écran quand on réduit la fenêtre.
  const baseBtn = `h-8 w-10 flex-shrink-0 flex items-center justify-center rounded-md transition-colors ${
    L
      ? (strong ? "text-slate-900 hover:text-black hover:bg-black/10" : "text-slate-500 hover:text-black hover:bg-black/10")
      : "text-slate-400 hover:text-white hover:bg-white/10"
  }`;

  return (
    <div className="flex items-center gap-0.5 ml-2 flex-shrink-0">
      <button onClick={() => win().minimize()} className={baseBtn} title="Minimize">
        <Minus className="w-4 h-4" />
      </button>
      <button
        onClick={() => win().toggleMaximize()}
        className={baseBtn}
        title={isMaximized ? "Restore" : "Maximize"}
      >
        {isMaximized ? (
          <Copy className="w-3.5 h-3.5 -scale-x-100" />
        ) : (
          <Square className="w-3.5 h-3.5" />
        )}
      </button>
      <button
        onClick={() => win().close()}
        className={`h-8 w-10 flex-shrink-0 flex items-center justify-center rounded-md transition-colors hover:text-white hover:bg-red-500/90 ${
          L ? (strong ? "text-slate-900" : "text-slate-500") : "text-slate-400"
        }`}
        title="Close"
      >
        <X className="w-4 h-4" />
      </button>
    </div>
  );
}
