"use client";

import { X, RefreshCw, Check, AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useI18n } from "@/contexts/i18n-context";
import { MODAL_GLASS, MODAL_GLASS_LIGHT } from "@/lib/modal-glass";
import { useThemeOptional } from "@/contexts/theme-context";

export type ExportChecksumStatus = "checking" | "ok" | "bad";

interface ChecksumModalProps {
  onClose: () => void;
  onExportWithoutChecksum: () => void;
  /** Checksum déjà bon : export direct, le nom de fichier porte _ChecksumOK */
  onExportChecksumOk: () => void;
  onExportWithChecksum: () => void;
  isClosing?: boolean;
  isCalculating?: boolean;
  calculationComplete?: boolean;
  /** État du checksum des données à exporter, vérifié à l'ouverture :
   *  ok → export direct, bad → proposer la correction avant l'export. */
  status?: ExportChecksumStatus;
}

export function ChecksumModal({
  onClose,
  onExportWithoutChecksum,
  onExportChecksumOk,
  onExportWithChecksum,
  isClosing = false,
  isCalculating = false,
  calculationComplete = false,
  status = "checking",
}: ChecksumModalProps) {
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
        {!isCalculating && !calculationComplete && status !== "checking" && (
          <button
            onClick={onClose}
            className={`absolute top-4 right-4 z-10 p-2 rounded-lg transition-colors ${L ? "hover:bg-black/5" : "hover:bg-white/5"}`}
            style={{ color: L ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.6)' }}
          >
            <X className="w-5 h-5" />
          </button>
        )}

        {/* Modal Content - Same style as ProjectInfoEditModal */}
        <div className="border rounded-lg p-8" style={L ? MODAL_GLASS_LIGHT : MODAL_GLASS}>
          {/* Header with icon — état vérifié à l'ouverture : spinner pendant
              la vérification, vert si le checksum est déjà bon, alerte s'il
              doit être recalculé (puis les phases correction/terminé). */}
          <div className="flex flex-col items-center mb-6">
            <div
              className="w-20 h-20 rounded-full flex items-center justify-center mb-6"
              style={{
                backgroundColor: calculationComplete || (!isCalculating && status === "ok")
                  ? 'rgba(34, 197, 94, 0.15)'
                  : !isCalculating && status === "bad"
                    ? 'rgba(245, 158, 11, 0.15)'
                    : 'rgba(59, 130, 246, 0.15)'
              }}
            >
              {calculationComplete || (!isCalculating && status === "ok") ? (
                <Check className="w-10 h-10" style={{ color: '#22c55e' }} />
              ) : !isCalculating && status === "bad" ? (
                <AlertTriangle className="w-10 h-10" style={{ color: '#f59e0b' }} />
              ) : (
                <RefreshCw
                  className={`w-10 h-10 ${isCalculating || status === "checking" ? 'animate-spin' : ''}`}
                  style={{ color: '#3b82f6' }}
                />
              )}
            </div>

            {/* Title */}
            <h2 className={`text-xl font-semibold text-center ${L ? "text-slate-900" : "text-white"}`}>
              {isCalculating
                ? t.checksum.calculating
                : calculationComplete
                  ? t.checksum.complete
                  : status === "checking"
                    ? t.checksum.verifying
                    : status === "ok"
                      ? t.checksum.exportStatusOk
                      : t.checksum.exportStatusBad}
            </h2>
          </div>

          {/* Action buttons — selon l'état du checksum */}
          {!isCalculating && !calculationComplete && status === "ok" && (
            <div className="flex">
              <Button
                onClick={onExportChecksumOk}
                className="flex-1 bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400 text-white"
              >
                {t.checksum.exportNow}
              </Button>
            </div>
          )}
          {!isCalculating && !calculationComplete && status === "bad" && (
            <div className="flex gap-3">
              <Button
                onClick={onExportWithoutChecksum}
                variant="outline"
                className="flex-1"
              >
                {t.checksum.exportWithoutCorrection}
              </Button>
              <Button
                onClick={onExportWithChecksum}
                className="flex-1 bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400 text-white"
              >
                {t.checksum.correctBeforeExport}
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
