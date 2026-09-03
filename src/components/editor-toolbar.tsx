"use client";

import { useState, useEffect, useRef } from "react";
import { Button } from "@/components/ui/button";
import {
  LogOut,
  Settings as SettingsIcon,
  Zap,
  ChevronDown,
  Check,
  Plus,
  Minus,
} from "lucide-react";
import { useRouter } from "next/navigation";
import { useTheme } from "@/contexts/theme-context";
import { useI18n } from "@/contexts/i18n-context";
import { WindowControls } from "@/components/window-controls";

interface EditorToolbarProps {
  projectName: string;
  hexdumpSize?: "8b" | "16b";
  hexdumpFormat?: "hex" | "dec";
  easyViewMode?: boolean;
  previewOpen?: boolean;
  onHexdumpSizeChange?: (size: "8b" | "16b") => void;
  // Ordre des octets en 16 bits : HiLo (big-endian, EDC16/MJD) ou LoHi (EDC15)
  hexdumpByteOrder?: "hilo" | "lohi";
  onHexdumpByteOrderChange?: (order: "hilo" | "lohi") => void;
  onHexdumpFormatChange?: (format: "hex" | "dec") => void;
  onEasyViewModeChange?: (enabled: boolean) => void;
  onPreviewClick?: () => void;
  onSettingsClick?: () => void;
  // Zoom de l'éditeur (webview) — affiché à gauche du bouton paramètres
  zoomPercent?: number;
  onZoomIn?: () => void;
  onZoomOut?: () => void;
  onCloseProject?: () => void;
  // Modify map values
  hasActiveMap?: boolean;
  onModifyApply?: (operation: 'add' | 'fill', value: number) => void;
  // Controlled modifyValue for keyboard shortcuts in MapViewer
  modifyValue?: string;
  onModifyValueChange?: (value: string) => void;
  // Compare versions
  onCompareClick?: () => void;
}

