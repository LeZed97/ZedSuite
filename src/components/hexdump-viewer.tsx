"use client";

import { useEffect, useState, useRef, useMemo, useCallback, memo } from "react";

// Map region interface for highlighting
export interface MapRegion {
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

interface HexdumpViewerProps {
  fileData: number[];
  // Données du fichier ORIGINAL (jamais modifié) : sert à colorer les valeurs
  // modifiées (rouge = au-dessus de l'origine, bleu = en dessous, comme
  // WinOLS 5) et à marquer les modifications dans la minimap.
  originalFileData?: number[];
  fileName: string;
  size?: "8b" | "16b";
  format?: "hex" | "dec";
  containerWidth?: string;
  minWidthOverride?: string;
  theme?: "default" | "light" | "oled";
  mapRegions?: MapRegion[]; // List of maps to highlight
  selectedMapAddress?: number | null; // Currently selected map address (for scrolling to it)
  onMapClick?: (mapRegion: MapRegion) => void; // Callback when a map region is clicked
  onScrollComplete?: () => void; // Callback when scroll to selected map is complete
  // Search results highlighting
  searchResults?: number[]; // Array of addresses where search matches were found
  currentSearchIndex?: number; // Index of the currently focused search result
  searchDataSize?: '8b' | '16b'; // Size of the searched data (to highlight correct bytes)
  scrollKey?: number; // Key to force scroll even when address is the same
  // Search button
  onSearchClick?: () => void; // Callback when search button is clicked
  searchButtonLabel?: string; // Label for the search button (i18n)
}

// Single color for all maps - solid gray with border
const MAP_COLOR = {
  bg: '#3a3a3a',
  border: '#6a6a6a',
  text: '#ffffff',
  labelBg: '#4a4a4a',
};

// Search result highlight colors
const SEARCH_COLOR = {
  bg: '#b8860b', // Dark goldenrod for all results
  currentBg: '#ffd700', // Bright gold for current result
  text: '#000000',
};

// Couleurs des valeurs modifiées vs fichier d'origine (convention WinOLS 5) :
// au-dessus de l'origine = rouge, en dessous = bleu.
const DIFF_COLORS = {
  dark: { above: '#ff5252', below: '#4da3ff' },
  light: { above: '#c62828', below: '#1565c0' },
};

// Largeur de la colonne minimap (px) — réduite de 25 % (32 → 24)
const MINIMAP_WIDTH = 24;

const ROW_HEIGHT = 18;
// Rows rendered above/below the viewport. The visible range is quantized to
// CHUNK-row steps so scrolling only triggers a re-render every CHUNK rows
// instead of on every scroll event.
const OVERSCAN = 30;
const RANGE_CHUNK = 25;

type ByteMapInfo = { mapRegion: MapRegion; isStart: boolean; isEnd: boolean };
type ByteSearchInfo = { isCurrent: boolean; isStart: boolean };

interface HexRowProps {
  rowIndex: number;
  fileData: number[];
  // Fichier original (identité stable) pour la coloration des modifications
  originalFileData: number[] | null;
  fileDataLength: number;
  size: "8b" | "16b";
  format: "hex" | "dec";
  theme: "default" | "light" | "oled";
  bytesPerValue: number;
  valuesPerRow: number;
  bytesPerRow: number;
  byteToMapInfo: Map<number, ByteMapInfo>;
  byteToSearchInfo: Map<number, ByteSearchInfo>;
  // The hovered map ONLY when it intersects this row, null otherwise —
  // keeps the memo effective for every other row.
  hoveredMap: MapRegion | null;
  onByteClick: (byteAddress: number) => void;
  onByteHover: (byteAddress: number | null) => void;
  onLabelClick: (mapRegion: MapRegion) => void;
  onLabelHover: (mapRegion: MapRegion | null) => void;
}

// One hexdump row. Memoized: during scrolling the already-mounted rows keep
// strictly identical props (stable maps/handlers from the parent), so React
// skips them entirely and only mounts the rows entering the window. Without
// this, dragging the scrollbar re-rendered every visible row on every scroll
// event and the table went blank until mouse release.
const HexRow = memo(function HexRow({
  rowIndex,
  fileData,
  originalFileData,
  fileDataLength,
  size,
  format,
  theme,
  bytesPerValue,
  valuesPerRow,
  bytesPerRow,
  byteToMapInfo,
  byteToSearchInfo,
  hoveredMap,
  onByteClick,
  onByteHover,
  onLabelClick,
  onLabelHover,
}: HexRowProps) {
  const textColor = theme === 'light' ? '#000000' : '#ffffff';
  const addressColor = theme === 'light' ? '#000000' : '#e1e1e1';
  const asciiColor = theme === 'light' ? 'rgba(0, 0, 0, 0.7)' : 'rgba(255, 255, 255, 0.7)';
  const emptyColor = theme === 'light' ? 'rgba(0, 0, 0, 0.2)' : 'rgba(255, 255, 255, 0.2)';
  const hoverBg = theme === 'light' ? 'rgba(0, 0, 0, 0.05)' : 'rgba(255, 255, 255, 0.05)';

  const startByte = rowIndex * bytesPerRow;
  const address = startByte.toString(16).toUpperCase().padStart(5, '0');

  const values: JSX.Element[] = [];
  const asciiValues: JSX.Element[] = [];

  // Check if this row contains any map starts (for label display)
  const mapStartsInRow: { mapRegion: MapRegion; byteOffset: number }[] = [];
  for (let j = 0; j < bytesPerRow; j++) {
    const byteOffset = startByte + j;
    const mapInfo = byteToMapInfo.get(byteOffset);
    if (mapInfo?.isStart) {
      mapStartsInRow.push({ mapRegion: mapInfo.mapRegion, byteOffset });
    }
  }

  // ASCII representation
  for (let j = 0; j < bytesPerRow; j++) {
    const byteOffset = startByte + j;
    const mapInfo = byteToMapInfo.get(byteOffset);

    let char = ' ';
    if (byteOffset < fileDataLength) {
      const byte = fileData[byteOffset];
      char = (byte >= 32 && byte <= 126) ? String.fromCharCode(byte) : '.';
    }

    asciiValues.push(
      <span
        key={`ascii-${j}`}
        className={`${mapInfo ? 'cursor-pointer' : ''}`}
        onClick={() => mapInfo && onByteClick(byteOffset)}
      >
        {char}
      </span>
    );
  }

  // Values display
  for (let j = 0; j < valuesPerRow; j++) {
    const byteOffset = startByte + (j * bytesPerValue);
    const mapInfo = byteToMapInfo.get(byteOffset);
    const searchInfo = byteToSearchInfo.get(byteOffset);

    let displayValue = '';
    // -1 = valeur sous l'origine (bleu), +1 = au-dessus (rouge), 0 = intacte
    let diffSign = 0;
    if (byteOffset + bytesPerValue <= fileDataLength) {
      let value: number;

      if (size === "8b") {
        value = fileData[byteOffset];
        displayValue = format === "hex"
          ? value.toString(16).toUpperCase().padStart(2, '0')
          : value.toString(10).padStart(3, '0');
      } else {
        value = fileData[byteOffset] | (fileData[byteOffset + 1] << 8);
        displayValue = format === "hex"
          ? value.toString(16).toUpperCase().padStart(4, '0')
          : value.toString(10).padStart(5, '0');
      }

      // Comparaison à l'origine au niveau de la VALEUR affichée (8/16 bits)
      if (originalFileData && byteOffset + bytesPerValue <= originalFileData.length) {
        const orig = size === "8b"
          ? originalFileData[byteOffset]
          : (originalFileData[byteOffset] | (originalFileData[byteOffset + 1] << 8));
        if (value > orig) diffSign = 1;
        else if (value < orig) diffSign = -1;
      }
    } else {
      displayValue = size === "8b" ? (format === "hex" ? '  ' : '   ') : (format === "hex" ? '    ' : '     ');
    }

    const isInMap = !!mapInfo;
    const isHovered = hoveredMap !== null && mapInfo?.mapRegion === hoveredMap;
    const isSearchResult = !!searchInfo;
    const isCurrentSearchResult = searchInfo?.isCurrent ?? false;

    // Determine background color: search results take priority over map highlighting
    let bgColor: string | undefined;
    if (isSearchResult) {
      bgColor = isCurrentSearchResult ? SEARCH_COLOR.currentBg : SEARCH_COLOR.bg;
    } else if (isInMap) {
      bgColor = isHovered ? MAP_COLOR.border : MAP_COLOR.bg;
    }

    // Determine text color. Priorité : recherche (fond doré, texte noir) >
    // modification vs origine (rouge/bleu WinOLS) > map > normal.
    const diffColors = theme === 'light' ? DIFF_COLORS.light : DIFF_COLORS.dark;
    let cellTextColor: string;
    if (isSearchResult) {
      cellTextColor = SEARCH_COLOR.text;
    } else if (diffSign !== 0) {
      cellTextColor = diffSign > 0 ? diffColors.above : diffColors.below;
    } else if (isInMap) {
      cellTextColor = MAP_COLOR.text;
    } else {
      cellTextColor = displayValue.trim() === '' ? emptyColor : textColor;
    }

    values.push(
      <div
        key={`val-${j}`}
        className={`text-center ${isInMap ? 'cursor-pointer' : 'hover:bg-primary/20'}`}
        style={{
          width: size === "16b" ? '2.4rem' : '1.85rem',
          backgroundColor: bgColor,
          color: cellTextColor,
          marginRight: '2px',
          borderTop: isInMap ? `1px solid ${MAP_COLOR.border}` : undefined,
          borderBottom: isInMap ? `1px solid ${MAP_COLOR.border}` : undefined,
          borderLeft: mapInfo?.isStart ? `1px solid ${MAP_COLOR.border}` : undefined,
          borderRight: mapInfo?.isEnd ? `1px solid ${MAP_COLOR.border}` : undefined,
          fontWeight: isCurrentSearchResult || diffSign !== 0 ? 'bold' : undefined,
          borderRadius: isSearchResult ? '2px' : undefined,
        }}
        onClick={() => mapInfo && onByteClick(byteOffset)}
        onMouseEnter={() => onByteHover(byteOffset)}
        onMouseLeave={() => onByteHover(null)}
      >
        {displayValue}
      </div>
    );
  }

  // Get the first map that starts in this row (for the label)
  const firstMapStart = mapStartsInRow[0];

  return (
    <div>
      {/* Data row */}
      <div
        className="flex gap-3 py-[2px] transition-colors font-mono text-[11px]"
        style={{
          position: 'absolute',
          top: `${rowIndex * ROW_HEIGHT}px`,
          left: 0,
          right: 0,
          height: `${ROW_HEIGHT}px`,
        }}
        onMouseEnter={(e) => {
          if (!hoveredMap) e.currentTarget.style.background = hoverBg;
        }}
        onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}
      >
        {/* Address */}
        <div className="font-semibold flex-shrink-0" style={{ width: '3rem', color: addressColor }}>
          {address}
        </div>

        {/* Values with label overlay */}
        <div className="flex flex-shrink-0 relative">
          {values}

          {/* Map label overlay - always at start of values */}
          {firstMapStart && (() => {
            const { mapRegion } = firstMapStart;
            // Only show a codeblock badge when the map actually belongs to a
            // codeblock (EDC15P). EDC16 & co have codeblock_id null/undefined
            // — showing "CB null" there was a bug.
            const cbStr = typeof mapRegion.codeblock_id === 'number' ? `CB${mapRegion.codeblock_id}` : '';
            const isLabelHovered = hoveredMap === mapRegion;

            return (
              <div
                className="font-mono text-[9px] px-1 cursor-pointer flex items-center absolute"
                style={{
                  left: '0',
                  top: '0',
                  height: '100%',
                  backgroundColor: isLabelHovered ? MAP_COLOR.border : MAP_COLOR.labelBg,
                  color: MAP_COLOR.text,
                  border: `1px solid ${MAP_COLOR.border}`,
                  borderRadius: '2px',
                  whiteSpace: 'nowrap',
                  zIndex: 10,
                }}
                onClick={() => onLabelClick(mapRegion)}
                onMouseEnter={() => onLabelHover(mapRegion)}
                onMouseLeave={() => onLabelHover(null)}
                title={`${mapRegion.name}${cbStr ? ` [${cbStr}]` : ''}`}
              >
                {mapRegion.name} {cbStr && `[${cbStr}]`}
              </div>
            );
          })()}
        </div>

        {/* ASCII */}
        <div className="tracking-normal flex-shrink-0" style={{ color: asciiColor }}>
          {asciiValues}
        </div>
      </div>
    </div>
  );
});

