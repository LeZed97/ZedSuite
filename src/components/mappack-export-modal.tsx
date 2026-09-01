"use client";

import { X, FileJson, RefreshCw, Check, Coins } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useI18n } from "@/contexts/i18n-context";
import { MODAL_GLASS, MODAL_GLASS_LIGHT } from "@/lib/modal-glass";
import { useThemeOptional } from "@/contexts/theme-context";

interface MappackExportModalProps {
  onClose: () => void;
  onConfirm: () => void;
  cost: number;
  isClosing?: boolean;
  isExporting?: boolean;
  exportComplete?: boolean;
}

export function MappackExportModal({
  onClose,
  onConfirm,
  cost,
  isClosing = false,
  isExporting = false,
  exportComplete = false,
}: MappackExportModalProps) {
  const { t } = useI18n();
  // Suit le thème de l'écran hôte (dashboard ou éditeur)
  const themeCtx = useThemeOptional();
  const L = (themeCtx?.theme ?? "default") === "light";


  return (
    <div
      className="fixed inset-0 z-[80] flex items-center justify-center backdrop-blur-sm"
      style={{
        backgroundColor: '#000000a2',
        animation: isClosing ? 'backdropFadeOut 0.2s ease-out forwards' : 'backdropFadeIn 0.2s ease-out forwards'
      }}
    >
      <div
        className="relative w-full max-w-md overflow-hidden"
        style={{
          animation: isClosing ? 'modalCollapse 0.2s ease-out forwards' : 'modalExpand 0.2s ease-out forwards'
        }}
      >
        {/* Close button - only on initial choice screen */}
        {!isExporting && !exportComplete && (
          <button
            onClick={onClose}
            className={`absolute top-4 right-4 z-10 p-2 rounded-lg transition-colors ${L ? "hover:bg-black/5" : "hover:bg-white/5"}`}
            style={{ color: L ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.6)' }}
          >
            <X className="w-5 h-5" />
          </button>
        )}

        {/* Modal Content - Same style as ChecksumModal */}
        <div className="border rounded-lg p-8" style={L ? MODAL_GLASS_LIGHT : MODAL_GLASS}>
          {/* Header with icon */}
          <div className="flex flex-col items-center mb-6">
            {/* Circular icon - animate during export. Default/exporting use the
                editor logo's red gradient, translucent like the checksum
                modal's blue disc; success stays green. */}
            <div
              className={`w-20 h-20 rounded-full flex items-center justify-center mb-6 ${
                exportComplete
                  ? ''
                  : 'bg-gradient-to-br from-red-600/20 via-red-500/20 to-orange-500/20'
              }`}
              style={exportComplete ? { backgroundColor: 'rgba(34, 197, 94, 0.15)' } : undefined}
            >
              {exportComplete ? (
                <Check className="w-10 h-10" style={{ color: '#22c55e' }} />
              ) : isExporting ? (
                <RefreshCw className={`w-10 h-10 animate-spin ${L ? "text-slate-700" : "text-white"}`} />
              ) : (
                <FileJson className={`w-10 h-10 ${L ? "text-slate-700" : "text-white"}`} />
              )}
            </div>

            {/* Title */}
            <h2 className={`text-xl font-semibold text-center ${L ? "text-slate-900" : "text-white"}`}>
              {isExporting
                ? t.mappackExport.exporting
                : exportComplete
                  ? t.mappackExport.complete
                  : t.mappackExport.title}
            </h2>

            {/* Details - hidden during export/complete */}
            {!isExporting && !exportComplete && (
              <div className="mt-4 text-center space-y-1">
                <p className="text-sm" style={{ color: L ? 'rgba(0, 0, 0, 0.55)' : 'rgba(255, 255, 255, 0.6)' }}>
                  {t.mappackExport.description}
                </p>
                {/* Local app: export is free and unlimited — the cost line only
                    shows when a cost is actually configured (never by default) */}
                {cost > 0 && (
                  <p className="text-sm font-medium flex items-center justify-center gap-1.5" style={{ color: '#3b82f6' }}>
                    <Coins className="w-4 h-4 text-blue-400" />
                    {t.mappackExport.cost.replace("{cost}", String(cost))}
                  </p>
                )}
              </div>
            )}
          </div>

          {/* Action buttons - hidden during export */}
          {!isExporting && !exportComplete && (
            <div className="flex gap-3">
              <Button
                onClick={onClose}
                variant="outline"
                className="flex-1"
              >
                {t.mappackExport.cancel}
              </Button>
              <Button
                onClick={onConfirm}
                className="flex-1 bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400 text-white"
              >
                {t.mappackExport.confirm}
              </Button>
            </div>
          )}
        </div>
      </div>

      <style jsx>{`
        @keyframes backdropFadeIn {
          from { opacity: 0; }
          to { opacity: 1; }
        }
        @keyframes backdropFadeOut {
          from { opacity: 1; }
          to { opacity: 0; }
        }
        @keyframes modalExpand {
          from {
            opacity: 0;
            transform: scale(0.95);
          }
          to {
            opacity: 1;
            transform: scale(1);
          }
        }
        @keyframes modalCollapse {
          from {
            opacity: 1;
            transform: scale(1);
          }
          to {
            opacity: 0;
            transform: scale(0.95);
          }
        }
      `}</style>
    </div>
  );
}
