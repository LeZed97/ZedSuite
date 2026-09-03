"use client";

import { useState, useEffect, useRef, useMemo, useCallback, memo } from "react";
import { createPortal } from "react-dom";
import { X, ChevronLeft, ChevronRight, ChevronDown, Check, ArrowLeftRight, Loader2 } from "lucide-react";
import { useI18n } from "@/contexts/i18n-context";
import { useTheme } from "@/contexts/theme-context";
import { Button } from "@/components/ui/button";
import axios from "axios";
import { isBigEndianEcu } from "@/lib/ecu-endianness";
import { getModalGlassStyle } from "@/lib/modal-glass";

interface VersionDto {
  id: string;
  fileId: string;
  name: string;
  isCurrent: boolean;
  baseVersionId?: string | null;
  createdAt: string;
}

interface Difference {
  address: number;
  value1: number;
  value2: number;
}

interface MapRegion {
  name: string;
  address: number;
  size: number;
  codeblock_id?: number | null;
  dimensions?: {
    TwoDimensional?: {
      rows: number;
      cols: number;
    };
  };
}

interface CompareModalProps {
  isOpen: boolean;
  onClose: () => void;
  versions: VersionDto[];
  fileId: string;
  originalFileData: number[];
  // État courant EN MÉMOIRE (fichier + édits non sauvegardés, cf.
  // buildEditedFileData) : exposé comme pseudo-version « État actuel » —
  // sans elle, une modification non enregistrée n'appartient à aucune
  // version et la comparaison affiche 0 différence.
  currentFileData?: number[];
  // Reconstruction FIABLE des octets d'une version (page.tsx :
  // buildVersionFileData — corrections/flips/binaires appliqués). Le fallback
  // interne reste utilisé si absent.
  resolveVersionData?: (versionId: string) => Promise<number[]>;
  hexdumpSize: "8b" | "16b";
  hexdumpFormat: "hex" | "dec";
  // Ordre des octets initial en 16 bits (suit l'hexdump de l'éditeur)
  hexdumpByteOrder?: "hilo" | "lohi";
  mapRegions?: MapRegion[];
  ecuType?: string;
}

/** Lecture d'une valeur 16 bits selon l'ordre des octets choisi. */
const read16 = (arr: number[], off: number, order: "hilo" | "lohi"): number =>
  order === "hilo" ? (((arr[off] ?? 0) << 8) | (arr[off + 1] ?? 0)) : ((arr[off] ?? 0) | ((arr[off + 1] ?? 0) << 8));

// Id réservé de la pseudo-version « État actuel (non sauvegardé) »
const CURRENT_STATE_ID = "__current_state__";

const ROW_HEIGHT = 18;
const VALUES_PER_ROW = 8; // Always 8 columns
// Virtualization: rows rendered above/below the viewport, range quantized to
// CHUNK-row steps so scrolling re-renders every CHUNK rows, not every event.
const OVERSCAN = 30;
const RANGE_CHUNK = 25;

// Map color (same as hexdump-viewer)
const MAP_COLOR = {
  bg: '#3a3a3a',
  border: '#6a6a6a',
  text: '#ffffff',
  labelBg: '#4a4a4a',
};

// Couleurs des valeurs différentes (même convention WinOLS que l'hexdump) :
// le côté le plus HAUT en rouge, le plus BAS en bleu, texte gras + fond teinté.
const DIFF_COLORS = {
  dark: { above: '#ff5252', below: '#4da3ff', aboveBg: 'rgba(255,82,82,0.14)', belowBg: 'rgba(77,163,255,0.14)' },
  light: { above: '#c62828', below: '#1565c0', aboveBg: 'rgba(198,40,40,0.12)', belowBg: 'rgba(21,101,192,0.12)' },
};

// Largeur de la colonne minimap partagée (px) — même valeur que l'hexdump
const MINIMAP_WIDTH = 32;

type ByteMapInfo = { mapRegion: MapRegion; isStart: boolean; isEnd: boolean };

// ---------------------------------------------------------------------------
// Version dropdown — same theme as the topbar operation dropdown: trigger with
// rotating chevron, fixed-position panel (rgba(0,0,0,0.9) / white in light),
// hover rows with a Check on the active item, closes on outside click /
// Escape / scroll / resize.
// ---------------------------------------------------------------------------
interface VersionSelectProps {
  label: string;
  placeholder: string;
  versions: VersionDto[];
  selectedId: string;
  disabledId: string; // version already picked on the other side
  currentBadge: string;
  theme: string;
  onSelect: (id: string) => void;
}