export function HexdumpViewer({
  fileData,
  originalFileData,
  fileName,
  size = "8b",
  format = "hex",
  containerWidth = "30%",
  minWidthOverride,
  theme = "default",
  mapRegions = [],
  selectedMapAddress = null,
  onMapClick,
  onScrollComplete,
  searchResults = [],
  currentSearchIndex = -1,
  searchDataSize = '8b',
  scrollKey = 0,
  onSearchClick,
  searchButtonLabel = "Search",
}: HexdumpViewerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const scrollContentRef = useRef<HTMLDivElement>(null);
  const [visibleRange, setVisibleRange] = useState({ start: 0, end: 100 });
  const [hoveredMap, setHoveredMap] = useState<MapRegion | null>(null);
  // Minimap (remplace la scrollbar) : canvas plein fichier + indicateur de
  // viewport piloté en style direct (aucun re-render au scroll)
  const minimapRef = useRef<HTMLDivElement>(null);
  const minimapCanvasRef = useRef<HTMLCanvasElement>(null);
  const minimapViewportRef = useRef<HTMLDivElement>(null);
  const [minimapSize, setMinimapSize] = useState({ w: 0, h: 0 });

  // Safely get file data length (handle undefined/null)
  const safeFileData = useMemo(() => fileData || [], [fileData]);
  const fileDataLength = safeFileData.length;

  // Calculate bytes per row based on size.
  // CRITIQUE : bytesPerRow doit valoir valuesPerRow * bytesPerValue. L'ancien
  // 16 fixe en mode 8b (8 valeurs de 1 octet, adresses au pas de 16) cachait
  // UN OCTET SUR DEUX — les modifications tombant dans la moitié invisible
  // semblaient ne jamais apparaître.
  const { bytesPerValue, valuesPerRow, bytesPerRow, totalRows } = useMemo(() => {
    const bytesPerValue = size === "8b" ? 1 : 2;
    const valuesPerRow = 8;
    const bytesPerRow = valuesPerRow * bytesPerValue; // 8b: 8, 16b: 16
    const totalRows = Math.ceil(fileDataLength / bytesPerRow);
    return { bytesPerValue, valuesPerRow, bytesPerRow, totalRows };
  }, [size, fileDataLength]);

  // Create a map of byte address -> map info for quick lookup
  const byteToMapInfo = useMemo(() => {
    const map = new Map<number, ByteMapInfo>();

    mapRegions.forEach((region) => {
      const endAddress = region.address + region.size - 1;

      for (let addr = region.address; addr <= endAddress && addr < fileDataLength; addr++) {
        map.set(addr, {
          mapRegion: region,
          isStart: addr === region.address,
          isEnd: addr === endAddress
        });
      }
    });

    return map;
  }, [mapRegions, fileDataLength]);

  // Modifications vs fichier d'origine, au niveau des VALEURS affichées
  // (8/16 bits) : liste d'adresses + signe pour la minimap, compteurs pour le
  // header. Recalculé uniquement quand les données ou le mode changent.
  const safeOriginalData = useMemo(() => (originalFileData && originalFileData.length > 0 ? originalFileData : null), [originalFileData]);
  const diffInfo = useMemo(() => {
    const entries: { addr: number; sign: 1 | -1 }[] = [];
    let above = 0;
    let below = 0;
    if (safeOriginalData) {
      const len = Math.min(fileDataLength, safeOriginalData.length);
      const step = size === "8b" ? 1 : 2;
      for (let addr = 0; addr + step <= len; addr += step) {
        const cur = step === 1
          ? safeFileData[addr]
          : (safeFileData[addr] | (safeFileData[addr + 1] << 8));
        const orig = step === 1
          ? safeOriginalData[addr]
          : (safeOriginalData[addr] | (safeOriginalData[addr + 1] << 8));
        if (cur > orig) { entries.push({ addr, sign: 1 }); above++; }
        else if (cur < orig) { entries.push({ addr, sign: -1 }); below++; }
      }
    }
    return { entries, above, below };
  }, [safeFileData, safeOriginalData, fileDataLength, size]);

  // Create a map of byte address -> search result info for quick lookup
  const byteToSearchInfo = useMemo(() => {
    const map = new Map<number, ByteSearchInfo>();
    const searchBytesPerValue = searchDataSize === '8b' ? 1 : 2;

    searchResults.forEach((addr, index) => {
      const isCurrent = index === currentSearchIndex;
      // Mark all bytes that belong to this search result
      for (let i = 0; i < searchBytesPerValue; i++) {
        map.set(addr + i, {
          isCurrent,
          isStart: i === 0
        });
      }
    });

    return map;
  }, [searchResults, currentSearchIndex, searchDataSize]);

  // Scroll to selected map/search result when it changes - centers the target row vertically
  useEffect(() => {
    if (selectedMapAddress !== null && scrollContentRef.current) {
      const rowIndex = Math.floor(selectedMapAddress / bytesPerRow);
      // Center the target row in the middle of the visible area
      const visibleHeight = scrollContentRef.current.clientHeight;
      const scrollTop = rowIndex * ROW_HEIGHT - visibleHeight / 2 + ROW_HEIGHT / 2;
      scrollContentRef.current.scrollTo({ top: Math.max(0, scrollTop), behavior: 'smooth' });

      // Notify parent that scroll is complete (after animation)
      // Only call if scrollKey is 0 (not a search navigation)
      if (onScrollComplete && scrollKey === 0) {
        setTimeout(() => {
          onScrollComplete();
        }, 500); // Wait for smooth scroll animation to complete
      }
    }
  }, [selectedMapAddress, bytesPerRow, onScrollComplete, scrollKey]);

  useEffect(() => {
    const container = scrollContentRef.current;
    if (!container) return;

    // rAF-throttled: coalesce the scroll events of a frame into one range
    // computation, and quantize the range to RANGE_CHUNK rows so a state
    // update (= re-render) only happens every RANGE_CHUNK rows.
    let ticking = false;
    const updateRange = () => {
      ticking = false;
      const scrollTop = container.scrollTop;
      const containerHeight = container.clientHeight;

      const rawStart = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
      const rawEnd = Math.min(totalRows, Math.ceil((scrollTop + containerHeight) / ROW_HEIGHT) + OVERSCAN);
      const start = Math.floor(rawStart / RANGE_CHUNK) * RANGE_CHUNK;
      const end = Math.min(totalRows, Math.ceil(rawEnd / RANGE_CHUNK) * RANGE_CHUNK);

      // Indicateur de viewport de la minimap : style direct (pas de re-render),
      // dimensions lues en live pour éviter toute closure périmée.
      const vp = minimapViewportRef.current;
      const mm = minimapRef.current;
      if (vp && mm) {
        const totalH = totalRows * ROW_HEIGHT;
        const h = mm.clientHeight;
        if (totalH > 0 && h > 0) {
          const vh = Math.max(10, (containerHeight / totalH) * h);
          const top = Math.min(h - vh, (scrollTop / totalH) * h);
          vp.style.top = `${Math.max(0, top)}px`;
          vp.style.height = `${vh}px`;
        }
      }

      setVisibleRange(prev => (prev.start === start && prev.end === end) ? prev : { start, end });
    };
    const handleScroll = () => {
      if (!ticking) {
        ticking = true;
        requestAnimationFrame(updateRange);
      }
    };

    container.addEventListener('scroll', handleScroll, { passive: true });
    updateRange();

    return () => container.removeEventListener('scroll', handleScroll);
  }, [totalRows, size, format]);

  // --- Minimap : mesure du conteneur (hauteur = zone scrollable) ---
  useEffect(() => {
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
  }, [fileDataLength]);

  // --- Minimap : dessin du fichier complet (texture WinOLS) + marques de
  // modifications. Redessiné uniquement quand les données, le thème, les
  // dimensions ou les diffs changent — jamais au scroll (le viewport est un
  // div séparé piloté en style direct).
  useEffect(() => {
    const canvas = minimapCanvasRef.current;
    if (!canvas || !minimapSize.w || !minimapSize.h || fileDataLength === 0) return;
    const dpr = window.devicePixelRatio || 1;
    const W = Math.max(1, Math.round(minimapSize.w * dpr));
    const H = Math.max(1, Math.round(minimapSize.h * dpr));
    canvas.width = W;
    canvas.height = H;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const img = ctx.createImageData(W, H);
    const data = img.data;
    const totalPx = W * H;
    const light = theme === 'light';

    // Texture façon WinOLS : chaque pixel représente un segment contigu du
    // fichier. Le REMPLISSAGE (runs uniformes de 0x00 ou 0xFF, très majoritaire
    // dans un dump flash) est rendu comme un fond sombre ; les zones de vraies
    // données ressortent en blocs gris, d'autant plus clairs que les valeurs
    // varient. Sans cette distinction, un dump 2 Mo rempli de FF donne une
    // dalle gris clair uniforme illisible.
    const bytesPerPx = fileDataLength / totalPx;
    for (let p = 0; p < totalPx; p++) {
      const start = Math.floor(p * bytesPerPx);
      const end = Math.min(fileDataLength, Math.max(start + 1, Math.floor((p + 1) * bytesPerPx)));
      // Jusqu'à 8 échantillons répartis dans le segment
      const sampleStep = Math.max(1, Math.floor((end - start) / 8));
      let mn = 255;
      let mx = 0;
      let sum = 0;
      let n = 0;
      for (let i = start; i < end; i += sampleStep) {
        const b = safeFileData[i] ?? 0;
        if (b < mn) mn = b;
        if (b > mx) mx = b;
        sum += b;
        n++;
      }
      const isEmpty = mn === mx && (mn === 0x00 || mn === 0xFF);
      // Luminosité ∝ valeur moyenne des octets, comme la vue d'ensemble
      // WinOLS : zones de valeurs basses sombres, valeurs hautes claires.
      // (L'ancienne heuristique à base de variance inversait des zones sur
      // EDC15 — code plein de 0x00 rendu clair.) Les runs uniformes 00/FF
      // (remplissage flash) restent rendus comme un fond neutre.
      const mean = sum / n;
      let v: number;
      if (light) {
        v = isEmpty ? 242 : Math.round(30 + mean * 0.75);
      } else {
        v = isEmpty ? 15 : Math.round(28 + mean * 0.8);
      }
      const o = p * 4;
      data[o] = v;
      data[o + 1] = v;
      data[o + 2] = light ? v : Math.min(255, v + 6); // léger biais bleu (fond #0a0b0f)
      data[o + 3] = 255;
    }

    // Marques de modifications : rouge au-dessus de l'origine, bleu en dessous
    // (pastille 2x2 pour rester visible à l'échelle du fichier entier).
    const mark = (p: number, r: number, g: number, b: number) => {
      const x = p % W;
      const y = Math.floor(p / W);
      for (let dy = 0; dy < 2; dy++) {
        for (let dx = 0; dx < 2; dx++) {
          const xx = Math.min(W - 1, x + dx);
          const yy = Math.min(H - 1, y + dy);
          const o = (yy * W + xx) * 4;
          data[o] = r;
          data[o + 1] = g;
          data[o + 2] = b;
        }
      }
    };
    for (const { addr, sign } of diffInfo.entries) {
      const p = Math.min(totalPx - 1, Math.floor((addr / fileDataLength) * totalPx));
      if (sign > 0) mark(p, 255, 82, 82);
      else mark(p, 77, 163, 255);
    }

    ctx.putImageData(img, 0, 0);

    // Resynchroniser l'indicateur de viewport après (re)dessin/resize
    scrollContentRef.current?.dispatchEvent(new Event('scroll'));
  }, [safeFileData, fileDataLength, diffInfo, minimapSize, theme]);

  // --- Minimap : clic / glisser pour naviguer (centre le viewport) ---
  const handleMinimapMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const mm = minimapRef.current;
    const sc = scrollContentRef.current;
    if (!mm || !sc) return;
    const rect = mm.getBoundingClientRect();
    const totalH = totalRows * ROW_HEIGHT;
    const scrollTo = (clientY: number) => {
      const frac = Math.min(1, Math.max(0, (clientY - rect.top) / rect.height));
      sc.scrollTop = frac * totalH - sc.clientHeight / 2;
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

  const handleByteClick = useCallback((byteAddress: number) => {
    const mapInfo = byteToMapInfo.get(byteAddress);
    if (mapInfo && onMapClick) {
      onMapClick(mapInfo.mapRegion);
    }
  }, [byteToMapInfo, onMapClick]);

  const handleByteHover = useCallback((byteAddress: number | null) => {
    if (byteAddress === null) {
      setHoveredMap(null);
    } else {
      const mapInfo = byteToMapInfo.get(byteAddress);
      setHoveredMap(mapInfo?.mapRegion || null);
    }
  }, [byteToMapInfo]);

  const handleLabelClick = useCallback((mapRegion: MapRegion) => {
    onMapClick?.(mapRegion);
  }, [onMapClick]);

  const handleLabelHover = useCallback((mapRegion: MapRegion | null) => {
    setHoveredMap(mapRegion);
  }, []);

  const minWidth = minWidthOverride ?? (size === "8b" ? "500px" : "580px");
  const getHeaderTextColor = () => theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)';
  const getBorderColor = () => theme === 'light' ? 'rgba(0, 0, 0, 0.1)' : 'rgba(255, 255, 255, 0.1)';

  // Rows to render for the current window
  const rows = useMemo(() => {
    const list: number[] = [];
    for (let i = visibleRange.start; i < visibleRange.end; i++) {
      list.push(i);
    }
    return list;
  }, [visibleRange]);

  return (
    <div
      ref={containerRef}
      className={`h-full bg-transparent flex ${theme === 'light' ? 'light-theme' : ''}`}
      style={{ width: containerWidth, minWidth, marginBottom: '16px', height: 'calc(100% - 16px)' }}
    >
      {/* Main content area */}
      <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
        {/* Header */}
        <div className="p-3 pb-0">
          <div className="mb-3 pb-1 flex items-center justify-between gap-2" style={{ borderBottom: `1px solid ${getBorderColor()}` }}>
            <div className="text-[11px] flex items-center min-w-0 flex-1" style={{ color: getHeaderTextColor() }}>
              <span>
                {fileName.length > 35 ? fileName.substring(0, 30) + '...' : fileName}
              </span>
              <span className="flex-shrink-0 ml-2"> - {(fileDataLength / 1024).toFixed(2)} Ko</span>
              {mapRegions.length > 0 && (
                <span className="ml-2 text-primary flex-shrink-0">
                  • {mapRegions.length} maps
                </span>
              )}
            </div>
            {onSearchClick && (
              <button
                onClick={onSearchClick}
                className="text-[11px] px-3 py-1 rounded font-medium transition-colors duration-200 hover:bg-yellow-500/50"
                style={{
                  color: theme === 'light' ? '#000000' : '#ffffff',
                  background: theme === 'light' ? 'rgba(0, 0, 0, 0.05)' : 'rgba(255, 255, 255, 0.06)',
                  border: `1px solid ${theme === 'light' ? 'rgba(0, 0, 0, 0.12)' : 'rgba(255, 255, 255, 0.12)'}`,
                }}
              >
                {searchButtonLabel}
              </button>
            )}
          </div>

        </div>

        {/* Offset column header (aligné sur la grille des valeurs) */}
        <div
          className="flex gap-3 font-mono text-[10px] px-3 pb-1 select-none flex-shrink-0"
          style={{ color: getHeaderTextColor() }}
        >
          <div className="flex-shrink-0 font-semibold" style={{ width: '3rem' }}>Offset</div>
          <div className="flex flex-shrink-0">
            {Array.from({ length: valuesPerRow }, (_, j) => (
              <div
                key={j}
                className="text-center"
                style={{ width: size === "16b" ? '2.4rem' : '1.85rem', marginRight: '2px' }}
              >
                {(j * bytesPerValue).toString(16).toUpperCase().padStart(2, '0')}
              </div>
            ))}
          </div>
        </div>

        {/* Scrollable content + minimap */}
        <div className="flex-1 flex min-h-0">
        <div
          ref={scrollContentRef}
          className={`flex-1 overflow-auto px-3 pb-3 hexdump-noscrollbar ${theme === 'light' ? 'light-theme' : ''}`}
        >
          {/* Virtualized content */}
          <div
            className="relative"
            style={{ height: `${totalRows * ROW_HEIGHT}px` }}
          >
            {rows.map((rowIndex) => {
              // Pass the hovered map to a row ONLY when it intersects it, so
              // hover changes re-render just the concerned rows and the memo
              // keeps every other row untouched.
              const startByte = rowIndex * bytesPerRow;
              const endByte = startByte + bytesPerRow - 1;
              const rowHoveredMap =
                hoveredMap &&
                hoveredMap.address <= endByte &&
                hoveredMap.address + hoveredMap.size - 1 >= startByte
                  ? hoveredMap
                  : null;

              return (
                <HexRow
                  key={rowIndex}
                  rowIndex={rowIndex}
                  fileData={safeFileData}
                  originalFileData={safeOriginalData}
                  fileDataLength={fileDataLength}
                  size={size}
                  format={format}
                  theme={theme}
                  bytesPerValue={bytesPerValue}
                  valuesPerRow={valuesPerRow}
                  bytesPerRow={bytesPerRow}
                  byteToMapInfo={byteToMapInfo}
                  byteToSearchInfo={byteToSearchInfo}
                  hoveredMap={rowHoveredMap}
                  onByteClick={handleByteClick}
                  onByteHover={handleByteHover}
                  onLabelClick={handleLabelClick}
                  onLabelHover={handleLabelHover}
                />
              );
            })}
          </div>
        </div>

        {/* Minimap du fichier (remplace la scrollbar) : vue d'ensemble du
            binaire, marques rouge/bleu des modifications, viewport draggable */}
        {fileDataLength > 0 && (
          <div
            ref={minimapRef}
            onMouseDown={handleMinimapMouseDown}
            className="flex-shrink-0 relative select-none mb-3 mr-2 rounded-md overflow-hidden"
            style={{
              width: `${MINIMAP_WIDTH}px`,
              border: `1px solid ${getBorderColor()}`,
              cursor: 'pointer',
              backgroundColor: theme === 'light' ? 'rgba(0,0,0,0.04)' : 'rgba(255,255,255,0.03)',
            }}
            title="Vue d'ensemble du fichier — cliquer / glisser pour naviguer"
          >
            <canvas
              ref={minimapCanvasRef}
              className="absolute inset-0 w-full h-full"
              style={{ imageRendering: 'pixelated' }}
            />
            {/* Indicateur de viewport (position pilotée en style direct) —
                teinte rouge dégradée, même identité que le reste de l'app */}
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
        )}
        </div>
      </div>
    </div>
  );
}
