"use client";

import { AlertTriangle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { MODAL_GLASS, MODAL_GLASS_LIGHT } from "@/lib/modal-glass";
import { useThemeOptional } from "@/contexts/theme-context";

interface ConfirmModalProps {
  title: string;
  description: string;
  /** Primary (confirm) button label. */
  confirmLabel: string;
  /** Secondary (cancel) button label. */
  cancelLabel: string;
  /** Optional middle button (e.g. "Discard") between Cancel and Confirm. */
  middleLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
  onMiddle?: () => void;
  /** Disables the confirm button and swaps its label for `loadingLabel`. */
  isLoading?: boolean;
  loadingLabel?: string;
  isClosing?: boolean;
}

/**
 * Confirmation dialog matching the editor Settings window theme:
 * backdrop #000000a2 + blur, container `border rounded-lg` on the shared
 * MODAL_GLASS surface,
 * white title, slate description, outline secondary buttons + a red→orange
 * gradient primary button. Same expand/collapse animations as the other
 * editor modals.
 */
export function ConfirmModal({
  title,
  description,
  confirmLabel,
  cancelLabel,
  middleLabel,
  onConfirm,
  onCancel,
  onMiddle,
  isLoading = false,
  loadingLabel,
  isClosing = false,
}: ConfirmModalProps) {
  // Suit le thème de l'écran hôte (dashboard ou éditeur)
  const themeCtx = useThemeOptional();
  const L = (themeCtx?.theme ?? "default") === "light";

  return (
    <div
      className="fixed inset-0 z-[80] flex items-center justify-center backdrop-blur-sm"
      style={{
        backgroundColor: "#000000a2",
        animation: isClosing
          ? "backdropFadeOut 0.2s ease-out forwards"
          : "backdropFadeIn 0.2s ease-out forwards",
      }}
      onClick={onCancel}
    >
      <div
        className="relative w-full max-w-md mx-4"
        style={{
          animation: isClosing
            ? "modalCollapse 0.2s ease-out forwards"
            : "modalExpand 0.2s ease-out forwards",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="border rounded-lg p-6" style={L ? MODAL_GLASS_LIGHT : MODAL_GLASS}>
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 rounded-full bg-orange-500/20">
              <AlertTriangle className="w-6 h-6 text-orange-500" />
            </div>
            <h3 className={`text-lg font-semibold ${L ? "text-slate-900" : "text-white"}`}>{title}</h3>
          </div>
          <p className={`mb-6 ${L ? "text-slate-600" : "text-slate-400"}`}>{description}</p>
          <div className="flex gap-3 justify-end">
            <Button variant="outline" size="sm" className="h-9 px-4" onClick={onCancel}>
              {cancelLabel}
            </Button>
            {middleLabel && onMiddle && (
              <Button variant="outline" size="sm" className="h-9 px-4" onClick={onMiddle}>
                {middleLabel}
              </Button>
            )}
            <Button
              size="sm"
              className="h-9 px-6 text-white bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400"
              onClick={onConfirm}
              disabled={isLoading}
            >
              {isLoading ? loadingLabel || confirmLabel : confirmLabel}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
