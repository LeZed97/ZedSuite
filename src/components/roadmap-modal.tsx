"use client";

// Fenêtre « Feuille de route » du dashboard : même habillage que la fenêtre
// info, contenu = ROADMAP.md du dépôt (à jour depuis GitHub, sinon la copie
// livrée avec la version). L'auteur alimente le fichier, l'app l'affiche.
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { MODAL_GLASS, MODAL_GLASS_LIGHT } from "@/lib/modal-glass";
import { openExternal } from "@/lib/open-external";
import {
  loadRoadmap,
  parseInline,
  parseRoadmapMarkdown,
  ZEDSUITE_ROADMAP_URL,
  type RoadmapContent,
} from "@/lib/roadmap";

interface RoadmapModalProps {
  open: boolean;
  onClose: () => void;
  theme: string;
  /** Langue de l'app (EN, FR, ES, IT, DE) : la feuille de route s'affiche dans cette langue */
  language: string;
  /** Version installée, affichée sous le titre comme dans la fenêtre info */
  version?: string;
  labels: {
    title: string;
    loading: string;
    openOnGithub: string;
    close: string;
  };
}

export function RoadmapModal({ open, onClose, theme, language, version, labels }: RoadmapModalProps) {
  const [content, setContent] = useState<RoadmapContent | null>(null);
  const [loading, setLoading] = useState(false);
  const light = theme === "light";

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setLoading(true);
    void loadRoadmap(language).then((c) => {
      if (cancelled) return;
      setContent(c);
      setLoading(false);
    });
    return () => {
      cancelled = true;
    };
  }, [open, language]);

  if (!open) return null;

  const textMain = light ? "text-slate-900" : "text-white";
  const textSoft = light ? "text-slate-600" : "text-white/60";
  const textBody = light ? "text-slate-700" : "text-white/80";
  const linkCls = `underline underline-offset-2 ${light ? "text-slate-800 hover:text-black" : "text-white/80 hover:text-white"}`;

  const renderInline = (text: string, key: string) =>
    parseInline(text).map((seg, i) => {
      const k = `${key}-${i}`;
      switch (seg.type) {
        case "bold":
          return <strong key={k} className={textMain}>{seg.text}</strong>;
        case "italic":
          return <em key={k}>{seg.text}</em>;
        case "code":
          return <code key={k} className={`px-1 rounded text-[0.85em] ${light ? "bg-black/[0.06]" : "bg-white/[0.08]"}`}>{seg.text}</code>;
        case "link":
          return (
            <button key={k} type="button" onClick={() => void openExternal(seg.url)} className={linkCls}>
              {seg.text}
            </button>
          );
        default:
          return <span key={k}>{seg.text}</span>;
      }
    });

  const blocks = content ? parseRoadmapMarkdown(content.markdown) : [];

  return (
    <div
      className="fixed inset-0 flex items-center justify-center backdrop-blur-sm z-[100]"
      style={{ backgroundColor: "#000000a2", animation: "fadeIn 0.2s ease-out forwards" }}
      onClick={onClose}
    >
      <div
        className="rounded-lg shadow-2xl p-6 max-w-5xl w-full mx-4 border max-h-[85vh] flex flex-col"
        style={{ ...(light ? MODAL_GLASS_LIGHT : MODAL_GLASS), animation: "scaleIn 0.2s ease-out forwards" }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-3 mb-3">
          {/* Même icône que la fenêtre À propos */}
          <img src="/zedsuite-icon.svg" alt="ZedSuite" className="w-10 h-10 object-contain" />
          <div className="min-w-0">
            <h3 className={`text-lg font-semibold leading-tight ${textMain}`}>{labels.title}</h3>
            <p className={`text-xs ${textSoft}`}>{version ? `ZedSuite v${version}` : "ZedSuite"} — Open source, GPL-3.0</p>
          </div>
        </div>

        <div className={`flex-1 overflow-y-auto pr-1 text-sm ${textBody}`}>
          {loading && <p className={textSoft}>{labels.loading}</p>}
          {!loading &&
            blocks.map((b, i) => {
              const key = `b${i}`;
              switch (b.type) {
                case "heading":
                  // Le titre de niveau 1 du fichier fait doublon avec celui de la fenêtre
                  if (b.level === 1) return null;
                  return (
                    <h4 key={key} className={`font-semibold ${b.level === 2 ? "text-sm mt-4 mb-1.5" : "text-sm mt-3 mb-1"} ${textMain}`}>
                      {renderInline(b.text, key)}
                    </h4>
                  );
                case "list":
                  return (
                    <ul key={key} className="space-y-1 mb-2">
                      {b.items.map((it, j) => (
                        <li key={`${key}-${j}`} className="flex gap-2">
                          <span className={textSoft}>•</span>
                          <span>{renderInline(it, `${key}-${j}`)}</span>
                        </li>
                      ))}
                    </ul>
                  );
                case "rule":
                  return <hr key={key} className={`my-3 ${light ? "border-black/[0.08]" : "border-white/[0.08]"}`} />;
                default:
                  return (
                    <p key={key} className="mb-2">
                      {renderInline(b.text, key)}
                    </p>
                  );
              }
            })}
        </div>

        <div className="relative flex justify-between items-center mt-4">
          <button type="button" onClick={() => void openExternal(ZEDSUITE_ROADMAP_URL)} className={`text-sm ${linkCls}`}>
            {labels.openOnGithub}
          </button>
          {/* Logo ZedPerf centré en bas, comme la fenêtre À propos :
              noir/rouge en thème clair, blanc/rouge en thème sombre */}
          <img
            src={light ? "/zedperf-light.png" : "/zedperf-dark.png"}
            alt="ZedPerf"
            className="absolute left-1/2 -translate-x-1/2 h-6 w-auto object-contain pointer-events-none select-none"
          />
          <Button
            size="sm"
            className="h-9 px-6 text-white bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400"
            onClick={onClose}
          >
            {labels.close}
          </Button>
        </div>
      </div>
    </div>
  );
}