export function EditorToolbar({
  projectName,
  hexdumpSize = "8b",
  hexdumpFormat = "hex",
  easyViewMode = false,
  previewOpen = false,
  onHexdumpSizeChange,
  hexdumpByteOrder = "lohi",
  onHexdumpByteOrderChange,
  onHexdumpFormatChange,
  onEasyViewModeChange,
  onPreviewClick,
  onSettingsClick,
  zoomPercent = 100,
  onZoomIn,
  onZoomOut,
  onCloseProject,
  hasActiveMap = false,
  onModifyApply,
  modifyValue: controlledModifyValue,
  onModifyValueChange,
  onCompareClick,
}: EditorToolbarProps) {
  const router = useRouter();
  const { theme } = useTheme();
  const { t } = useI18n();

  // State for modify controls - use controlled value if provided
  const [modifyOperation, setModifyOperation] = useState<'add' | 'fill'>('add');
  const [internalModifyValue, setInternalModifyValue] = useState<string>('1');

  // Custom dropdown for the operation selector (styled like the app's
  // context menus instead of the native <select>). The panel is rendered
  // position:fixed (like the context menus) because the toolbar container
  // has overflow-hidden and the workspace creates its own stacking context —
  // an absolute panel would be clipped behind the editor background.
  const [operationMenuPos, setOperationMenuPos] = useState<{ x: number; y: number } | null>(null);
  const isOperationMenuOpen = operationMenuPos !== null;
  const operationMenuRef = useRef<HTMLDivElement>(null);

  const toggleOperationMenu = (e: React.MouseEvent<HTMLButtonElement>) => {
    if (operationMenuPos) {
      setOperationMenuPos(null);
      return;
    }
    const rect = e.currentTarget.getBoundingClientRect();
    setOperationMenuPos({ x: rect.left, y: rect.bottom + 4 });
  };

  useEffect(() => {
    if (!isOperationMenuOpen) return;
    const handleOutside = (e: MouseEvent) => {
      if (operationMenuRef.current && !operationMenuRef.current.contains(e.target as Node)) {
        setOperationMenuPos(null);
      }
    };
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOperationMenuPos(null);
    };
    const handleScrollOrResize = () => setOperationMenuPos(null);
    document.addEventListener('mousedown', handleOutside);
    document.addEventListener('keydown', handleEscape);
    window.addEventListener('resize', handleScrollOrResize);
    window.addEventListener('scroll', handleScrollOrResize, true);
    return () => {
      document.removeEventListener('mousedown', handleOutside);
      document.removeEventListener('keydown', handleEscape);
      window.removeEventListener('resize', handleScrollOrResize);
      window.removeEventListener('scroll', handleScrollOrResize, true);
    };
  }, [isOperationMenuOpen]);

  // Use controlled value if provided, otherwise use internal state
  const modifyValue = controlledModifyValue ?? internalModifyValue;
  const setModifyValue = (value: string) => {
    if (onModifyValueChange) {
      onModifyValueChange(value);
    } else {
      setInternalModifyValue(value);
    }
  };

  // Glassmorphism tokens — translucent toolbar; OLED stays near-black opaque
  const getToolbarBg = () => {
    switch (theme) {
      case 'light':
        return 'rgba(255,255,255,0.6)';
      case 'oled':
        return 'rgba(0,0,0,0.8)';
      default:
        return 'rgba(10,11,15,0.55)';
    }
  };

  const getBorderColor = () => {
    switch (theme) {
      case 'light':
        return 'rgba(15,20,35,0.1)';
      case 'oled':
        return 'rgba(255,255,255,0.06)';
      default:
        return 'rgba(255,255,255,0.08)';
    }
  };

  const getButtonBg = () => {
    switch (theme) {
      case 'light':
        return 'rgba(255,255,255,0.6)';
      case 'oled':
        return 'rgba(20,20,23,0.85)';
      default:
        return 'rgba(22,25,34,0.55)';
    }
  };

  const getTextColor = () => {
    return theme === 'light' ? '#000000' : 'rgba(255, 255, 255, 0.7)';
  };

  const getButtonHoverClass = () => {
    return theme === 'light' ? 'hover:bg-black/10' : 'hover:bg-white/10';
  };

  const handleCloseProject = () => {
    if (onCloseProject) {
      // Use callback if provided (for unsaved changes check)
      onCloseProject();
    } else {
      // Fallback: Clear session storage and return to dashboard
      sessionStorage.removeItem("currentProject");
      router.push("/dashboard");
    }
  };

  return (
    <div
      data-tauri-drag-region
      className="h-12 flex items-center gap-0.5 sm:gap-1 px-1 sm:px-2 lg:px-4 flex-shrink-0 w-full overflow-hidden"
      style={{
        // NOTE: no backdrop-filter here — it would turn the toolbar into the
        // containing block for the position:fixed Add/Fill dropdown (viewport
        // coords would break). Nothing scrolls behind the toolbar anyway; the
        // glass look comes from the translucent bg + ambient halos behind it.
        background: getToolbarBg(),
        borderBottom: `1px solid ${getBorderColor()}`
      }}
    >
        {/* Groupe unifié : 8b / 16b / Hex / Dec */}
        <div className="flex items-center rounded-lg px-0.5 sm:px-1 flex-shrink-0" style={{ background: getButtonBg(), border: `1px solid ${getBorderColor()}` }}>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onHexdumpSizeChange?.("8b")}
            className={`h-7 px-1.5 sm:px-2 lg:px-3 text-xs ${
              hexdumpSize === "8b"
                ? "bg-blue-600/40 text-white-500 hover:bg-blue-400/40"
                : getButtonHoverClass()
            }`}
            style={{ color: hexdumpSize === "8b" ? (theme === 'light' ? '#000000' : undefined) : getTextColor() }}
          >
            8b
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onHexdumpSizeChange?.("16b")}
            className={`h-7 px-1.5 sm:px-2 lg:px-3 text-xs ${
              hexdumpSize === "16b"
                ? "bg-blue-600/40 text-white-500 hover:bg-blue-400/40"
                : getButtonHoverClass()
            }`}
            style={{ color: hexdumpSize === "16b" ? (theme === 'light' ? '#000000' : undefined) : getTextColor() }}
          >
            16b
          </Button>
          {/* Ordre des octets de la vue 16 bits : HiLo ↔ LoHi (toujours
              visible dans la pastille, n'agit que sur l'affichage 16b) */}
          {(
            <Button
              variant="ghost"
              size="sm"
              onClick={() => onHexdumpByteOrderChange?.(hexdumpByteOrder === "hilo" ? "lohi" : "hilo")}
              title={hexdumpByteOrder === "hilo" ? "High byte first (big-endian) — click for LoHi" : "Low byte first (little-endian) — click for HiLo"}
              className={`h-7 px-1.5 sm:px-2 lg:px-3 text-xs ${getButtonHoverClass()}`}
              style={{ color: getTextColor() }}
            >
              {hexdumpByteOrder === "hilo" ? "HiLo" : "LoHi"}
            </Button>
          )}
          {/* Séparateur */}
          <div className="w-px h-5 mx-0.5 sm:mx-1" style={{ background: theme === 'light' ? '#dee2e6' : 'rgba(255, 255, 255, 0.1)' }}></div>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onHexdumpFormatChange?.("hex")}
            className={`h-7 px-1.5 sm:px-2 lg:px-3 text-xs ${
              hexdumpFormat === "hex"
                ? "bg-red-600/40 text-white-500 hover:bg-red-400/40"
                : getButtonHoverClass()
            }`}
            style={{ color: hexdumpFormat === "hex" ? (theme === 'light' ? '#000000' : undefined) : getTextColor() }}
          >
            Hex
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onHexdumpFormatChange?.("dec")}
            className={`h-7 px-1.5 sm:px-2 lg:px-3 text-xs ${
              hexdumpFormat === "dec"
                ? "bg-red-600/40 text-white-500 hover:bg-red-400/40"
                : getButtonHoverClass()
            }`}
            style={{ color: hexdumpFormat === "dec" ? (theme === 'light' ? '#000000' : undefined) : getTextColor() }}
          >
            Dec
          </Button>
        </div>

        {/* Compare button - hover orange */}
        <div className="flex items-center rounded-lg px-1 ml-0.5 sm:ml-1 flex-shrink-0" style={{ background: getButtonBg(), border: `1px solid ${getBorderColor()}` }}>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onCompareClick?.()}
            className="h-7 px-2 sm:px-3 text-xs hover:bg-orange-600/40 hover:text-black-400 transition-colors duration-200"
            style={{ color: getTextColor() }}
            title={t.toolbar.compare}
          >
            {t.toolbar.compare}
          </Button>
        </div>

        {/* Ecart souple : large quand la fenetre l'est (position
            d'origine des pastilles), reduit a l'ecart commun sinon */}
        <div data-tauri-drag-region className="flex-1 min-w-[12px] max-w-[6rem]" />
        {/* EasyView button */}
        <div className="flex items-center rounded-lg px-1 flex-shrink-0" style={{ background: getButtonBg(), border: `1px solid ${getBorderColor()}` }}>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onEasyViewModeChange?.(!easyViewMode)}
            className={`h-7 px-2 sm:px-3 text-xs transition-colors duration-200 ${
              easyViewMode
                ? "bg-green-600/40 text-white-500 hover:bg-green-400/40"
                : "hover:bg-green-600/40 hover:text-white-400"
            }`}
            style={{ color: easyViewMode ? (theme === 'light' ? '#000000' : undefined) : getTextColor() }}
            title={t.toolbar.easyView}
          >
            {t.toolbar.easyView}
          </Button>
        </div>

        {/* Preview button */}
        <div className="flex items-center rounded-lg px-1 ml-0.5 sm:ml-1 flex-shrink-0" style={{ background: getButtonBg(), border: `1px solid ${getBorderColor()}` }}>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onPreviewClick?.()}
            className={`h-7 px-2 sm:px-3 text-xs transition-colors duration-200 ${
              previewOpen
                ? "bg-purple-600/40 text-white-500 hover:bg-purple-400/40"
                : "hover:bg-purple-600/40 hover:text-white-400"
            }`}
            style={{ color: previewOpen ? (theme === 'light' ? '#000000' : undefined) : getTextColor() }}
            title={t.toolbar.preview}
          >
            {t.toolbar.preview}
          </Button>
        </div>

        {/* Ecart souple : large quand la fenetre l'est (position
            d'origine des pastilles), reduit a l'ecart commun sinon */}
        <div data-tauri-drag-region className="flex-1 min-w-[12px] max-w-[6rem]" />
        {/* Modify controls */}
        <div className="flex items-center rounded-lg px-1 flex-shrink-0" style={{ background: getButtonBg(), border: `1px solid ${getBorderColor()}` }}>
          <div className="relative" ref={operationMenuRef}>
            <button
              type="button"
              onClick={toggleOperationMenu}
              className={`h-7 pl-2 pr-1.5 text-xs rounded-l border-0 outline-none cursor-pointer flex items-center gap-1 transition-colors ${getButtonHoverClass()}`}
              style={{ background: 'transparent', color: getTextColor() }}
              title={modifyOperation === 'add' ? t.toolbar.add : t.toolbar.fill}
            >
              {modifyOperation === 'add' ? t.toolbar.add : t.toolbar.fill}
              <ChevronDown className={`w-3 h-3 transition-transform duration-150 ${isOperationMenuOpen ? 'rotate-180' : ''}`} />
            </button>

            {operationMenuPos && (
              <div
                className="rounded-lg shadow-lg p-1.5 flex flex-col gap-0.5 text-[12px] min-w-[110px]"
                style={{
                  position: 'fixed',
                  left: operationMenuPos.x,
                  top: operationMenuPos.y,
                  zIndex: 9999,
                  background: theme === 'light' ? 'rgba(255, 255, 255, 0.98)' : 'rgba(0, 0, 0, 0.9)',
                  border: theme === 'light' ? '1px solid rgba(0, 0, 0, 0.15)' : '1px solid rgba(128, 128, 128, 0.5)',
                  color: theme === 'light' ? '#000000' : '#ffffff',
                }}
              >
                {(['add', 'fill'] as const).map((op) => (
                  <button
                    key={op}
                    type="button"
                    className={`px-3 py-1.5 text-left rounded transition-colors flex items-center justify-between gap-2 ${
                      theme === 'light' ? 'hover:bg-black/10' : 'hover:bg-white/10'
                    } ${modifyOperation === op ? (theme === 'light' ? 'bg-black/5' : 'bg-white/5') : ''}`}
                    onClick={() => {
                      setModifyOperation(op);
                      setOperationMenuPos(null);
                    }}
                  >
                    {op === 'add' ? t.toolbar.add : t.toolbar.fill}
                    {modifyOperation === op && <Check className="w-3 h-3" />}
                  </button>
                ))}
              </div>
            )}
          </div>
          <input
            type="text"
            value={modifyValue}
            onChange={(e) => setModifyValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && modifyValue.trim() && hasActiveMap) {
                const parsed = Number(modifyValue.replace(",", "."));
                if (!Number.isNaN(parsed)) {
                  onModifyApply?.(modifyOperation, parsed);
                }
              }
            }}
            placeholder="1"
            className="h-7 w-8 sm:w-10 lg:w-12 px-0.5 sm:px-1 text-xs border-0 outline-none text-center rounded"
            style={{
              background: theme === 'light' ? 'rgba(0, 0, 0, 0.03)' : 'rgba(255, 255, 255, 0.03)',
              color: getTextColor(),
            }}
            title={modifyOperation === 'add' ? "Value to add (negative to subtract)" : "Value to fill all cells"}
          />
          <Button
            variant="ghost"
            size="sm"
            disabled={!hasActiveMap || !modifyValue.trim()}
            onClick={() => {
              const parsed = Number(modifyValue.replace(",", "."));
              if (!Number.isNaN(parsed)) {
                onModifyApply?.(modifyOperation, parsed);
              }
            }}
            className={`h-7 w-7 p-0 transition-colors duration-200 ${
              hasActiveMap && modifyValue.trim()
                ? getButtonHoverClass()
                : "opacity-50 cursor-not-allowed"
            }`}
            style={{ color: getTextColor() }}
            title="Apply modification to active map"
          >
            <Zap className="w-4 h-4" fill="currentColor" />
          </Button>
        </div>
      {/* Ecart souple sans plafond : absorbe toute la place restante, ce
          qui garde les commandes a droite en grand ecran ; il se resorbe
          jusqu'a 12px comme les autres quand la fenetre retrecit. */}
      <div data-tauri-drag-region className="flex-1 min-w-[12px]" />

        {/* Zoom de l'éditeur : pastille [-] valeur [+] comme les autres
            groupes de la topbar */}
        <div
          className="flex items-center rounded-lg px-1 mr-0.5 sm:mr-1 flex-shrink-0"
          style={{ background: getButtonBg(), border: `1px solid ${getBorderColor()}` }}
        >
          <Button
            variant="ghost"
            size="sm"
            className={`h-7 w-7 p-0 ${getButtonHoverClass()}`}
            onClick={onZoomOut}
            title="Zoom −"
            style={{ color: getTextColor() }}
          >
            <Minus className="w-3.5 h-3.5" />
          </Button>
          <span
            className="text-xs tabular-nums w-9 text-center select-none"
            style={{ color: getTextColor() }}
          >
            {zoomPercent}%
          </span>
          <Button
            variant="ghost"
            size="sm"
            className={`h-7 w-7 p-0 ${getButtonHoverClass()}`}
            onClick={onZoomIn}
            title="Zoom +"
            style={{ color: getTextColor() }}
          >
            <Plus className="w-3.5 h-3.5" />
          </Button>
        </div>
        <Button
          variant="ghost"
          size="sm"
          className={`h-8 w-8 p-0 ${getButtonHoverClass()}`}
          onClick={onSettingsClick}
          title={t.toolbar.settings}
          style={{ color: getTextColor() }}
        >
          <SettingsIcon className="w-4 h-4" />
        </Button>

        {/* Quitter le projet — icône porte (PAS une croix : la croix est
            réservée à la fermeture de la fenêtre, juste à droite) */}
        <Button
          variant="ghost"
          size="sm"
          className="h-8 w-8 p-0 hover:bg-red-500/20"
          onClick={handleCloseProject}
          title={t.toolbar.closeProject}
          style={{ color: getTextColor() }}
        >
          <LogOut className="w-4 h-4" />
        </Button>

        {/* Contrôles fenêtre groupés dans une pastille — même langage visuel
            que le groupe 8b/16b/Hex/Dec à gauche de la toolbar */}
        <div
          className="flex items-center rounded-lg px-0.5 sm:px-1 ml-0.5 sm:ml-1"
          style={{ background: getButtonBg(), border: `1px solid ${getBorderColor()}` }}
        >
          <WindowControls />
        </div>
    </div>
  );
}

