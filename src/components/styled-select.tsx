"use client";

import { useState, useRef, useEffect } from "react";
import { createPortal } from "react-dom";
import { ChevronDown, Check } from "lucide-react";
import { useThemeOptional } from "@/contexts/theme-context";

export interface StyledSelectOption {
  value: string;
  label: string;
}

interface StyledSelectProps {
  value: string;
  options: StyledSelectOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
  id?: string;
  className?: string;
  /** Minimum panel width in px (defaults to trigger width). */
  minWidth?: number;
  /**
   * "auto" (default): follow the app theme. "dark": always dark styling —
   * for selects living inside always-dark surfaces (e.g. the Settings modal,
   * which keeps its dark glass look even when the app theme is light).
   */
  appearance?: "auto" | "dark";
}

/**
 * Dropdown styled exactly like the Add/Fill operation menu in the editor
 * toolbar: a trigger showing the current label + a rotating ChevronDown, and
 * a fixed-positioned panel (dark rgba(0,0,0,0.9) / light rgba(255,255,255,0.98))
 * with hoverable items and a Check on the active one. Closes on outside click,
 * Escape, scroll, and resize.
 */
export function StyledSelect({
  value,
  options,
  onChange,
  disabled = false,
  id,
  className = "",
  minWidth,
  appearance = "auto",
}: StyledSelectProps) {
  // Safe outside a ThemeProvider (e.g. the dark-only dashboard) — falls back
  // to the default dark theme.
  const theme = useThemeOptional()?.theme ?? "default";
  // Light styling only when the app theme is light AND the select is not
  // forced dark (always-dark host surfaces like the Settings modal).
  const isLight = appearance !== "dark" && theme === "light";
  const [menuPos, setMenuPos] = useState<{
    x: number;
    y: number;
    w: number;
    /** Panel max height, clamped to the available viewport space. */
    maxH: number;
    /** Set when the panel opens upward: distance from the viewport bottom. */
    bottomPx?: number;
  } | null>(null);
  const isOpen = menuPos !== null;
  const menuRef = useRef<HTMLDivElement>(null);
  // The panel is portaled to document.body (so position:fixed coordinates stay
  // viewport-relative even when an ancestor has a transform/backdrop-filter,
  // e.g. the settings modal's expand animation) — it needs its own ref for the
  // outside-click check.
  const panelRef = useRef<HTMLDivElement>(null);

  const selected = options.find((o) => o.value === value);

  const toggleMenu = (e: React.MouseEvent<HTMLButtonElement>) => {
    if (disabled) return;
    if (menuPos) {
      setMenuPos(null);
      return;
    }
    const rect = e.currentTarget.getBoundingClientRect();
    // Long option lists (e.g. years) get an internal scroll, clamped to the
    // viewport; open upward when there is clearly more room above.
    const below = window.innerHeight - rect.bottom - 12;
    const above = rect.top - 12;
    if (below >= 160 || below >= above) {
      setMenuPos({ x: rect.left, y: rect.bottom + 4, w: rect.width, maxH: Math.min(320, below) });
    } else {
      setMenuPos({
        x: rect.left,
        y: rect.bottom + 4,
        w: rect.width,
        maxH: Math.min(320, above),
        bottomPx: window.innerHeight - rect.top + 4,
      });
    }
  };

  useEffect(() => {
    if (!isOpen) return;
    const handleOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        menuRef.current && !menuRef.current.contains(target) &&
        panelRef.current && !panelRef.current.contains(target)
      ) {
        setMenuPos(null);
      }
    };
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") setMenuPos(null);
    };
    const handleScrollOrResize = (e?: Event) => {
      // Ignore scrolls happening inside the panel itself (long option lists)
      if (e && panelRef.current && e.target instanceof Node && panelRef.current.contains(e.target)) return;
      setMenuPos(null);
    };
    document.addEventListener("mousedown", handleOutside);
    document.addEventListener("keydown", handleEscape);
    window.addEventListener("resize", handleScrollOrResize);
    window.addEventListener("scroll", handleScrollOrResize, true);
    return () => {
      document.removeEventListener("mousedown", handleOutside);
      document.removeEventListener("keydown", handleEscape);
      window.removeEventListener("resize", handleScrollOrResize);
      window.removeEventListener("scroll", handleScrollOrResize, true);
    };
  }, [isOpen]);

  return (
    <div className="relative" ref={menuRef}>
      <button
        type="button"
        id={id}
        onClick={toggleMenu}
        disabled={disabled}
        className={`px-3 py-2 rounded-lg text-sm flex items-center justify-between gap-2 outline-none transition-colors ${
          disabled ? "opacity-50 cursor-not-allowed" : "cursor-pointer"
        } ${className}`}
        style={{
          background: isLight ? "rgba(0, 0, 0, 0.05)" : "rgba(0, 0, 0, 0.15)",
          border: isLight ? "1px solid rgba(0, 0, 0, 0.15)" : "1px solid rgba(255, 255, 255, 0.2)",
          color: isLight ? "#000000" : "#ffffff",
        }}
      >
        <span className="truncate">{selected ? selected.label : ""}</span>
        <ChevronDown
          className={`w-3 h-3 flex-shrink-0 transition-transform duration-150 ${isOpen ? "rotate-180" : ""}`}
        />
      </button>

      {menuPos && createPortal(
        <div
          ref={panelRef}
          className="rounded-lg shadow-lg p-1.5 flex flex-col gap-0.5 text-[12px] overflow-y-auto upload-scroll"
          style={{
            position: "fixed",
            left: menuPos.x,
            top: menuPos.bottomPx == null ? menuPos.y : undefined,
            bottom: menuPos.bottomPx,
            maxHeight: menuPos.maxH,
            minWidth: Math.max(menuPos.w, minWidth ?? 0),
            zIndex: 9999,
            background: isLight ? "rgba(255, 255, 255, 0.98)" : "rgba(0, 0, 0, 0.9)",
            border: isLight ? "1px solid rgba(0, 0, 0, 0.15)" : "1px solid rgba(128, 128, 128, 0.5)",
            color: isLight ? "#000000" : "#ffffff",
          }}
        >
          {options.map((opt) => (
            <button
              key={opt.value}
              type="button"
              className={`px-3 py-1.5 text-left rounded transition-colors flex items-center justify-between gap-2 ${
                isLight ? "hover:bg-black/10" : "hover:bg-white/10"
              } ${
                value === opt.value ? (isLight ? "bg-black/5" : "bg-white/5") : ""
              }`}
              onClick={() => {
                onChange(opt.value);
                setMenuPos(null);
              }}
            >
              {opt.label}
              {value === opt.value && <Check className="w-3 h-3 flex-shrink-0" />}
            </button>
          ))}
        </div>,
        document.body
      )}
    </div>
  );
}
