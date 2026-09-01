"use client";

import { useState, useRef, useEffect } from "react";
import { Pencil } from "lucide-react";
import { Button } from "@/components/ui/button";
import { MODAL_GLASS, MODAL_GLASS_LIGHT } from "@/lib/modal-glass";
import { useThemeOptional } from "@/contexts/theme-context";

interface PromptModalProps {
  title: string;
  /** Optional helper text under the title. */
  description?: string;
  /** Initial input value. */
  initialValue?: string;
  confirmLabel: string;
  cancelLabel: string;
  onConfirm: (value: string) => void;
  onCancel: () => void;
  /** HTML input type (e.g. "text"). Defaults to "text". */
  inputType?: string;
  placeholder?: string;
  isClosing?: boolean;
}

/**
 * Single-input dialog matching the editor Settings window theme:
 * backdrop #000000a2 + blur, container `border rounded-lg` on the shared
 * MODAL_GLASS surface,
 * white title, slate description, outline cancel + red→orange gradient
 * confirm. Auto-focuses/selects the input; Enter confirms, Escape cancels.
 */
export function PromptModal({
  title,
  description,
  initialValue = "",
  confirmLabel,
  cancelLabel,
  onConfirm,
  onCancel,
  inputType = "text",
  placeholder,
  isClosing = false,
}: PromptModalProps) {
  const [value, setValue] = useState(initialValue);
  // Suit le thème de l'écran hôte (dashboard ou éditeur)
  const themeCtx = useThemeOptional();
  const L = (themeCtx?.theme ?? "default") === "light";

  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // Focus and select the field on open
    const id = requestAnimationFrame(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    });
    return () => cancelAnimationFrame(id);
  }, []);

  const confirm = () => onConfirm(value);

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
              <Pencil className="w-5 h-5 text-orange-500" />
            </div>
            <h3 className={`text-lg font-semibold ${L ? "text-slate-900" : "text-white"}`}>{title}</h3>
          </div>
          {description && <p className={`mb-4 text-sm ${L ? "text-slate-600" : "text-slate-400"}`}>{description}</p>}
          <input
            ref={inputRef}
            type={inputType}
            value={value}
            placeholder={placeholder}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                confirm();
              } else if (e.key === "Escape") {
                e.preventDefault();
                onCancel();
              }
            }}
            className={`w-full px-3 py-2 mb-6 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-orange-500/50 ${L ? "text-slate-900 bg-black/[0.05] border border-black/20" : "text-white bg-black/15 border border-white/20"}`}
          />
          <div className="flex gap-3 justify-end">
            <Button variant="outline" size="sm" className="h-9 px-4" onClick={onCancel}>
              {cancelLabel}
            </Button>
            <Button
              size="sm"
              className="h-9 px-6 text-white bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400"
              onClick={confirm}
            >
              {confirmLabel}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