function VersionSelect({
  label,
  placeholder,
  versions,
  selectedId,
  disabledId,
  currentBadge,
  theme,
  onSelect,
}: VersionSelectProps) {
  const [menuPos, setMenuPos] = useState<{ x: number; y: number; width: number } | null>(null);
  const isOpen = menuPos !== null;
  const rootRef = useRef<HTMLDivElement>(null);
  // Le panneau est PORTALÉ vers document.body : il n'est plus dans le
  // sous-arbre de rootRef, la détection de clic extérieur doit donc le
  // vérifier séparément — sinon le mousedown sur un item ferme le menu avant
  // que son onClick (au mouseup) ne parte, et la sélection est perdue.
  const menuRef = useRef<HTMLDivElement>(null);

  const selected = versions.find((v) => v.id === selectedId);

  const toggleMenu = (e: React.MouseEvent<HTMLButtonElement>) => {
    if (menuPos) {
      setMenuPos(null);
      return;
    }
    const rect = e.currentTarget.getBoundingClientRect();
    setMenuPos({ x: rect.left, y: rect.bottom + 4, width: rect.width });
  };

  useEffect(() => {
    if (!isOpen) return;
    const handleOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      const inRoot = rootRef.current?.contains(target) ?? false;
      const inMenu = menuRef.current?.contains(target) ?? false;
      if (!inRoot && !inMenu) {
        setMenuPos(null);
      }
    };
    // Capture phase + stopPropagation so an open menu swallows Escape before
    // the modal's own Escape-to-close handler sees it.
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        setMenuPos(null);
      }
    };
    // Ne pas fermer quand on scrolle DANS le menu lui-même (liste longue)
    const handleScrollOrResize = (e?: Event) => {
      if (e && e.target instanceof Node && menuRef.current?.contains(e.target)) return;
      setMenuPos(null);
    };
    document.addEventListener('mousedown', handleOutside);
    document.addEventListener('keydown', handleEscape, true);
    window.addEventListener('resize', handleScrollOrResize);
    window.addEventListener('scroll', handleScrollOrResize, true);
    return () => {
      document.removeEventListener('mousedown', handleOutside);
      document.removeEventListener('keydown', handleEscape, true);
      window.removeEventListener('resize', handleScrollOrResize);
      window.removeEventListener('scroll', handleScrollOrResize, true);
    };
  }, [isOpen]);

  const formatDate = (iso: string) => {
    const d = new Date(iso);
    if (isNaN(d.getTime())) return '';
    return d.toLocaleDateString(undefined, { day: '2-digit', month: '2-digit', year: '2-digit' });
  };

  return (
    <div className="flex-1 min-w-0" ref={rootRef}>
      <label
        className="block text-[11px] mb-1"
        style={{ color: theme === "light" ? "#666666" : "#aaaaaa" }}
      >
        {label}
      </label>
      <button
        type="button"
        onClick={toggleMenu}
        className="w-full h-9 px-3 rounded text-[12px] cursor-pointer flex items-center justify-between gap-2 transition-colors"
        style={{
          background: theme === "light" ? "#f1f3f5" : theme === "oled" ? "#0a0a0c" : "rgba(255,255,255,0.06)",
          border: `1px solid ${theme === "light" ? "#dee2e6" : theme === "oled" ? "rgba(255,255,255,0.08)" : "rgba(255,255,255,0.12)"}`,
          color: selected
            ? (theme === "light" ? "#000000" : "#ffffff")
            : (theme === "light" ? "#999999" : "#777777"),
        }}
      >
        <span className="truncate">{selected?.name || placeholder}</span>
        <ChevronDown
          className={`w-3 h-3 flex-shrink-0 transition-transform duration-150 ${isOpen ? 'rotate-180' : ''}`}
          style={{ color: theme === "light" ? "#666666" : "#999999" }}
        />
      </button>

      {/* Menu PORTALÉ vers document.body : le conteneur de la modale porte un
          backdrop-filter (surface verre) qui en ferait le containing block de
          ce panneau position:fixed — les coordonnées viewport seraient alors
          décalées. Portalé, le menu reste en coordonnées viewport ET peut
          porter son propre blur sans risque. */}
      {menuPos && createPortal(
        <div
          ref={menuRef}
          className="rounded-lg shadow-xl p-1.5 flex flex-col gap-0.5 text-[12px] overflow-y-auto"
          style={{
            position: 'fixed',
            left: menuPos.x,
            top: menuPos.y,
            minWidth: Math.max(menuPos.width, 180),
            maxHeight: '240px',
            zIndex: 10000,
            background: theme === 'light'
              ? 'rgba(255, 255, 255, 0.97)'
              : theme === 'oled' ? 'rgba(10, 10, 12, 0.95)' : 'rgba(22, 25, 34, 0.92)',
            border: `1px solid ${theme === 'light' ? 'rgba(0, 0, 0, 0.12)' : 'rgba(255, 255, 255, 0.1)'}`,
            backdropFilter: theme === 'oled' ? 'blur(6px)' : 'blur(18px) saturate(140%)',
            WebkitBackdropFilter: theme === 'oled' ? 'blur(6px)' : 'blur(18px) saturate(140%)',
            color: theme === 'light' ? '#000000' : '#ffffff',
          }}
        >
          {versions.map((v) => {
            const isSelected = v.id === selectedId;
            const isDisabled = v.id === disabledId;
            return (
              <button
                key={v.id}
                type="button"
                disabled={isDisabled}
                className={`px-3 py-1.5 text-left rounded transition-colors flex items-center gap-2 ${
                  isDisabled
                    ? 'opacity-40 cursor-not-allowed'
                    : theme === 'light' ? 'hover:bg-black/10' : 'hover:bg-white/10'
                } ${isSelected ? (theme === 'light' ? 'bg-black/5' : 'bg-white/5') : ''}`}
                onClick={() => {
                  onSelect(v.id);
                  setMenuPos(null);
                }}
              >
                <span className="truncate">{v.name}</span>
                {v.isCurrent && (
                  <span
                    className="text-[9px] px-1 py-px rounded flex-shrink-0"
                    style={{ background: 'rgba(59, 130, 246, 0.2)', color: '#60a5fa' }}
                  >
                    {currentBadge}
                  </span>
                )}
                <span className="ml-auto flex items-center gap-1.5 flex-shrink-0">
                  <span className="text-[10px]" style={{ color: theme === 'light' ? '#999999' : '#777777' }}>
                    {formatDate(v.createdAt)}
                  </span>
                  {isSelected && <Check className="w-3 h-3" />}
                </span>
              </button>
            );
          })}
        </div>,
        document.body
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// One hexdump row of a compare panel. Memoized so scrolling only mounts the
// rows entering the window; row-specific props (selection, current diff,
// hovered map) are passed only when they concern this row, keeping every
// other row's props strictly identical between renders.
// ---------------------------------------------------------------------------
interface CompareRowProps {
  rowIndex: number;
  data: number[];
  // Données de l'AUTRE panneau (identité stable) : sert à colorer chaque
  // différence selon sa direction (valeur plus haute = rouge, plus basse =
  // bleu), même convention que l'hexdump / WinOLS.
  otherData: number[];
  side: "left" | "right";
  theme: string;
  hexdumpSize: "8b" | "16b";
  byteOrder: "hilo" | "lohi";
  hexdumpFormat: "hex" | "dec";
  bytesPerRow: number;
  byteToMapInfo: Map<number, ByteMapInfo>;
  diffAddresses: Set<number>;
  // -1 when not in this row
  currentDiffAddress: number;
  // -1 when not in this row
  selectedAddress: number;
  // null when the hovered map does not intersect this row
  hoveredMapAddress: number | null;
  onSelectAddress: (address: number) => void;
  onHoverMap: (address: number | null) => void;
}

const CompareRow = memo(function CompareRow({
  rowIndex,
  data,
  otherData,
  side,
  theme,
  hexdumpSize,
  byteOrder,
  hexdumpFormat,
  bytesPerRow,
  byteToMapInfo,
  diffAddresses,
  currentDiffAddress,
  selectedAddress,
  hoveredMapAddress,
  onSelectAddress,
  onHoverMap,
}: CompareRowProps) {
  const bytesPerValue = hexdumpSize === "8b" ? 1 : 2;
  const startByte = rowIndex * bytesPerRow;
  const address = startByte.toString(16).toUpperCase().padStart(5, "0");
  const asciiColor = theme === 'light' ? 'rgba(0, 0, 0, 0.7)' : 'rgba(255, 255, 255, 0.7)';

  const values: JSX.Element[] = [];
  const asciiChars: JSX.Element[] = [];

  // Check if this row contains any map starts (for label display)
  const mapStartsInRow: { mapRegion: MapRegion; byteOffset: number }[] = [];
  for (let j = 0; j < bytesPerRow; j++) {
    const byteOffset = startByte + j;
    const mapInfo = byteToMapInfo.get(byteOffset);
    if (mapInfo?.isStart) {
      mapStartsInRow.push({ mapRegion: mapInfo.mapRegion, byteOffset });
    }
  }

  // ASCII characters (bytesPerRow bytes per row) - no coloring
  for (let j = 0; j < bytesPerRow; j++) {
    const byteOffset = startByte + j;

    let char = ' ';
    if (byteOffset < data.length) {
      const byte = data[byteOffset];
      char = (byte >= 32 && byte <= 126) ? String.fromCharCode(byte) : '.';
    }

    asciiChars.push(
      <span key={`ascii-${j}`}>
        {char}
      </span>
    );
  }

  // Values display (always 8 values per row)
  for (let j = 0; j < VALUES_PER_ROW; j++) {
    const byteOffset = startByte + j * bytesPerValue;
    const mapInfo = byteToMapInfo.get(byteOffset);
    const isInMap = !!mapInfo;

    let displayValue = "";
    // Direction de la différence vs l'autre panneau : +1 = plus haut (rouge),
    // -1 = plus bas (bleu). Décodage identique à l'affichage.
    let diffSign = 0;
    if (byteOffset + bytesPerValue <= data.length) {
      let value: number;
      if (bytesPerValue === 1) {
        value = data[byteOffset];
        displayValue =
          hexdumpFormat === "hex"
            ? value.toString(16).toUpperCase().padStart(2, "0")
            : value.toString(10).padStart(3, "0");
      } else {
        value = read16(data, byteOffset, byteOrder);
        displayValue =
          hexdumpFormat === "hex"
            ? value.toString(16).toUpperCase().padStart(4, "0")
            : value.toString(10).padStart(5, "0");
      }
      const other = bytesPerValue === 1
        ? (otherData[byteOffset] ?? 0)
        : read16(otherData, byteOffset, byteOrder);
      if (value > other) diffSign = 1;
      else if (value < other) diffSign = -1;
    } else {
      displayValue =
        hexdumpSize === "8b"
          ? hexdumpFormat === "hex"
            ? "  "
            : "   "
          : hexdumpFormat === "hex"
          ? "    "
          : "     ";
    }

    const isDiff = diffAddresses.has(byteOffset);
    const isCurrentDiff = byteOffset === currentDiffAddress;
    const isSelected = selectedAddress === byteOffset;

    // Check if this cell is in a hovered map region
    const isInHoveredMap = hoveredMapAddress !== null && mapInfo && mapInfo.mapRegion.address === hoveredMapAddress;

    // Sélection > diff courante (or) > différence (rouge/bleu directionnel,
    // convention hexdump/WinOLS) > map (fond gris, hover)
    const diffColors = theme === "light" ? DIFF_COLORS.light : DIFF_COLORS.dark;
    let bgColor: string | undefined;
    if (isSelected) {
      bgColor = "#3b82f6"; // Blue for selection
    } else if (isCurrentDiff) {
      bgColor = "#ffd700";
    } else if (isDiff) {
      bgColor = diffSign >= 0 ? diffColors.aboveBg : diffColors.belowBg;
    } else if (isInMap) {
      bgColor = isInHoveredMap ? MAP_COLOR.border : MAP_COLOR.bg; // Lighter on hover
    }

    // Determine text color
    let textColor: string;
    if (isSelected) {
      textColor = "#ffffff";
    } else if (isCurrentDiff) {
      textColor = "#000000";
    } else if (isDiff) {
      textColor = diffSign >= 0 ? diffColors.above : diffColors.below;
    } else if (isInMap) {
      textColor = MAP_COLOR.text;
    } else {
      textColor = theme === "light" ? "#000000" : "#ffffff";
    }

    values.push(
      <div
        key={`val-${j}`}
        style={{
          width: hexdumpSize === "16b" ? "2.4rem" : "1.85rem",
          textAlign: "center",
          backgroundColor: bgColor,
          color: textColor,
          marginRight: "2px",
          borderRadius: isDiff || isSelected ? "2px" : undefined,
          fontWeight: isCurrentDiff || isSelected || isDiff ? "bold" : undefined,
          borderTop: isInMap && !isDiff && !isSelected ? `1px solid ${MAP_COLOR.border}` : undefined,
          borderBottom: isInMap && !isDiff && !isSelected ? `1px solid ${MAP_COLOR.border}` : undefined,
          borderLeft: mapInfo?.isStart && !isDiff && !isSelected ? `1px solid ${MAP_COLOR.border}` : undefined,
          borderRight: mapInfo?.isEnd && !isDiff && !isSelected ? `1px solid ${MAP_COLOR.border}` : undefined,
          cursor: 'pointer',
        }}
        onClick={(e) => {
          e.stopPropagation();
          onSelectAddress(byteOffset);
        }}
        onMouseEnter={() => {
          if (mapInfo) {
            onHoverMap(mapInfo.mapRegion.address);
          }
        }}
        onMouseLeave={() => {
          onHoverMap(null);
        }}
      >
        {displayValue}
      </div>
    );
  }

  // Get the first map that starts in this row (for the label)
  const firstMapStart = mapStartsInRow[0];

  return (
    <div
      className="flex gap-2 font-mono text-[11px]"
      style={{
        position: "absolute",
        top: `${rowIndex * ROW_HEIGHT}px`,
        left: 0,
        right: 0,
        height: `${ROW_HEIGHT}px`,
        lineHeight: `${ROW_HEIGHT}px`,
      }}
    >
      {/* Address */}
      <span
        className="font-semibold flex-shrink-0"
        style={{ width: "3rem", color: theme === "light" ? "#000000" : "#e1e1e1" }}
      >
        {address}
      </span>

      {/* Values with map label overlay */}
      <div className="flex-shrink-0 relative flex">
        {values}

        {/* Map label overlay */}
        {firstMapStart && (() => {
          const { mapRegion } = firstMapStart;
          // Only show a codeblock badge when the map actually belongs to a
          // codeblock (EDC15P) — EDC16 & co have codeblock_id null.
          const cbStr = typeof mapRegion.codeblock_id === 'number' ? `CB${mapRegion.codeblock_id}` : '';

          return (
            <div
              className="font-mono text-[9px] px-1 absolute flex items-center"
              style={{
                left: '0',
                top: '0',
                height: '100%',
                backgroundColor: MAP_COLOR.labelBg,
                color: MAP_COLOR.text,
                border: `1px solid ${MAP_COLOR.border}`,
                borderRadius: '2px',
                whiteSpace: 'nowrap',
                zIndex: 10,
                pointerEvents: 'none', // Allow clicks to pass through to cells below
              }}
              title={`${mapRegion.name}${cbStr ? ` [${cbStr}]` : ''}`}
            >
              {mapRegion.name} {cbStr && `[${cbStr}]`}
            </div>
          );
        })()}
      </div>

      {/* ASCII */}
      <div className="tracking-normal flex-shrink-0" style={{ color: asciiColor }}>
        {asciiChars}
      </div>
    </div>
  );
});

export function CompareModal({
  isOpen,
  onClose,
  versions,
  fileId,
  originalFileData,
  currentFileData,
  resolveVersionData,
  hexdumpSize: initialHexdumpSize,
  hexdumpFormat: initialHexdumpFormat,
  hexdumpByteOrder: initialByteOrder = "lohi",
  mapRegions = [],
  ecuType,
}: CompareModalProps) {
  const { t } = useI18n();
  const { theme } = useTheme();

  // Liste présentée dans les sélecteurs : pseudo-version « État actuel » en
  // tête (si dispo), puis les versions sauvegardées.
  const hasCurrentState = !!currentFileData && currentFileData.length > 0;
  const displayVersions = useMemo<VersionDto[]>(() => {
    if (!hasCurrentState) return versions;
    return [
      { id: CURRENT_STATE_ID, fileId, name: t.compare.currentState, isCurrent: false, createdAt: "" },
      ...versions,
    ];
  }, [hasCurrentState, versions, fileId, t.compare.currentState]);

  // Local format state (independent from parent)
  const [hexdumpSize, setHexdumpSize] = useState<"8b" | "16b">(initialHexdumpSize);
  const [hexdumpFormat, setHexdumpFormat] = useState<"hex" | "dec">(initialHexdumpFormat);
  const [byteOrder, setByteOrder] = useState<"hilo" | "lohi">(initialByteOrder);

  // Selection state
  const [selectedVersion1, setSelectedVersion1] = useState<string>("");
  const [selectedVersion2, setSelectedVersion2] = useState<string>("");
  const [isComparing, setIsComparing] = useState(false);

  // Compare view state
  const [showCompareView, setShowCompareView] = useState(false);
  const [version1Data, setVersion1Data] = useState<number[]>([]);
  const [version2Data, setVersion2Data] = useState<number[]>([]);
  const [differences, setDifferences] = useState<Difference[]>([]);
  const [currentDiffIndex, setCurrentDiffIndex] = useState(0);
  const [selectedAddress, setSelectedAddress] = useState<number | null>(null); // Address clicked by user
  const [hoveredMapAddress, setHoveredMapAddress] = useState<number | null>(null); // Address of hovered map region

  // Virtualization state
  const [visibleRange, setVisibleRange] = useState({ start: 0, end: 50 });

  // Scroll refs for synchronized scrolling
  const scrollRef1 = useRef<HTMLDivElement>(null);
  const scrollRef2 = useRef<HTMLDivElement>(null);
  const isScrolling = useRef(false);

  // Resize state
  const [modalSize, setModalSize] = useState({ width: 900, height: 500 });
  const isResizing = useRef(false);
  const resizeStart = useRef({ x: 0, y: 0, width: 0, height: 0 });

  // Dragging state
  const modalRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null);
  const dragState = useRef<{
    startX: number;
    startY: number;
    originX: number;
    originY: number;
  } | null>(null);

  // Reset state when modal opens
  useEffect(() => {
    if (isOpen) {
      setShowCompareView(false);
      setDifferences([]);
      setCurrentDiffIndex(0);
      setSelectedAddress(null);
      setPosition(null);
      setVisibleRange({ start: 0, end: 50 });
      setHexdumpSize(initialHexdumpSize);
      setHexdumpFormat(initialHexdumpFormat);
      setByteOrder(initialByteOrder);
      // Présélection utile : Ori à gauche, « État actuel » à droite quand il
      // existe — un clic sur Comparer répond au cas le plus courant (voir ses
      // modifications en cours vs l'origine).
      const ori = versions.find((v) => v.name === "Ori");
      setSelectedVersion1(ori ? ori.id : "");
      setSelectedVersion2(hasCurrentState ? CURRENT_STATE_ID : "");
    }
    // versions/hasCurrentState volontairement hors deps : ne re-réinitialise
    // qu'à l'OUVERTURE, pas si la liste se rafraîchit pendant l'utilisation.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, initialHexdumpSize, initialHexdumpFormat]);

  // Create a map of byte address -> map info for quick lookup
  const byteToMapInfo = useMemo(() => {
    const map = new Map<number, ByteMapInfo>();
    const maxLength = Math.max(version1Data.length, version2Data.length, originalFileData.length);

    mapRegions.forEach((region) => {
      const endAddress = region.address + region.size - 1;

      for (let addr = region.address; addr <= endAddress && addr < maxLength; addr++) {
        map.set(addr, {
          mapRegion: region,
          isStart: addr === region.address,
          isEnd: addr === endAddress
        });
      }
    });

    return map;
  }, [mapRegions, version1Data.length, version2Data.length, originalFileData.length]);

  // Handle scroll for virtualization — rAF-throttled and quantized to
  // RANGE_CHUNK rows so dragging the scrollbar doesn't re-render on every
  // scroll event (same fix as the hexdump viewer).
  const scrollTicking = useRef(false);
  const handleVirtualScroll = useCallback((container: HTMLDivElement | null) => {
    if (!container || scrollTicking.current) return;
    scrollTicking.current = true;
    requestAnimationFrame(() => {
      scrollTicking.current = false;
      const scrollTop = container.scrollTop;
      const containerHeight = container.clientHeight;

      const rawStart = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
      const rawEnd = Math.ceil((scrollTop + containerHeight) / ROW_HEIGHT) + OVERSCAN;
      const start = Math.floor(rawStart / RANGE_CHUNK) * RANGE_CHUNK;
      const end = Math.ceil(rawEnd / RANGE_CHUNK) * RANGE_CHUNK;

      setVisibleRange(prev => (prev.start === start && prev.end === end) ? prev : { start, end });
    });
  }, []);

  // Load version data (apply modifications to original)
  const loadVersionData = async (versionId: string): Promise<number[]> => {
    // Pseudo-version « État actuel » : octets déjà construits par le parent
    // (fichier courant + édits en mémoire, cf. buildEditedFileData)
    if (versionId === CURRENT_STATE_ID && currentFileData) {
      return [...currentFileData];
    }

    // Chemin fiable : reconstruction par le parent (corrections effectives,
    // dé-flip des coordonnées, modifs binaires, fichiers importés)
    if (resolveVersionData) {
      try {
        return await resolveVersionData(versionId);
      } catch (error) {
        console.error("resolveVersionData error, falling back:", error);
      }
    }

    const version = versions.find(v => v.id === versionId);
    if (!version) return [...originalFileData];

    // For "Ori" version, return original data
    if (version.name === "Ori") {
      return [...originalFileData];
    }

    // Load modifications for this version
    try {
      const response = await axios.get(`/api/versioning/map-edits?versionId=${versionId}`);
      const edits = response.data.edits || [];

      // Start with original data
      const data = [...originalFileData];

      // Apply modifications
      edits.forEach((edit: any) => {
        const mapAddress = edit.map_address; // API returns map_address (with underscore)

        if (mapAddress === -1 && edit.payload?.changes) {
          // Binary modifications (direct byte changes)
          edit.payload.changes.forEach((change: { address: number; newValue: number }) => {
            if (change.address >= 0 && change.address < data.length) {
              data[change.address] = change.newValue;
            }
          });
        } else if (mapAddress >= 0 && edit.payload?.changedCells) {
          // Map cell modifications - need to convert row/col to byte addresses
          const mapRegion = mapRegions.find(m => m.address === mapAddress);
          if (mapRegion) {
            // Get map dimensions
            const rows = mapRegion.dimensions?.TwoDimensional?.rows || 1;
            const cols = mapRegion.dimensions?.TwoDimensional?.cols || 1;
            const totalCells = rows * cols;
            const cellSize = totalCells > 0 ? Math.max(1, Math.floor(mapRegion.size / totalCells)) : 2;

            // Apply each cell modification
            edit.payload.changedCells.forEach((cell: { row: number; col: number; value: number }) => {
              const cellOffset = (cell.row * cols + cell.col) * cellSize;
              const cellAddress = mapAddress + cellOffset;

              // Write the value as bytes
              // Determine endianness based on ECU type
              const bigEndian = isBigEndianEcu(ecuType);

              if (cellSize === 1) {
                if (cellAddress >= 0 && cellAddress < data.length) {
                  data[cellAddress] = cell.value & 0xFF;
                }
              } else if (cellSize === 2) {
                if (cellAddress >= 0 && cellAddress + 1 < data.length) {
                  if (bigEndian) {
                    // BIG ENDIAN for EDC16/MJD6: high byte first, low byte second
                    data[cellAddress] = (cell.value >> 8) & 0xFF;
                    data[cellAddress + 1] = cell.value & 0xFF;
                  } else {
                    // LITTLE ENDIAN for EDC15 and others: low byte first, high byte second
                    data[cellAddress] = cell.value & 0xFF;
                    data[cellAddress + 1] = (cell.value >> 8) & 0xFF;
                  }
                }
              }
            });
          }
        }
      });

      return data;
    } catch (error) {
      console.error("Error loading version data:", error);
      return [...originalFileData];
    }
  };

  // Compare versions - recalculate differences when format changes
  const recalculateDifferences = useCallback((data1: number[], data2: number[], size: "8b" | "16b") => {
    const diffs: Difference[] = [];
    const bytesPerValue = size === "8b" ? 1 : 2;
    const maxLength = Math.max(data1.length, data2.length);
    const bigEndian = isBigEndianEcu(ecuType);

    for (let addr = 0; addr < maxLength; addr += bytesPerValue) {
      let value1: number, value2: number;

      if (bytesPerValue === 1) {
        value1 = data1[addr] ?? 0;
        value2 = data2[addr] ?? 0;
      } else {
        if (bigEndian) {
          // BIG ENDIAN for EDC16/MJD6: high byte first, low byte second
          value1 = ((data1[addr] ?? 0) << 8) | (data1[addr + 1] ?? 0);
          value2 = ((data2[addr] ?? 0) << 8) | (data2[addr + 1] ?? 0);
        } else {
          // LITTLE ENDIAN for EDC15 and others: low byte first, high byte second
          value1 = (data1[addr] ?? 0) | ((data1[addr + 1] ?? 0) << 8);
          value2 = (data2[addr] ?? 0) | ((data2[addr + 1] ?? 0) << 8);
        }
      }

      if (value1 !== value2) {
        diffs.push({ address: addr, value1, value2 });
      }
    }

    return diffs;
  }, [ecuType]);

  // Compare versions
  const handleCompare = async () => {
    if (!selectedVersion1 || !selectedVersion2) return;

    setIsComparing(true);
    try {
      const [data1, data2] = await Promise.all([
        loadVersionData(selectedVersion1),
        loadVersionData(selectedVersion2),
      ]);

      setVersion1Data(data1);
      setVersion2Data(data2);

      const diffs = recalculateDifferences(data1, data2, hexdumpSize);
      setDifferences(diffs);
      setCurrentDiffIndex(0);
      setShowCompareView(true);
    } catch (error) {
      console.error("Error comparing versions:", error);
    } finally {
      setIsComparing(false);
    }
  };

  // Recalculate differences when size changes
  useEffect(() => {
    if (showCompareView && version1Data.length > 0 && version2Data.length > 0) {
      const diffs = recalculateDifferences(version1Data, version2Data, hexdumpSize);
      setDifferences(diffs);
      setCurrentDiffIndex(0);
    }
  }, [hexdumpSize, showCompareView, version1Data, version2Data, recalculateDifferences]);

  // Initialize visible range when compare view opens and set up native scroll listeners
  useEffect(() => {
    if (showCompareView) {
      // Initialize visible range based on container height
      const container = scrollRef1.current;
      if (container) {
        const containerHeight = container.clientHeight || 400;
        const endRow = Math.ceil(containerHeight / ROW_HEIGHT) + OVERSCAN;
        setVisibleRange({ start: 0, end: Math.ceil(endRow / RANGE_CHUNK) * RANGE_CHUNK });
      }
    }
  }, [showCompareView]);

  // Calculate bytes per row based on size (8 values * bytesPerValue)
  const bytesPerRow = hexdumpSize === "8b" ? VALUES_PER_ROW : VALUES_PER_ROW * 2;

  // Navigate differences
  const goToDifference = useCallback((index: number) => {
    if (index < 0 || index >= differences.length) return;
    setCurrentDiffIndex(index);

    // Scroll so the difference is vertically centered
    const diff = differences[index];
    const rowIndex = Math.floor(diff.address / bytesPerRow);
    const visibleHeight = scrollRef1.current?.clientHeight ?? 200;
    const scrollTop = rowIndex * ROW_HEIGHT - visibleHeight / 2 + ROW_HEIGHT / 2;

    if (scrollRef1.current) {
      scrollRef1.current.scrollTop = Math.max(0, scrollTop);
    }
    if (scrollRef2.current) {
      scrollRef2.current.scrollTop = Math.max(0, scrollTop);
    }
  }, [differences, bytesPerRow]);

  const goToPrevDiff = useCallback(() => {
    if (differences.length === 0) return;

    // If user clicked somewhere, find previous diff from that position
    if (selectedAddress !== null) {
      const prevDiffIndex = [...differences].reverse().findIndex(d => d.address < selectedAddress);
      if (prevDiffIndex !== -1) {
        const actualIndex = differences.length - 1 - prevDiffIndex;
        goToDifference(actualIndex);
        setSelectedAddress(null); // Clear selection after navigation
        return;
      }
      // No diff before, wrap to last
      goToDifference(differences.length - 1);
      setSelectedAddress(null);
      return;
    }

    // Normal navigation
    const newIndex = currentDiffIndex > 0 ? currentDiffIndex - 1 : differences.length - 1;
    goToDifference(newIndex);
  }, [differences, selectedAddress, currentDiffIndex, goToDifference]);

  const goToNextDiff = useCallback(() => {
    if (differences.length === 0) return;

    // If user clicked somewhere, find next diff from that position
    if (selectedAddress !== null) {
      const nextDiffIndex = differences.findIndex(d => d.address > selectedAddress);
      if (nextDiffIndex !== -1) {
        goToDifference(nextDiffIndex);
        setSelectedAddress(null); // Clear selection after navigation
        return;
      }
      // No diff after, wrap to first
      goToDifference(0);
      setSelectedAddress(null);
      return;
    }

    // Normal navigation
    const newIndex = currentDiffIndex < differences.length - 1 ? currentDiffIndex + 1 : 0;
    goToDifference(newIndex);
  }, [differences, selectedAddress, currentDiffIndex, goToDifference]);

  // Keyboard: Escape closes the modal (open version menus swallow it first),
  // Left/Right arrows navigate differences in compare view.
  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      } else if (showCompareView && e.key === 'ArrowLeft') {
        e.preventDefault();
        goToPrevDiff();
      } else if (showCompareView && e.key === 'ArrowRight') {
        e.preventDefault();
        goToNextDiff();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, showCompareView, onClose, goToPrevDiff, goToNextDiff]);

  // Synchronized scrolling - update virtualization immediately during scroll
  const handleScroll = (source: "left" | "right") => {
    const sourceRef = source === "left" ? scrollRef1.current : scrollRef2.current;
    const targetRef = source === "left" ? scrollRef2.current : scrollRef1.current;

    // Always update virtualization for the scrolling container
    handleVirtualScroll(sourceRef);

    // Sync the other panel (with guard to prevent infinite loop)
    if (!isScrolling.current && sourceRef && targetRef) {
      isScrolling.current = true;
      targetRef.scrollTop = sourceRef.scrollTop;
      requestAnimationFrame(() => {
        isScrolling.current = false;
      });
    }

    // Indicateur de viewport de la minimap (style direct, déclaré plus bas —
    // appelé uniquement au scroll, donc toujours initialisé)
    updateMinimapViewport();
  };

  // Drag handlers
  const handleDragStart = (e: React.MouseEvent) => {
    e.preventDefault();
    const rect = modalRef.current?.getBoundingClientRect();
    if (!rect) return;

    const currentX = position?.x ?? rect.left;
    const currentY = position?.y ?? rect.top;

    dragState.current = {
      startX: e.clientX,
      startY: e.clientY,
      originX: currentX,
      originY: currentY,
    };

    document.addEventListener("mousemove", handleDragMove);
    document.addEventListener("mouseup", handleDragEnd);
  };

  const handleDragMove = (e: MouseEvent) => {
    if (!dragState.current) return;
    const dx = e.clientX - dragState.current.startX;
    const dy = e.clientY - dragState.current.startY;
    setPosition({
      x: dragState.current.originX + dx,
      y: dragState.current.originY + dy,
    });
  };

  const handleDragEnd = () => {
    dragState.current = null;
    document.removeEventListener("mousemove", handleDragMove);
    document.removeEventListener("mouseup", handleDragEnd);
  };

  // Resize handlers
  const handleResizeStart = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    isResizing.current = true;
    resizeStart.current = {
      x: e.clientX,
      y: e.clientY,
      width: modalSize.width,
      height: modalSize.height,
    };

    document.addEventListener("mousemove", handleResizeMove);
    document.addEventListener("mouseup", handleResizeEnd);
  };

  const handleResizeMove = (e: MouseEvent) => {
    if (!isResizing.current) return;
    const dx = e.clientX - resizeStart.current.x;
    const dy = e.clientY - resizeStart.current.y;

    setModalSize({
      width: Math.max(700, resizeStart.current.width + dx),
      height: Math.max(400, resizeStart.current.height + dy),
    });
  };

  const handleResizeEnd = () => {
    isResizing.current = false;
    document.removeEventListener("mousemove", handleResizeMove);
    document.removeEventListener("mouseup", handleResizeEnd);
  };

  // Create difference address set for highlighting - memoized
  const diffAddresses = useMemo(() => {
    const set = new Set<number>();
    const bytesPerValue = hexdumpSize === "8b" ? 1 : 2;
    differences.forEach((d) => {
      for (let i = 0; i < bytesPerValue; i++) {
        set.add(d.address + i);
      }
    });
    return set;
  }, [differences, hexdumpSize]);

  const currentDiffAddress = differences[currentDiffIndex]?.address ?? -1;

  // Calculate total rows
  const totalRows = useMemo(() => {
    const maxLength = Math.max(version1Data.length, version2Data.length);
    return Math.ceil(maxLength / bytesPerRow);
  }, [version1Data.length, version2Data.length, bytesPerRow]);

  // The full region of the hovered map, to pass the hover only to the rows it
  // actually intersects (keeps the row memo effective everywhere else)
  const hoveredMapRegion = useMemo(() => {
    if (hoveredMapAddress === null) return null;
    return mapRegions.find(m => m.address === hoveredMapAddress) || null;
  }, [hoveredMapAddress, mapRegions]);

  // Stable handlers for the memoized rows
  const handleSelectAddress = useCallback((address: number) => {
    setSelectedAddress(address);
  }, []);

  const handleHoverMap = useCallback((address: number | null) => {
    setHoveredMapAddress(address);
  }, []);

  // Theme helpers — surface verre partagée (3 thèmes : default/light/oled) +
  // séparateurs hairline, même langage que les autres modales de l'éditeur.
  const glass = getModalGlassStyle(theme as 'default' | 'light' | 'oled');
  const hairline = theme === 'light'
    ? 'rgba(0, 0, 0, 0.1)'
    : theme === 'oled' ? 'rgba(255, 255, 255, 0.07)' : 'rgba(255, 255, 255, 0.09)';
  const getTextColor = () => theme === 'light' ? '#000000' : '#ffffff';
  const getButtonBg = () => theme === 'light' ? '#f1f3f5' : theme === 'oled' ? '#101013' : 'rgba(255,255,255,0.07)';
  const getButtonHoverClass = () => theme === 'light' ? 'hover:bg-black/10' : 'hover:bg-white/10';

  // --- Minimap partagée (les deux panneaux scrollent ensemble) -------------
  const minimapRef = useRef<HTMLDivElement>(null);
  const minimapCanvasRef = useRef<HTMLCanvasElement>(null);
  const minimapViewportRef = useRef<HTMLDivElement>(null);
  const [minimapSize, setMinimapSize] = useState({ w: 0, h: 0 });

  // Position de l'indicateur de viewport — style direct, pas de re-render
  const updateMinimapViewport = useCallback(() => {
    const vp = minimapViewportRef.current;
    const mm = minimapRef.current;
    const sc = scrollRef1.current;
    if (!vp || !mm || !sc) return;
    const totalH = totalRows * ROW_HEIGHT;
    const h = mm.clientHeight;
    if (totalH <= 0 || h <= 0) return;
    const vh = Math.max(10, (sc.clientHeight / totalH) * h);
    const top = Math.min(h - vh, (sc.scrollTop / totalH) * h);
    vp.style.top = `${Math.max(0, top)}px`;
    vp.style.height = `${vh}px`;
  }, [totalRows]);

  // Mesure du conteneur minimap (suit le resize de la modale)
  useEffect(() => {
    if (!showCompareView) return;
    const el = minimapRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      const r = el.getBoundingClientRect();
      const w = Math.round(r.width);
      const h = Math.round(r.height);
      setMinimapSize(prev => (prev.w === w && prev.h === h) ? prev : { w, h });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [showCompareView]);

  // Dessin : texture WinOLS du fichier (panneau droit) + marques des
  // différences (rouge = droite plus haute, bleu = droite plus basse).
  useEffect(() => {
    if (!showCompareView) return;
    const canvas = minimapCanvasRef.current;
    const dataLen = version2Data.length || version1Data.length;
    if (!canvas || !minimapSize.w || !minimapSize.h || dataLen === 0) return;
    const dpr = window.devicePixelRatio || 1;
    const W = Math.max(1, Math.round(minimapSize.w * dpr));
    const H = Math.max(1, Math.round(minimapSize.h * dpr));
    canvas.width = W;
    canvas.height = H;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const src = version2Data.length > 0 ? version2Data : version1Data;
    const img = ctx.createImageData(W, H);
    const px = img.data;
    const totalPx = W * H;
    const light = theme === 'light';
    const bytesPerPx = dataLen / totalPx;
    for (let p = 0; p < totalPx; p++) {
      const start = Math.floor(p * bytesPerPx);
      const end = Math.min(dataLen, Math.max(start + 1, Math.floor((p + 1) * bytesPerPx)));
      const sampleStep = Math.max(1, Math.floor((end - start) / 8));
      let mn = 255, mx = 0, sum = 0, n = 0;
      for (let i = start; i < end; i += sampleStep) {
        const b = src[i] ?? 0;
        if (b < mn) mn = b;
        if (b > mx) mx = b;
        sum += b;
        n++;
      }
      const isEmpty = mn === mx && (mn === 0x00 || mn === 0xFF);
      const v = light
        ? (isEmpty ? 242 : Math.round(188 - (sum / n) * 0.22 - (mx - mn) * 0.22))
        : (isEmpty ? 15 : Math.round(82 + (sum / n) * 0.2 + (mx - mn) * 0.28));
      const o = p * 4;
      px[o] = v;
      px[o + 1] = v;
      px[o + 2] = light ? v : Math.min(255, v + 6);
      px[o + 3] = 255;
    }
    const mark = (p: number, r: number, g: number, b: number) => {
      const x = p % W;
      const y = Math.floor(p / W);
      for (let dy = 0; dy < 2; dy++) {
        for (let dx = 0; dx < 2; dx++) {
          const o = ((Math.min(H - 1, y + dy)) * W + Math.min(W - 1, x + dx)) * 4;
          px[o] = r; px[o + 1] = g; px[o + 2] = b;
        }
      }
    };
    for (const d of differences) {
      const p = Math.min(totalPx - 1, Math.floor((d.address / dataLen) * totalPx));
      if (d.value2 >= d.value1) mark(p, 255, 82, 82);
      else mark(p, 77, 163, 255);
    }
    ctx.putImageData(img, 0, 0);
    updateMinimapViewport();
  }, [showCompareView, version1Data, version2Data, differences, minimapSize, theme, updateMinimapViewport]);

  // Clic / glisser dans la minimap : navigue les DEUX panneaux (sync)
  const handleMinimapMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const mm = minimapRef.current;
    if (!mm) return;
    const rect = mm.getBoundingClientRect();
    const totalH = totalRows * ROW_HEIGHT;
    const scrollTo = (clientY: number) => {
      const sc1 = scrollRef1.current;
      const sc2 = scrollRef2.current;
      if (!sc1) return;
      const frac = Math.min(1, Math.max(0, (clientY - rect.top) / rect.height));
      const top = frac * totalH - sc1.clientHeight / 2;
      sc1.scrollTop = top;
      if (sc2) sc2.scrollTop = top;
    };
    scrollTo(e.clientY);
    const onMove = (ev: MouseEvent) => scrollTo(ev.clientY);
    const onUp = () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
    };
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
  }, [totalRows]);

  // Ligne d'en-têtes d'offsets (même langage que l'hexdump), alignée sur la
  // grille des valeurs de chaque panneau (gap-2 + adresse 3rem + cellules).
  const offsetHeader = (
    <div
      className="flex gap-2 font-mono text-[10px] px-2 pt-1 pb-0.5 select-none flex-shrink-0"
      style={{ color: theme === 'light' ? 'rgba(0,0,0,0.5)' : 'rgba(255,255,255,0.5)' }}
    >
      <div className="flex-shrink-0 font-semibold" style={{ width: '3rem' }}>Offset</div>
      <div className="flex flex-shrink-0">
        {Array.from({ length: VALUES_PER_ROW }, (_, j) => (
          <div
            key={j}
            className="text-center"
            style={{ width: hexdumpSize === "16b" ? '2.4rem' : '1.85rem', marginRight: '2px' }}
          >
            {(j * (hexdumpSize === "8b" ? 1 : 2)).toString(16).toUpperCase().padStart(2, '0')}
          </div>
        ))}
      </div>
    </div>
  );

  // Render one side's visible rows, forwarding row-specific props only to the
  // rows they concern
  const renderRows = (data: number[], side: "left" | "right") => {
    const rows: JSX.Element[] = [];
    const end = Math.min(visibleRange.end, totalRows);
    for (let rowIndex = visibleRange.start; rowIndex < end; rowIndex++) {
      const startByte = rowIndex * bytesPerRow;
      const endByte = startByte + bytesPerRow - 1;

      const rowCurrentDiff = currentDiffAddress >= startByte && currentDiffAddress <= endByte
        ? currentDiffAddress : -1;
      const rowSelected = selectedAddress !== null && selectedAddress >= startByte && selectedAddress <= endByte
        ? selectedAddress : -1;
      const rowHoveredMap = hoveredMapRegion &&
        hoveredMapRegion.address <= endByte &&
        hoveredMapRegion.address + hoveredMapRegion.size - 1 >= startByte
          ? hoveredMapRegion.address : null;

      rows.push(
        <CompareRow
          key={rowIndex}
          rowIndex={rowIndex}
          data={data}
          otherData={side === "left" ? version2Data : version1Data}
          side={side}
          theme={theme}
          hexdumpSize={hexdumpSize}
          byteOrder={byteOrder}
          hexdumpFormat={hexdumpFormat}
          bytesPerRow={bytesPerRow}
          byteToMapInfo={byteToMapInfo}
          diffAddresses={diffAddresses}
          currentDiffAddress={rowCurrentDiff}
          selectedAddress={rowSelected}
          hoveredMapAddress={rowHoveredMap}
          onSelectAddress={handleSelectAddress}
          onHoverMap={handleHoverMap}
        />
      );
    }
    return rows;
  };

  if (!isOpen) return null;

  const getVersion1Name = () => displayVersions.find((v) => v.id === selectedVersion1)?.name || "";
  const getVersion2Name = () => displayVersions.find((v) => v.id === selectedVersion2)?.name || "";

  return (
    <div className="fixed inset-0 z-[9999] flex items-center justify-center backdrop-blur-sm" style={{ backgroundColor: '#000000a2' }}>
      <div
        ref={modalRef}
        className="rounded-lg shadow-2xl select-none flex flex-col border"
        style={{
          // Surface verre partagée (default/light/oled) — le menu des versions
          // est portalé vers body, le backdrop-filter est donc sans risque.
          ...glass,
          position: position ? "fixed" : "relative",
          top: position ? position.y : undefined,
          left: position ? position.x : undefined,
          width: showCompareView ? `${modalSize.width}px` : "440px",
          height: showCompareView ? `${modalSize.height}px` : "auto",
          minWidth: "440px",
          minHeight: showCompareView ? "400px" : "auto",
        }}
      >
        {/* Header */}
        <div
          className="px-4 py-2 flex items-center justify-between cursor-move flex-shrink-0"
          style={{ borderBottom: `1px solid ${hairline}` }}
          onMouseDown={handleDragStart}
        >
          <span
            className="text-[13px] font-semibold"
            style={{ color: theme === "light" ? "#000000" : "#ffffff" }}
          >
            {t.compare.title}
          </span>
          <button
            onClick={onClose}
            onMouseDown={(e) => e.stopPropagation()}
            className={`p-1 rounded transition-colors ${theme === "light" ? "hover:bg-black/5" : "hover:bg-white/10"}`}
            style={{ color: theme === "light" ? "rgba(0, 0, 0, 0.6)" : "rgba(255, 255, 255, 0.6)" }}
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {!showCompareView ? (
          // Version selection view
          <div className="p-6 flex flex-col gap-4">
            <p
              className="text-[12px] text-center"
              style={{ color: theme === "light" ? "#666666" : "#999999" }}
            >
              {t.compare.selectVersions}
            </p>

            <div className="flex gap-3 items-center">
              {/* Left version */}
              <VersionSelect
                label={t.compare.left}
                placeholder={t.compare.selectVersion}
                versions={displayVersions}
                selectedId={selectedVersion1}
                disabledId={selectedVersion2}
                currentBadge={t.compare.currentBadge}
                theme={theme}
                onSelect={setSelectedVersion1}
              />

              {/* Swap button */}
              <button
                onClick={() => {
                  const temp = selectedVersion1;
                  setSelectedVersion1(selectedVersion2);
                  setSelectedVersion2(temp);
                }}
                className={`p-2 rounded transition-colors mt-5 flex-shrink-0 ${getButtonHoverClass()}`}
                style={{ color: theme === "light" ? "#666666" : "#999999" }}
                title={t.compare.swapVersions}
              >
                <ArrowLeftRight className="w-4 h-4" />
              </button>

              {/* Right version */}
              <VersionSelect
                label={t.compare.right}
                placeholder={t.compare.selectVersion}
                versions={displayVersions}
                selectedId={selectedVersion2}
                disabledId={selectedVersion1}
                currentBadge={t.compare.currentBadge}
                theme={theme}
                onSelect={setSelectedVersion2}
              />
            </div>

            {/* Buttons */}
            <div className="flex gap-2 mt-2">
              <button
                onClick={onClose}
                className={`px-4 py-2 rounded text-[12px] font-medium transition-colors ${getButtonHoverClass()}`}
                style={{
                  color: theme === "light" ? "#666666" : "#999999",
                  border: `1px solid ${theme === "light" ? "#dee2e6" : "rgba(255, 255, 255, 0.2)"}`,
                }}
              >
                {t.common.close}
              </button>
              <button
                onClick={handleCompare}
                disabled={!selectedVersion1 || !selectedVersion2 || isComparing}
                className={`flex-1 px-4 py-2 rounded text-[12px] font-medium transition-colors flex items-center justify-center gap-2 ${
                  selectedVersion1 && selectedVersion2 && !isComparing
                    ? "text-white bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400"
                    : "text-gray-500 bg-gray-700 cursor-not-allowed"
                }`}
              >
                {isComparing && <Loader2 className="w-3.5 h-3.5 animate-spin" />}
                {isComparing ? t.compare.comparing : t.compare.startCompare}
              </button>
            </div>
          </div>
        ) : (
          // Compare view
          <div className="flex-1 flex flex-col overflow-hidden">
            {/* Version headers */}
            <div className="flex flex-shrink-0" style={{ borderBottom: `1px solid ${hairline}` }}>
              <div
                className="flex-1 px-3 py-1.5 text-[11px] font-semibold text-center"
                style={{
                  background: theme === "light" ? "#fee2e2" : "rgba(239, 68, 68, 0.2)",
                  color: theme === "light" ? "#991b1b" : "#fca5a5",
                }}
              >
                {getVersion1Name()}
              </div>
              <div
                className="flex-1 px-3 py-1.5 text-[11px] font-semibold text-center"
                style={{
                  background: theme === "light" ? "#dcfce7" : "rgba(34, 197, 94, 0.2)",
                  color: theme === "light" ? "#166534" : "#86efac",
                }}
              >
                {getVersion2Name()}
              </div>
            </div>

            {/* Dual hexdump view — scrollbars natives masquées, une minimap
                partagée (les deux panneaux scrollent ensemble) les remplace */}
            <div className="flex-1 flex overflow-hidden">
              {/* Left panel */}
              <div className="flex-1 flex flex-col min-w-0" style={{ borderRight: `1px solid ${hairline}` }}>
                {offsetHeader}
                <div
                  ref={scrollRef1}
                  className="flex-1 overflow-auto p-2 pt-0 hexdump-noscrollbar"
                  onScroll={() => handleScroll("left")}
                >
                  <div className="relative" style={{ height: `${totalRows * ROW_HEIGHT}px` }}>
                    {renderRows(version1Data, "left")}
                  </div>
                </div>
              </div>

              {/* Right panel */}
              <div className="flex-1 flex flex-col min-w-0">
                {offsetHeader}
                <div
                  ref={scrollRef2}
                  className="flex-1 overflow-auto p-2 pt-0 hexdump-noscrollbar"
                  onScroll={() => handleScroll("right")}
                >
                  <div className="relative" style={{ height: `${totalRows * ROW_HEIGHT}px` }}>
                    {renderRows(version2Data, "right")}
                  </div>
                </div>
              </div>

              {/* Minimap partagée */}
              <div
                ref={minimapRef}
                onMouseDown={handleMinimapMouseDown}
                className="flex-shrink-0 relative select-none my-2 mr-2 rounded-md overflow-hidden"
                style={{
                  width: `${MINIMAP_WIDTH}px`,
                  border: `1px solid ${hairline}`,
                  cursor: 'pointer',
                  backgroundColor: theme === 'light' ? 'rgba(0,0,0,0.04)' : 'rgba(255,255,255,0.03)',
                }}
                title="Vue d'ensemble — cliquer / glisser pour naviguer"
              >
                <canvas
                  ref={minimapCanvasRef}
                  className="absolute inset-0 w-full h-full"
                  style={{ imageRendering: 'pixelated' }}
                />
                <div
                  ref={minimapViewportRef}
                  className="absolute left-0 right-0 pointer-events-none rounded-[3px]"
                  style={{
                    top: 0,
                    height: 24,
                    border: theme === 'light' ? '1px solid rgba(220,38,38,0.55)' : '1px solid rgba(239,68,68,0.6)',
                    background: theme === 'light'
                      ? 'linear-gradient(135deg, rgba(220,38,38,0.18), rgba(249,115,22,0.12))'
                      : 'linear-gradient(135deg, rgba(220,38,38,0.3), rgba(249,115,22,0.2))',
                    boxShadow: theme === 'light' ? '0 0 0 1px rgba(255,255,255,0.5)' : '0 0 0 1px rgba(0,0,0,0.45)',
                  }}
                />
              </div>
            </div>

            {/* Navigation and format bar */}
            <div
              className="px-4 py-2 flex items-center justify-between flex-shrink-0"
              style={{
                borderTop: `1px solid ${hairline}`,
                background: theme === "light" ? "rgba(0, 0, 0, 0.03)" : "rgba(255, 255, 255, 0.04)",
              }}
            >
              {/* Left: Format buttons */}
              <div className="flex items-center gap-1">
                <div className="flex items-center rounded px-0.5" style={{ background: getButtonBg() }}>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setHexdumpSize("8b")}
                    className={`h-6 px-2 text-[10px] ${
                      hexdumpSize === "8b"
                        ? "bg-blue-600/40 text-white hover:bg-blue-400/40"
                        : getButtonHoverClass()
                    }`}
                    style={{ color: hexdumpSize === "8b" ? (theme === 'light' ? '#000000' : undefined) : getTextColor() }}
                  >
                    8b
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setHexdumpSize("16b")}
                    className={`h-6 px-2 text-[10px] ${
                      hexdumpSize === "16b"
                        ? "bg-blue-600/40 text-white hover:bg-blue-400/40"
                        : getButtonHoverClass()
                    }`}
                    style={{ color: hexdumpSize === "16b" ? (theme === 'light' ? '#000000' : undefined) : getTextColor() }}
                  >
                    16b
                  </Button>
                  {/* Ordre des octets de la vue 16 bits : HiLo ↔ LoHi (toujours visible) */}
                  {(
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setByteOrder(byteOrder === "hilo" ? "lohi" : "hilo")}
                      title={byteOrder === "hilo" ? "High byte first — click for LoHi" : "Low byte first — click for HiLo"}
                      className={`h-6 px-2 text-[10px] ${getButtonHoverClass()}`}
                      style={{ color: getTextColor() }}
                    >
                      {byteOrder === "hilo" ? "HiLo" : "LoHi"}
                    </Button>
                  )}
                </div>
                <div className="flex items-center rounded px-0.5 ml-1" style={{ background: getButtonBg() }}>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setHexdumpFormat("hex")}
                    className={`h-6 px-2 text-[10px] ${
                      hexdumpFormat === "hex"
                        ? "bg-red-600/40 text-white hover:bg-red-400/40"
                        : getButtonHoverClass()
                    }`}
                    style={{ color: hexdumpFormat === "hex" ? (theme === 'light' ? '#000000' : undefined) : getTextColor() }}
                  >
                    Hex
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setHexdumpFormat("dec")}
                    className={`h-6 px-2 text-[10px] ${
                      hexdumpFormat === "dec"
                        ? "bg-red-600/40 text-white hover:bg-red-400/40"
                        : getButtonHoverClass()
                    }`}
                    style={{ color: hexdumpFormat === "dec" ? (theme === 'light' ? '#000000' : undefined) : getTextColor() }}
                  >
                    Dec
                  </Button>
                </div>
              </div>

              {/* Center: Navigation */}
              <div className="flex items-center gap-3">
                <button
                  onClick={() => setShowCompareView(false)}
                  className={`p-1 rounded transition-colors ${getButtonHoverClass()}`}
                  style={{
                    color: theme === "light" ? "#666666" : "#999999",
                    border: `1px solid ${theme === "light" ? "#dee2e6" : "rgba(255, 255, 255, 0.2)"}`,
                  }}
                  title={t.compare.backToSelection}
                >
                  <ChevronLeft className="w-4 h-4" />
                </button>

                <button
                  onClick={goToPrevDiff}
                  disabled={differences.length === 0}
                  className={`p-1 rounded transition-colors disabled:opacity-50 ${getButtonHoverClass()}`}
                  style={{
                    color: theme === "light" ? "#666666" : "#999999",
                    border: `1px solid ${theme === "light" ? "#dee2e6" : "rgba(255, 255, 255, 0.2)"}`,
                  }}
                  title={t.compare.previousDiff}
                >
                  <ChevronLeft className="w-4 h-4" />
                </button>

                <span
                  className="text-[11px] font-mono min-w-[140px] text-center"
                  style={{ color: theme === "light" ? "#000000" : "#ffffff" }}
                >
                  {differences.length > 0 ? (
                    <>
                      {currentDiffIndex + 1} / {differences.length} {t.compare.differences}
                    </>
                  ) : (
                    t.compare.noDifferences
                  )}
                </span>

                <button
                  onClick={goToNextDiff}
                  disabled={differences.length === 0}
                  className={`p-1 rounded transition-colors disabled:opacity-50 ${getButtonHoverClass()}`}
                  style={{
                    color: theme === "light" ? "#666666" : "#999999",
                    border: `1px solid ${theme === "light" ? "#dee2e6" : "rgba(255, 255, 255, 0.2)"}`,
                  }}
                  title={t.compare.nextDiff}
                >
                  <ChevronRight className="w-4 h-4" />
                </button>

                {differences.length > 0 && (
                  <span
                    className="text-[10px] font-mono"
                    style={{ color: theme === "light" ? "#666666" : "#999999" }}
                  >
                    @ {differences[currentDiffIndex]?.address.toString(16).toUpperCase().padStart(5, "0")}
                  </span>
                )}
              </div>

              {/* Right: Spacer for balance */}
              <div style={{ width: '150px' }} />
            </div>
          </div>
        )}

        {/* Resize handle */}
        {showCompareView && (
          <div
            className="absolute bottom-0 right-0 w-4 h-4 cursor-se-resize"
            style={{
              background: "linear-gradient(135deg, transparent 50%, rgba(128, 128, 128, 0.5) 50%)",
            }}
            onMouseDown={handleResizeStart}
          />
        )}
      </div>
    </div>
  );
}
