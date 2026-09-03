+"use client";

import { useState, useEffect, useLayoutEffect, useMemo, useRef, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { X, Repeat2 } from "lucide-react";
import axios from "axios";
import { useToast } from "@/hooks/use-toast";
import { useTheme } from "@/contexts/theme-context";
import { useI18n } from "@/contexts/i18n-context";
import { PromptModal } from "@/components/prompt-modal";
import { isBigEndianEcu, hasUnsignedAxes } from "@/lib/ecu-endianness";

// Import Plotly dynamiquement pour ├®viter les probl├¿mes SSR
import dynamic from "next/dynamic";

const Plot = dynamic(() => import("react-plotly.js"), { 
  ssr: false,
  loading: () => null,
});

type ViewMode = "text" | "2d" | "3d";

// Cache pour mémoriser les données extraites de chaque map (par adresse)
// Ce cache évite de recalculer les données à chaque changement de map
const CACHE_VERSION = "2025-01-boost-target-backend-swap-v33";

// Map globale pour sauvegarder les positions de caméra de chaque map 3D
// Persiste entre les montages/démontages du composant
const savedCameraPositions: Record<number, { eye: {x: number, y: number, z: number}, center: {x: number, y: number, z: number}, up: {x: number, y: number, z: number} }> = {};

/**
 * Étiquettes d'axes pour la 3D à espacement uniforme : les coordonnées de la
 * surface sont des INDICES de cellule (une ligne de maillage par cellule,
 * comme WinOLS), et ce helper fournit tickvals (indices) / ticktext (vraies
 * valeurs d'axe). Éclairci à ~12 ticks max pour rester lisible. Reprend la
 * même règle d'inversion Y que plot3DData. Partagé avec le panneau preview
 * de l'éditeur.
 */
export function buildPlot3DTicks(xLabels: string[], yLabels: string[]) {
  const needsYReverse =
    yLabels.length > 0 && parseFloat(yLabels[0]) > parseFloat(yLabels[yLabels.length - 1]);
  const ys = needsYReverse ? [...yLabels].reverse() : yLabels;
  const pick = (labels: string[]) => {
    const step = Math.max(1, Math.ceil(labels.length / 12));
    const vals: number[] = [];
    const text: string[] = [];
    for (let i = 0; i < labels.length; i += step) {
      const v = parseFloat(labels[i]);
      vals.push(i);
      text.push(Number.isFinite(v) ? String(v) : labels[i]);
    }
    return { vals, text };
  };
  const x = pick(xLabels);
  const y = pick(ys);
  return { xTickVals: x.vals, xTickText: x.text, yTickVals: y.vals, yTickText: y.text };
}

// Presse-papier interne pour éviter les permissions du navigateur
// Stocke les valeurs copiées avec leur structure (lignes/colonnes)
interface InternalClipboard {
  values: string[][];  // Tableau 2D pour préserver la structure
  type: 'cell' | 'xAxis' | 'yAxis';
  rows: number;
  cols: number;
}

// Backed by localStorage so a copy in one tab is paste-able in another tab.
const CLIPBOARD_STORAGE_KEY = 'zedsuite_map_clipboard';
let inMemoryClipboard: InternalClipboard | null = null;

function readClipboard(): InternalClipboard | null {
  if (typeof window === 'undefined') return inMemoryClipboard;
  try {
    const raw = window.localStorage.getItem(CLIPBOARD_STORAGE_KEY);
    if (!raw) return null;
    return JSON.parse(raw) as InternalClipboard;
  } catch {
    return inMemoryClipboard;
  }
}

function writeClipboard(value: InternalClipboard | null): void {
  inMemoryClipboard = value;
  if (typeof window === 'undefined') return;
  try {
    if (value === null) {
      window.localStorage.removeItem(CLIPBOARD_STORAGE_KEY);
    } else {
      window.localStorage.setItem(CLIPBOARD_STORAGE_KEY, JSON.stringify(value));
    }
  } catch {
    // localStorage may be unavailable (private mode, quota) — keep in-memory only.
  }
}

interface CachedMapData {
  mapValues: number[][];
  xAxisLabels: string[];
  yAxisLabels: string[];
  axesSwapped: boolean;
  rowsReversed: boolean;
  colsReversed: boolean;
  mapAddress: number;
  fileDataHash: string; // Hash simple pour vérifier si fileData a changé
  version?: string;
  // Dimensions for cache invalidation when map dimensions change
  apiRows?: number;
  apiCols?: number;
  // User invert-display override in effect for this cached view (null = aucun)
  invert?: boolean | null;
}

// Cache global en m├®moire (persiste pendant la session)
const mapDataCache = new Map<string, CachedMapData>();

// Export function to clear the cache (called when solutions are applied)
export const clearMapDataCache = () => {
  mapDataCache.clear();
};

// Clean description by removing Axis IDs suffix
// "Max IQ | X: Engine speed (rpm) | Y: Atm pressure (mbar) | Axis IDs: X=0xC034 Y=0xEC38"
// becomes "Max IQ | X: Engine speed (rpm) | Y: Atm pressure (mbar)"
const cleanDescription = (description: string | undefined): string => {
  if (!description) return '';
  // Remove " | Axis IDs: ..." suffix if present
  return description.replace(/\s*\|\s*Axis IDs:.*$/, '');
};

// Générer un hash pour fileData incluant plusieurs zones du fichier
// IMPORTANT: Inclure des données du milieu pour détecter les modifications de maps
const getFileDataHash = (fileData: number[] | undefined, mapAddress?: number): string => {
  if (!fileData || fileData.length === 0) return 'empty';
  const first = fileData.slice(0, 10).join(',');
  const last = fileData.slice(-10).join(',');
  const length = fileData.length;

  // Include middle of file to detect modifications
  const midStart = Math.floor(length / 2);
  const middle = fileData.slice(midStart, midStart + 10).join(',');

  // If map address provided, include data around the map too
  let mapData = '';
  if (mapAddress && mapAddress > 0 && mapAddress + 20 < length) {
    mapData = '_map' + fileData.slice(mapAddress, mapAddress + 20).join(',');
  }

  return `${length}_${first}_${middle}_${last}${mapData}`;
};

// Cl├® de cache bas├®e sur l'adresse de la map et le projet
const getCacheKey = (address: number, projectName?: string, fileName?: string): string => {
  const project = projectName || 'default';
  const file = fileName || 'default';
  return `map_${project}_${file}_${address}`;
};

// Fonction pour calculer la couleur du dégradé en fonction de la valeur
// Utilise le même dégradé que la vue 3D : bleu -> vert -> jaune -> orange -> rouge
const getValueColor = (value: number, min: number, max: number, theme: "default" | "light" | "oled" = "default"): string => {
  // Normaliser la valeur entre 0 et 1 (si map à plat, utiliser 1 pour obtenir le rouge)
  const normalized = max === min ? 1 : (value - min) / (max - min);

  // Dégradé de couleurs : bleu (0) -> vert (0.25) -> jaune (0.5) -> orange (0.75) -> rouge (1)
  const colorStops = [
    { pos: 0, color: [0, 55, 240] },      // Bleu
    { pos: 0.25, color: [0, 185, 0] },   // Vert
    { pos: 0.5, color: [200, 165, 0] },   // Jaune
    { pos: 0.75, color: [200, 120, 0] },  // Orange
    { pos: 1, color: [250, 0, 0] }        // Rouge
  ];

  // Trouver les deux couleurs entre lesquelles interpoler
  let lowerStop = colorStops[0];
  let upperStop = colorStops[colorStops.length - 1];

  for (let i = 0; i < colorStops.length - 1; i++) {
    if (normalized >= colorStops[i].pos && normalized <= colorStops[i + 1].pos) {
      lowerStop = colorStops[i];
      upperStop = colorStops[i + 1];
      break;
    }
  }

  // Interpoler entre les deux couleurs
  const localNormalized = (normalized - lowerStop.pos) / (upperStop.pos - lowerStop.pos);
  const r = Math.round(lowerStop.color[0] + (upperStop.color[0] - lowerStop.color[0]) * localNormalized);
  const g = Math.round(lowerStop.color[1] + (upperStop.color[1] - lowerStop.color[1]) * localNormalized);
  const b = Math.round(lowerStop.color[2] + (upperStop.color[2] - lowerStop.color[2]) * localNormalized);

  // Ajuster les couleurs selon le thème
  let finalR, finalG, finalB;

  if (theme === 'light') {
    // Pour le thème light : augmenter saturation et luminosité pour des couleurs plus vives
    // Convertir RGB vers HSL
    const rNorm = r / 255;
    const gNorm = g / 255;
    const bNorm = b / 255;
    const max = Math.max(rNorm, gNorm, bNorm);
    const min = Math.min(rNorm, gNorm, bNorm);
    const delta = max - min;

    let h = 0, s = 0, l = (max + min) / 2;

    if (delta !== 0) {
      s = l > 0.5 ? delta / (2 - max - min) : delta / (max + min);

      if (max === rNorm) {
        h = ((gNorm - bNorm) / delta + (gNorm < bNorm ? 6 : 0)) / 6;
      } else if (max === gNorm) {
        h = ((bNorm - rNorm) / delta + 2) / 6;
      } else {
        h = ((rNorm - gNorm) / delta + 4) / 6;
      }
    }

    // Augmenter la saturation de 25% et la luminosité de 10% pour des couleurs plus vives
    s = Math.min(1, s * 1.25);

    // Pour les tons bleus (h entre 0.5 et 0.7), augmenter davantage la luminosité pour la lisibilité du texte noir
    if (h >= 0.5 && h <= 0.7) {
      l = Math.min(0.85, l * 1.35); // Bleus plus clairs
    } else {
      l = Math.min(0.75, l * 1.1); // Autres couleurs : limiter la luminosité à 75%
    }

    // Reconvertir HSL vers RGB
    const hue2rgb = (p: number, q: number, t: number) => {
      if (t < 0) t += 1;
      if (t > 1) t -= 1;
      if (t < 1/6) return p + (q - p) * 6 * t;
      if (t < 1/2) return q;
      if (t < 2/3) return p + (q - p) * (2/3 - t) * 6;
      return p;
    };

    const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
    const p = 2 * l - q;

    finalR = Math.round(hue2rgb(p, q, h + 1/3) * 255);
    finalG = Math.round(hue2rgb(p, q, h) * 255);
    finalB = Math.round(hue2rgb(p, q, h - 1/3) * 255);
  } else {
    // Pour les autres thèmes : assombrir légèrement (90% de l'intensité)
    const darkenFactor = 0.9;
    finalR = Math.round(r * darkenFactor);
    finalG = Math.round(g * darkenFactor);
    finalB = Math.round(b * darkenFactor);
  }

  return `rgb(${finalR}, ${finalG}, ${finalB})`;
};

// Fonction pour transformer une couleur RGB selon le thème (utilisée pour la colorscale 3D)
const transformColorForTheme = (r: number, g: number, b: number, theme: "default" | "light" | "oled" = "default"): string => {
  let finalR, finalG, finalB;

  if (theme === 'light') {
    // Pour le thème light : augmenter saturation et luminosité
    const rNorm = r / 255;
    const gNorm = g / 255;
    const bNorm = b / 255;
    const max = Math.max(rNorm, gNorm, bNorm);
    const min = Math.min(rNorm, gNorm, bNorm);
    const delta = max - min;

    let h = 0, s = 0, l = (max + min) / 2;

    if (delta !== 0) {
      s = l > 0.5 ? delta / (2 - max - min) : delta / (max + min);

      if (max === rNorm) {
        h = ((gNorm - bNorm) / delta + (gNorm < bNorm ? 6 : 0)) / 6;
      } else if (max === gNorm) {
        h = ((bNorm - rNorm) / delta + 2) / 6;
      } else {
        h = ((rNorm - gNorm) / delta + 4) / 6;
      }
    }

    // Augmenter la saturation de 25%
    s = Math.min(1, s * 1.25);

    // Pour les tons bleus, augmenter davantage la luminosité
    if (h >= 0.5 && h <= 0.7) {
      l = Math.min(0.85, l * 1.35); // Bleus plus clairs
    } else {
      l = Math.min(0.75, l * 1.1); // Autres couleurs
    }

    // Reconvertir HSL vers RGB
    const hue2rgb = (p: number, q: number, t: number) => {
      if (t < 0) t += 1;
      if (t > 1) t -= 1;
      if (t < 1/6) return p + (q - p) * 6 * t;
      if (t < 1/2) return q;
      if (t < 2/3) return p + (q - p) * (2/3 - t) * 6;
      return p;
    };

    const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
    const p = 2 * l - q;

    finalR = Math.round(hue2rgb(p, q, h + 1/3) * 255);
    finalG = Math.round(hue2rgb(p, q, h) * 255);
    finalB = Math.round(hue2rgb(p, q, h - 1/3) * 255);
  } else {
    // Pour les autres thèmes : assombrir légèrement (90% de l'intensité)
    const darkenFactor = 1;
    finalR = Math.round(r * darkenFactor);
    finalG = Math.round(g * darkenFactor);
    finalB = Math.round(b * darkenFactor);
  }

  return `rgb(${finalR}, ${finalG}, ${finalB})`;
};

// Marges utilisées pour donner un léger souffle aux fenêtres texte
const TEXT_VIEW_PADDING_WIDTH_BASE = 0; // Espace minimal à droite (réduit car cellules responsives)
const TEXT_VIEW_PADDING_HEIGHT_BOTTOM = 8; // Espace minimal en bas pour les boutons
const TEXT_VIEW_CHROME_HEIGHT = 64; // titre + onglets avec marge réduite

// Bornes d'échelle des cellules (vue texte) — échelle uniforme pour conserver
// le format rectangulaire des cellules à toutes les tailles
const CELL_MIN_SCALE = 0.55; // réduction max : cellules ~31x11 (fenêtre bien plus compacte)
const CELL_MAX_SCALE = 1.3; // agrandissement max : cellules ~73x26
// Espace réservé sous le tableau : la rangée de boutons Text/2D/3D (~26px,
// positionnée en absolute bottom-0 DANS la zone de contenu) + légère respiration
const CELL_BOTTOM_GAP = 30;
// Largeur minimale d'une fenêtre en vue texte : la rangée de boutons
// Text/2D/3D (~150px) doit toujours tenir — les toutes petites maps (1x1, 2x2)
// gardent donc un peu d'espace à droite du tableau, c'est voulu
const TEXT_WINDOW_MIN_WIDTH = 175;

// ViewMode is now managed by parent component (EditorPage)
// No need for sessionStorage anymore

interface MapViewerProps {
  mapData: {
    name: string;
    address: number;
    size: number;
    map_type?: string;
    description?: string;
    confidence?: number;
    // EDC15 multi-codeblock : numéro affiché à côté du titre, comme dans la liste
    codeblock_id?: number;
    dimensions?: {
      TwoDimensional?: {
        rows: number;
        cols: number;
      };
      OneDimensional?: {
        length: number;
      };
    };
    x_axis_address?: number;
    y_axis_address?: number;
    correction_factor?: number;
    offset?: number;
    x_axis_correction?: number;
    y_axis_correction?: number;
    x_axis_offset?: number;
    y_axis_offset?: number;
    x_label?: string;
    y_label?: string;
    y_axis_inverted?: boolean;
    is_little_endian?: boolean;
    data_type?: string; // "UInt8", "UInt16", "UInt32", "Int8", "Int16", "Int32", "Float32"
  };
  fileData: number[];
  projectName?: string;
  fileName?: string;
  viewMode?: ViewMode; // Controlled viewMode from parent
  easyViewMode?: boolean; // Si true, affiche texte + 3D simultanément
  onViewModeChange?: (mode: ViewMode) => void; // Callback to update parent
  onAutoSize?: (width: number, height: number) => void; // Informe le parent de la taille n├®cessaire en vue texte
  onClose?: () => void;
  onDragStart?: (event: React.MouseEvent<HTMLDivElement>) => void;
  currentVersionId?: string | null;
  // Props pour la gestion globale des modifications
  initialChangedCells?: Record<string, number>; // Modifications initiales à charger
  onModificationsChange?: (changedCells: Record<string, number>) => void; // Callback quand les modifications changent
  // Persisted axis-label edits (the parent stores them across map close/reopen).
  initialXAxisLabels?: string[];
  initialYAxisLabels?: string[];
  onAxisLabelsChange?: (axes: { x?: string[]; y?: string[] }) => void;
  // Tells the parent how display row/col indices map back to file row/col, so
  // exports can write modified cells to the correct byte offsets.
  onAxesFlipChange?: (mapAddress: number, flip: { rowsReversed: boolean; colsReversed: boolean }) => void;
  theme?: "default" | "light" | "oled";
  // Notifie le parent qu'un redimensionnement custom est en cours (overlay anti-interaction)
  onResizeActiveChange?: (active: boolean) => void;
  // Callback pour informer le parent des infos de sélection (curseur global)
  onSelectionChange?: (info: {
    mapName: string;
    mapAddress: number;
    dimensions: string;
    selectedCount: number;
    selectedCells: Array<{ row: number; col: number; address: number; value: number }>;
  } | null) => void;
  // Callback pour partager les données 3D avec le parent (pour Preview window)
  onPlot3DDataChange?: (mapAddress: number, data: {
    plot3DData: any[];
    xAxisLabels: string[];
    yAxisLabels: string[];
    canShow3D: boolean;
  }) => void;
  // Callback pour ouvrir le modal Properties
  onOpenProperties?: () => void;
  // Paramètres d'affichage personnalisés pour cette map
  displaySettings?: {
    xAxis?: {
      mirror?: boolean;
      factor?: number;
      offset?: number;
      divisor?: number;
      precision?: number;
    };
    yAxis?: {
      mirror?: boolean;
      factor?: number;
      offset?: number;
      divisor?: number;
      precision?: number;
    };
    map?: {
      factor?: number;
      offset?: number;
      divisor?: number;
      precision?: number;
      // Override utilisateur de l'orientation : bascule le swap par défaut de la
      // map (undefined = défaut). Affichage seulement, pas l'export.
      invertDisplay?: boolean;
    };
  };
  // Callback quand l'utilisateur (dé)active l'inversion d'affichage depuis le
  // bouton du header. Le parent persiste dans les display-settings du projet.
  onToggleInvertDisplay?: (mapAddress: number, invert: boolean) => void;
  // Settings globaux de l'utilisateur
  disableTableColors?: boolean;
  disableGraphColors?: boolean;
  // All maps for finding similar maps
  allMaps?: Array<{
    name: string;
    address: number;
    size: number;
    dimensions?: {
      TwoDimensional?: {
        rows: number;
        cols: number;
      };
      OneDimensional?: {
        length: number;
      };
    };
  }>;
  // Callback to apply changes to similar maps
  onApplyToSimilarMaps?: (targetMaps: number[], copyType: 'modifications' | 'all') => void;
  // Value to use for +/- keyboard shortcuts (from toolbar Add input)
  incrementValue?: number;
  // Command from toolbar to apply modification (add/fill) to selected cells
  modifyCommand?: { operation: 'add' | 'fill'; value: number; timestamp: number } | null;
  // Callback to scroll hexdump to this map's address
  onViewInHexdump?: () => void;
  // Whether this map is the active/focused map (for keyboard shortcuts)
  isActive?: boolean;
  // ECU type for determining endianness (EDC16 uses Big-Endian, EDC15 uses Little-Endian)
  ecuType?: string;
}

export function MapViewer({
  mapData,
  fileData,
  projectName,
  fileName,
  viewMode: controlledViewMode,
  easyViewMode = false,
  onViewModeChange,
  onAutoSize,
  onClose,
  onDragStart,
  currentVersionId,
  initialChangedCells,
  onModificationsChange,
  initialXAxisLabels,
  initialYAxisLabels,
  onAxisLabelsChange,
  onAxesFlipChange,
  theme: themeProp,
  onResizeActiveChange,
  onSelectionChange,
  onPlot3DDataChange,
  onOpenProperties,
  displaySettings,
  onToggleInvertDisplay,
  disableTableColors = false,
  disableGraphColors = false,
  allMaps,
  onApplyToSimilarMaps,
  incrementValue = 1,
  modifyCommand,
  onViewInHexdump,
  isActive = false,
  ecuType,
}: MapViewerProps) {
  // Use controlled viewMode if provided, otherwise use internal state
  const [internalViewMode, setInternalViewMode] = useState<ViewMode>("text");
  const viewMode = controlledViewMode ?? internalViewMode;
  const { toast } = useToast();
  const { theme: themeContext } = useTheme();
  const theme = themeProp ?? themeContext;
  const { t } = useI18n();

  // Active value-edit prompt (cell / X axis / Y axis). null = closed. The
  // themed PromptModal replaces the native window.prompt() so it matches the
  // rest of the editor's windows.
  const [valuePrompt, setValuePrompt] = useState<{
    title: string;
    value: string;
    onSubmit: (input: string) => void;
  } | null>(null);

  // Helper function for window header background (glass family — same tokens
  // as the editor page's getWindowHeaderBg; solid-ish for data readability)
  const getWindowHeaderBg = () => {
    switch (theme) {
      case 'light': return 'rgba(255, 255, 255, 0.85)';
      case 'oled': return '#0f0f11';
      default: return 'rgba(28, 32, 44, 0.85)';
    }
  };

  const getWindowHeaderTextColor = () => {
    return theme === 'light' ? '#000000' : '#ffffff';
  };

  // Helper functions for table cells (default theme tinted toward the glass
  // ground instead of pure black; OLED keeps true black)
  const getCellBg = () => {
    switch (theme) {
      case 'light': return '#ffffff';
      case 'oled': return '#000000';
      default: return '#0d1017';
    }
  };
  const getCellBgHover = () => {
    switch (theme) {
      case 'light': return '#f8f9fa';
      case 'oled': return '#1a1a1a';
      default: return '#181c26';
    }
  };
  const getCellTextColor = () => theme === 'light' ? 'rgba(0, 0, 0, 0.7)' : 'rgba(255, 255, 255, 0.7)';
  const getCellBorderColor = () => theme === 'light' ? 'rgba(0, 0, 0, 0.2)' : 'rgba(255, 255, 255, 0.2)';
  // Fond des cellules/axes sélectionnés — le gris sombre écrase le texte noir
  // du thème clair, qui reçoit donc un gris-bleu clair lisible
  const getSelectionBg = () => (theme === 'light' ? '#c3cbd7e6' : '#595757e6');

  // Helper functions for view mode buttons (Text/2D/3D)
  const getViewButtonBg = () => {
    switch (theme) {
      case 'light': return '#ffffff';
      case 'oled': return '#111111';
      default: return '#141824';
    }
  };
  const getViewButtonBgHover = () => {
    switch (theme) {
      case 'light': return '#f8f9fa';
      case 'oled': return '#1f1f1f';
      default: return '#1c212e';
    }
  };

  // Helper function for axis cells background
  const getAxisCellBg = () => {
    switch (theme) {
      case 'light':
        return '#ffffff';
      case 'oled':
        return '#000000';
      default: // 'default' theme — glass-family dark tint
        return '#141824';
    }
  };

  const setViewMode = (mode: ViewMode) => {
    if (onViewModeChange) {
      // Controlled mode: notify parent
      onViewModeChange(mode);
    } else {
      // Uncontrolled mode: use internal state
      setInternalViewMode(mode);
    }
  };
const [mapValues, setMapValues] = useState<number[][]>([]);
const [xAxisLabels, setXAxisLabels] = useState<string[]>([]);
const [yAxisLabels, setYAxisLabels] = useState<string[]>([]);
const [changedCells, setChangedCells] = useState<Record<string, number>>({});
const originalValuesRef = useRef<number[][]>([]);
const [contextMenu, setContextMenu] = useState<{
  x: number;
  y: number;
  type: 'cell' | 'xAxis' | 'yAxis';
  row?: number;
  col?: number;
  index?: number;
  value?: number;
} | null>(null);
const [adjustedContextMenuPos, setAdjustedContextMenuPos] = useState<{ x: number; y: number } | null>(null);
const contextMenuRef = useRef<HTMLDivElement | null>(null);
// Header context menu (right-click on map title bar)
const [headerContextMenu, setHeaderContextMenu] = useState<{ x: number; y: number } | null>(null);
const headerContextMenuRef = useRef<HTMLDivElement | null>(null);

// Close header context menu when clicking outside
useEffect(() => {
  if (!headerContextMenu) return;

  const handleClickOutside = (e: MouseEvent) => {
    if (headerContextMenuRef.current && !headerContextMenuRef.current.contains(e.target as Node)) {
      setHeaderContextMenu(null);
    }
  };

  // Use setTimeout to avoid immediate close from the same click that opened the menu
  const timeoutId = setTimeout(() => {
    document.addEventListener('mousedown', handleClickOutside);
  }, 0);

  return () => {
    clearTimeout(timeoutId);
    document.removeEventListener('mousedown', handleClickOutside);
  };
}, [headerContextMenu]);

// Modal for absolute value input (with drag support)
const [absoluteValueModal, setAbsoluteValueModal] = useState<{
  isOpen: boolean;
  inputValue: string;
  onConfirm: (value: number) => void;
  position: { x: number; y: number };
  isDragging: boolean;
  dragOffset: { x: number; y: number };
} | null>(null);
// Modal for similar maps
const [similarMapsModal, setSimilarMapsModal] = useState<{
  isOpen: boolean;
  copyType: 'modifications' | 'all';
  selectedMaps: number[]; // addresses of selected maps
} | null>(null);
const tableContainerRef = useRef<HTMLDivElement | null>(null);
const tableRef = useRef<HTMLTableElement | null>(null);
const lastAutoSizeRef = useRef<{ w: number; h: number }>({ w: 0, h: 0 });
const skipAutoSizeRef = useRef<boolean>(false);

  // Derived display swap for Boost target: show RPM on Y (left) and IQ on X (top)
  const mapNameLowerDisplay = (mapData.name || "").toLowerCase();
  const isBoostTargetDisplay = useMemo(
    () => mapNameLowerDisplay.includes("boost target map"),
    [mapNameLowerDisplay]
  );
  const isInjectorDuration00Display = useMemo(
    () => mapNameLowerDisplay.includes("injector duration 00") || mapNameLowerDisplay === "duration 00",
    [mapNameLowerDisplay]
  );
  // Treat EDC16U34's "Duration NN" names as the same family as "Injector Duration NN"
  // when applying display-layer ordering decisions.
  const isInjectorDurationDisplay = useMemo(
    () => mapNameLowerDisplay.includes("injector duration") || /^duration \d+$/.test(mapNameLowerDisplay),
    [mapNameLowerDisplay]
  );

  const transposeMatrix = (matrix: number[][]): number[][] => {
    if (!matrix.length) return [];
    return matrix[0].map((_, colIndex) => matrix.map(row => row[colIndex]));
  };

  const isSelectorInjector = (mapData.name || "").toLowerCase().includes("selector for injector duration");

  // Base display data (before ordering adjustments)
  // Boost target: no display transpose here; axes handled at read-time
  let displayMapValues = mapValues;
  let displayXAxisLabels = xAxisLabels;
  let displayYAxisLabels = yAxisLabels;
  // Track whether the display→mapValues mapping needs row/col mirroring.
  // Cell clicks come back in display coords; updateCellValue writes to
  // mapValues, so we need this to translate the indices.
  let displayRowsFlippedCount = 0;
  let displayColsFlippedCount = 0;

  // Ordering adjustments. The helpers below return a `flipped` flag so the
  // caller can record whether the row/col axis was mirrored relative to
  // mapValues; we need that to translate display-coord clicks back to
  // mapValues indices in updateCellValue.
  const ensureXAsc = (labels: string[], values: number[][]) => {
    if (labels.length > 1) {
      const first = parseFloat(labels[0]);
      const last = parseFloat(labels[labels.length - 1]);
      if (!Number.isNaN(first) && !Number.isNaN(last) && first > last) {
        return {
          labels: [...labels].reverse(),
          values: values.map(row => [...row].reverse()),
          flipped: true,
        };
      }
    }
    return { labels, values, flipped: false };
  };

  const ensureYDesc = (labels: string[], values: number[][]) => {
    if (labels.length > 1) {
      const first = parseFloat(labels[0]);
      const last = parseFloat(labels[labels.length - 1]);
      if (!Number.isNaN(first) && !Number.isNaN(last) && first < last) {
        return {
          labels: [...labels].reverse(),
          values: [...values].reverse(),
          flipped: true,
        };
      }
    }
    return { labels, values, flipped: false };
  };

  const ensureYAsc = (labels: string[], values: number[][]) => {
    if (labels.length > 1) {
      const first = parseFloat(labels[0]);
      const last = parseFloat(labels[labels.length - 1]);
      if (!Number.isNaN(first) && !Number.isNaN(last) && first > last) {
        return {
          labels: [...labels].reverse(),
          values: [...values].reverse(),
          flipped: true,
        };
      }
    }
    return { labels, values, flipped: false };
  };
  const xAdjusted = ensureXAsc(displayXAxisLabels, displayMapValues);
  displayXAxisLabels = xAdjusted.labels;
  displayMapValues = xAdjusted.values;
  if (xAdjusted.flipped) displayColsFlippedCount++;
 

  // Y ordering:
  const isEgrMapDisplay =
    ((mapData.name || "").toLowerCase().includes("egr")) &&
    !(mapData.name || "").toLowerCase().includes("temperature");
  // - selector ascending
  // - injector duration maps: ascending (small -> grand vers le bas, comme WinOLS)
  // - others: descending
  if (isSelectorInjector) {
    if (displayYAxisLabels.length > 1) {
      const first = parseFloat(displayYAxisLabels[0]);
      const last = parseFloat(displayYAxisLabels[displayYAxisLabels.length - 1]);
      if (!Number.isNaN(first) && !Number.isNaN(last) && first > last) {
        displayYAxisLabels = [...displayYAxisLabels].reverse();
        displayMapValues = [...displayMapValues].reverse();
        displayRowsFlippedCount++;
      }
    }
  } else if (isInjectorDurationDisplay) {
    // Injector durations (incl. EDC16U34's bare "Duration NN" naming): force
    // Y descending (highest RPM at top, lowest at bottom).
    const yAdjusted = ensureYDesc(displayYAxisLabels, displayMapValues);
    displayYAxisLabels = yAdjusted.labels;
    displayMapValues = yAdjusted.values;
    if (yAdjusted.flipped) displayRowsFlippedCount++;
  } else if (isEgrMapDisplay) {
    // EGR: afficher RPM décroissant (max en haut, 0 en bas) comme WinOLS
    const yAdjusted = ensureYDesc(displayYAxisLabels, displayMapValues);
    displayYAxisLabels = yAdjusted.labels;
    displayMapValues = yAdjusted.values;
    if (yAdjusted.flipped) displayRowsFlippedCount++;
  } else {
    const yAdjusted = ensureYDesc(displayYAxisLabels, displayMapValues);
    displayYAxisLabels = yAdjusted.labels;
    displayMapValues = yAdjusted.values;
    if (yAdjusted.flipped) displayRowsFlippedCount++;
  }

  // Appliquer les settings de miroir depuis displaySettings (prioritaire sur la logique par défaut)
  if (displaySettings?.xAxis?.mirror) {
    // Inverser l'axe X (les colonnes)
    displayXAxisLabels = [...displayXAxisLabels].reverse();
    displayMapValues = displayMapValues.map(row => [...row].reverse());
    displayColsFlippedCount++;
  }
  if (displaySettings?.yAxis?.mirror) {
    // Inverser l'axe Y (les lignes)
    displayYAxisLabels = [...displayYAxisLabels].reverse();
    displayMapValues = [...displayMapValues].reverse();
    displayRowsFlippedCount++;
  }

  // Flip state (rows/cols mirrored vs mapValues) figé AVANT la transposition
  // d'inversion : les compteurs ci-dessus décrivent l'orientation des données
  // telles qu'assemblées depuis le fichier (matrice `mapValues`).
  const preRowsFlipped = (displayRowsFlippedCount % 2) === 1;
  const preColsFlipped = (displayColsFlippedCount % 2) === 1;

  // Bouton d'inversion du header (displaySettings.map.invertDisplay) : on
  // transpose le RÉSULTAT final (comme le mirror), sans toucher la lecture.
  // display[i][j] = pre[j][i] ; les lignes deviennent les colonnes et
  // inversement, donc on échange aussi les labels X/Y. Les valeurs restent
  // exactes (aucune relecture à un mauvais offset).
  const displayTransposed = displaySettings?.map?.invertDisplay === true;
  if (displayTransposed && displayMapValues.length > 0) {
    displayMapValues = transposeMatrix(displayMapValues);
    const tmp = displayXAxisLabels;
    displayXAxisLabels = displayYAxisLabels;
    displayYAxisLabels = tmp;
  }

  // Flips nets APRÈS transposition (les lignes d'affichage ↔ colonnes de
  // mapValues quand transposé), avant l'ordonnancement final ci-dessous.
  let netRowsFlipped = displayTransposed ? preColsFlipped : preRowsFlipped;
  let netColsFlipped = displayTransposed ? preRowsFlipped : preColsFlipped;

  // Ordonnancement FINAL, appliqué quelle que soit l'orientation (défaut,
  // transposé) pour garantir une règle unique et stable :
  //   - axe du haut (X d'affichage / colonnes) : croissant de GAUCHE à DROITE
  //   - axe de gauche (Y d'affichage / lignes) : croissant de BAS en HAUT
  //     (donc décroissant du haut vers le bas)
  // Chaque reversal bascule aussi le flip net correspondant, pour que
  // toMapCoords/toDisplayCoords traduisent toujours les clics correctement.
  // Exceptions (on NE reforce PAS l'ordre) :
  //   - familles à axe Y délibérément ASCENDANT (injector duration / selector) ;
  //   - axe pour lequel l'utilisateur a activé un MIROIR manuel : son choix
  //     explicite prime, sinon le miroir serait annulé à chaque render.
  {
    const xMirrored = !!displaySettings?.xAxis?.mirror;
    const yMirrored = !!displaySettings?.yAxis?.mirror;
    // NB : en mode transposé, le miroir X utilisateur agit visuellement sur
    // l'axe qui est devenu Y et inversement ; on protège donc les deux axes
    // dès qu'un miroir est présent pour rester prévisible.
    const anyMirror = xMirrored || yMirrored;

    if (!anyMirror) {
      const xFinal = ensureXAsc(displayXAxisLabels, displayMapValues);
      displayXAxisLabels = xFinal.labels;
      displayMapValues = xFinal.values;
      if (xFinal.flipped) netColsFlipped = !netColsFlipped;

      const keepYAscending = isSelectorInjector || isInjectorDurationDisplay;
      if (!keepYAscending) {
        const yFinal = ensureYDesc(displayYAxisLabels, displayMapValues);
        displayYAxisLabels = yFinal.labels;
        displayMapValues = yFinal.values;
        if (yFinal.flipped) netRowsFlipped = !netRowsFlipped;
      }
    }
  }

  // Net flip state between display indices and mapValues indices.
  const displayRowsFlipped = netRowsFlipped;
  const displayColsFlipped = netColsFlipped;

  // Translate display coords (from DOM clicks / rendered table) into mapValues
  // coords (which is what selectedCells, changedCells and mapValues all use).
  // Without this, editing the top-left cell on a Y-reversed map mutates the
  // bottom-left cell of mapValues — the "mirror" bug the user reports.
  // Quand la vue est transposée, display(row,col) ↦ mapValues(col,row) après
  // annulation des miroirs sur chaque axe d'affichage.
  const toMapCoords = (displayRow: number, displayCol: number) => {
    const totalRows = displayMapValues.length;
    const totalCols = displayMapValues[0]?.length ?? 0;
    const r = displayRowsFlipped && totalRows > 0 ? (totalRows - 1 - displayRow) : displayRow;
    const c = displayColsFlipped && totalCols > 0 ? (totalCols - 1 - displayCol) : displayCol;
    return displayTransposed ? { row: c, col: r } : { row: r, col: c };
  };
  // Inverse: translate mapValues coords back to display coords for selection
  // rendering (we store selectedCells in mapValues coords, but the table
  // iterates in display order).
  const toDisplayCoords = (mapRow: number, mapCol: number) => {
    const totalRows = displayMapValues.length;
    const totalCols = displayMapValues[0]?.length ?? 0;
    // Inverse de toMapCoords : d'abord dé-transposer, puis ré-appliquer les
    // miroirs d'affichage.
    const dr = displayTransposed ? mapCol : mapRow;
    const dc = displayTransposed ? mapRow : mapCol;
    return {
      row: displayRowsFlipped && totalRows > 0 ? (totalRows - 1 - dr) : dr,
      col: displayColsFlipped && totalCols > 0 ? (totalCols - 1 - dc) : dc,
    };
  };

  // --- Traduction d'un index d'axe AFFICHÉ vers l'axe SOURCE (state
  // xAxisLabels/yAxisLabels) en tenant compte de la transposition ET des
  // miroirs. Toutes les opérations d'axe (sélection +/-, menus contextuels
  // copy/paste/increase/decrease/absolute/original) passent par là pour
  // écrire au bon tableau, au bon index, quelle que soit l'orientation.
  //
  // Axe X d'affichage = colonnes. En transposé il pointe vers l'axe Y source.
  // Le flip de colonne relie l'index affiché à l'index source.
  const displayXAxisToSource = (displayIdx: number): { axis: 'x' | 'y'; index: number } => {
    const len = displayXAxisLabels.length;
    const srcIdx = displayColsFlipped && len > 0 ? (len - 1 - displayIdx) : displayIdx;
    return { axis: displayTransposed ? 'y' : 'x', index: srcIdx };
  };
  // Axe Y d'affichage = lignes. En transposé il pointe vers l'axe X source.
  const displayYAxisToSource = (displayIdx: number): { axis: 'x' | 'y'; index: number } => {
    const len = displayYAxisLabels.length;
    const srcIdx = displayRowsFlipped && len > 0 ? (len - 1 - displayIdx) : displayIdx;
    return { axis: displayTransposed ? 'x' : 'y', index: srcIdx };
  };
  // Applique une transformation à un libellé d'axe source (dispatch x/y).
  const mutateSourceAxis = (
    target: { axis: 'x' | 'y'; index: number },
    fn: (current: string) => string
  ) => {
    if (target.axis === 'x') {
      setXAxisLabels(prev => {
        const next = [...prev];
        if (target.index >= 0 && target.index < next.length) next[target.index] = fn(next[target.index]);
        return next;
      });
    } else {
      setYAxisLabels(prev => {
        const next = [...prev];
        if (target.index >= 0 && target.index < next.length) next[target.index] = fn(next[target.index]);
        return next;
      });
    }
  };
  // Lit la valeur source courante d'un axe (pour copy / original).
  const readSourceAxis = (target: { axis: 'x' | 'y'; index: number }): string => {
    const arr = target.axis === 'x' ? xAxisLabels : yAxisLabels;
    return arr[target.index] ?? '';
  };
  // Helpers "haut niveau" : depuis un index d'axe AFFICHÉ.
  const mutateDisplayXAxis = (displayIdx: number, fn: (current: string) => string) =>
    mutateSourceAxis(displayXAxisToSource(displayIdx), fn);
  const mutateDisplayYAxis = (displayIdx: number, fn: (current: string) => string) =>
    mutateSourceAxis(displayYAxisToSource(displayIdx), fn);

  // Injector duration 00: handled in Y ordering (descending); no extra reversal here

  // Injector duration 01-05: no special transpose here (keep axes/values as read)


  // Ref local pour savoir si c'est le premier render de cette instance du composant
  // Se réinitialise à false à chaque mount, mais persiste pendant les re-renders
  const hasCalculatedSizeRef = useRef<boolean>(false);

  // Taille naturelle RÉELLE du tableau à l'échelle 1, mesurée dans le DOM lors
  // du calcul initial. Sert de référence exacte pour l'échelle des cellules
  // (les estimations par constantes dérivent de quelques pixels, ce qui
  // décalait légèrement l'échelle à l'ouverture).
  const naturalTableSizeRef = useRef<{ w: number; h: number } | null>(null);

  // Stocker onAutoSize dans un ref pour éviter de re-déclencher useLayoutEffect
  const onAutoSizeRef = useRef(onAutoSize);
  useEffect(() => {
    onAutoSizeRef.current = onAutoSize;
  }, [onAutoSize]);

  // Réinitialise le cache d'auto-size quand on change de map
  useEffect(() => {
    lastAutoSizeRef.current = { w: 0, h: 0 };
    hasCalculatedSizeRef.current = false; // Reset pour permettre un nouveau calcul
    naturalTableSizeRef.current = null;
  }, [mapData.address]);


  const [selectedCells, setSelectedCells] = useState<Set<string>>(new Set());
  // Curseur clavier (coordonnées d'AFFICHAGE) : posé au clic, déplacé par
  // les flèches ; Ctrl+flèche étend la sélection (même modificateur que le
  // Ctrl+clic), Ctrl+C / Ctrl+V copient-collent la sélection.
  const keyboardCursorRef = useRef<{ row: number; col: number } | null>(null);
  const [isDragging, setIsDragging] = useState<boolean>(false);
  const [dragStart, setDragStart] = useState<{ row: number; col: number } | null>(null);
  const [isCtrlDragging, setIsCtrlDragging] = useState<boolean>(false);
  const [initialSelection, setInitialSelection] = useState<Set<string>>(new Set());
  const [hasMovedDuringDrag, setHasMovedDuringDrag] = useState<boolean>(false);

  // États pour la sélection des cellules d'axes
  const [selectedXAxisCells, setSelectedXAxisCells] = useState<Set<number>>(new Set());
  const [selectedYAxisCells, setSelectedYAxisCells] = useState<Set<number>>(new Set());
  const [isAxisDragging, setIsAxisDragging] = useState<boolean>(false);
  const [axisDragStart, setAxisDragStart] = useState<{ axis: 'x' | 'y'; index: number } | null>(null);
  const originalXAxisLabelsRef = useRef<string[]>([]);
  const originalYAxisLabelsRef = useRef<string[]>([]);
const [axesSwapped, setAxesSwapped] = useState<boolean>(false); // Track if axes were swapped for 3D view
  const [plotlyReady, setPlotlyReady] = useState<boolean>(true); // Plotly dynamique, on force true pour ne pas bloquer le rendu
  const [currentMapAddress, setCurrentMapAddress] = useState<number | null>(null); // Track which map data is loaded
  const [isDraggingWindow, setIsDraggingWindow] = useState<boolean>(false); // Track if window is being dragged
  const dragEndTimeoutRef = useRef<NodeJS.Timeout | null>(null); // Timeout to re-enable auto-sizing after drag ends

  // Dimensions dynamiques des cellules (vue texte) : les cellules de valeurs ET
  // les cellules d'axes partagent exactement les mêmes dimensions, appliquées
  // via des variables CSS sur le conteneur (pas de re-render React à chaque
  // pixel de resize = fluide). Échelle UNIFORME : le ratio largeur/hauteur des
  // cellules est constant, elles restent rectangulaires à toutes les tailles.
  const cellScaleRef = useRef(1); // échelle courante des cellules
  const isCustomResizingRef = useRef(false); // resize custom en cours (la poignée pilote tout)

  // Applique l'échelle s aux cellules via variables CSS (valeurs + axes,
  // exactement les mêmes) — aucun re-render React, fluide
  const applyCellVars = useCallback((container: HTMLElement, s: number) => {
    // Valeurs sous-pixel (pas d'arrondi au px entier) : un arrondi de 1px
    // multiplié par le nombre de lignes/colonnes ferait sauter la fenêtre
    // par paliers — le sous-pixel garantit un redimensionnement fluide.
    // Taille de base : 56x20 (ratio rectangulaire ~2.8)
    const w = Math.max(30, 56 * s);
    const h = Math.max(12, 20 * s);
    const yAxisW = Math.max(26, 44 * s);
    const font = Math.max(8, Math.min(13, Math.round(11 * s))); // police en px entiers (netteté)
    const pad = s < 0.9 ? '1px 2px' : '2px 4px';
    container.style.setProperty('--zs-cell-w', `${w.toFixed(2)}px`);
    container.style.setProperty('--zs-cell-h', `${h.toFixed(2)}px`);
    container.style.setProperty('--zs-yaxis-w', `${yAxisW.toFixed(2)}px`);
    container.style.setProperty('--zs-yaxis-max', `${(yAxisW + 16).toFixed(2)}px`);
    container.style.setProperty('--zs-cell-font', `${font}px`);
    container.style.setProperty('--zs-cell-pad', pad);
  }, []);

  // Inversion d'affichage : les dimensions passent de RxC à CxR, donc la fenêtre
  // doit se recaler sur le nouveau tableau. On rejoue le calcul initial
  // d'auto-size (même reset que lors d'un changement de map) au lieu de le
  // laisser bloqué par hasCalculatedSizeRef. useLayoutEffect (pas useEffect) et
  // déclaré JUSTE AVANT le layout-effect d'auto-size, pour que le reset
  // s'applique de façon synchrone avant que l'auto-size ne relise la garde.
  useLayoutEffect(() => {
    lastAutoSizeRef.current = { w: 0, h: 0 };
    hasCalculatedSizeRef.current = false;
    naturalTableSizeRef.current = null;
    cellScaleRef.current = 1;
  }, [displayTransposed]);

  // Informe le parent de la taille nécessaire pour la vue texte (évite les boucles)
  useLayoutEffect(() => {
    // En mode EasyView ou en mode text normal
    if ((!easyViewMode && viewMode !== "text") || skipAutoSizeRef.current) {
      return;
    }

    const tableEl = tableRef.current;
    if (!tableEl) {
      return;
    }

    // CRITIQUE: Ne calculer qu'UNE SEULE FOIS au montage initial
    if (hasCalculatedSizeRef.current) {
      return;
    }

    // Attendre que le navigateur ait fini de calculer les styles CSS
    const rafId = requestAnimationFrame(() => {
      let width: number;
      let height: number;

      // Vue texte normale : caler la fenêtre EXACTEMENT sur le tableau en
      // mesurant le chrome réel (titre + bordures) dans le DOM — aucune
      // constante estimée, donc aucun espace résiduel à droite ni en bas
      const container = tableContainerRef.current;
      const windowEl = container?.closest('[data-map-address]') as HTMLElement | null;

      // Remettre l'échelle des cellules à 1 AVANT de mesurer : si l'utilisateur
      // avait rétréci les cellules (echelle < 1) puis a inversé l'affichage, le
      // tableau serait mesuré à cette échelle et la fenêtre se calerait trop
      // petit (espace résiduel au retour à l'orientation d'origine). On mesure
      // toujours la taille naturelle (échelle 1).
      if (container) {
        cellScaleRef.current = 1;
        applyCellVars(container, 1);
      }

      // Largeur nécessaire pour afficher ENTIÈREMENT le titre + les dimensions
      // dans la barre du haut à l'ouverture (les petites maps ne doivent pas
      // ouvrir avec un titre tronqué). scrollWidth donne la largeur pleine du
      // texte même si l'ellipse est active.
      const computeTitleNeededWidth = (): number => {
        const titleEl = windowEl?.querySelector('h3');
        if (!titleEl) return 0;
        const dimsEl = titleEl.nextElementSibling as HTMLElement | null;
        // Bouton d'inversion (sibling suivant, si présent) : sans lui les
        // petites maps ouvrent avec un titre tronqué de sa largeur.
        const invertBtn = dimsEl?.nextElementSibling as HTMLElement | null;
        const invertW = invertBtn?.offsetWidth ? invertBtn.offsetWidth + 8 : 0;
        // paddings du header (~26px) + zone bouton fermer (pr-8 = 32px) + gap + bordures
        return Math.ceil(titleEl.scrollWidth + (dimsEl?.offsetWidth || 0) + invertW + 72);
      };

      if (!easyViewMode && container && windowEl) {
        const tableRect = tableEl.getBoundingClientRect();
        const winRect = windowEl.getBoundingClientRect();
        const contRect = container.getBoundingClientRect();
        const chromeW = winRect.width - contRect.width;
        const chromeH = winRect.height - contRect.height;
        // Mémoriser la taille naturelle réelle du tableau (échelle 1) —
        // référence exacte pour tous les calculs d'échelle suivants
        naturalTableSizeRef.current = { w: tableRect.width, h: tableRect.height };
        width = Math.max(
          TEXT_WINDOW_MIN_WIDTH,
          computeTitleNeededWidth(),
          Math.round(tableRect.width + chromeW + 2)
        );
        height = Math.round(tableRect.height + chromeH + CELL_BOTTOM_GAP);
      } else if (easyViewMode && container && windowEl) {
        // EasyView : même largeur que la fenêtre normale (mesure DOM du
        // tableau + chrome), hauteur = tableau + panneau 3D en dessous
        const tableRect = tableEl.getBoundingClientRect();
        const winRect = windowEl.getBoundingClientRect();
        const contRect = container.getBoundingClientRect();
        const chromeW = winRect.width - contRect.width;
        // Référence exacte (échelle 1) pour la mise à l'échelle des cellules
        naturalTableSizeRef.current = { w: tableRect.width, h: tableRect.height };
        width = Math.max(
          TEXT_WINDOW_MIN_WIDTH,
          computeTitleNeededWidth(),
          Math.round(tableRect.width + chromeW + 2)
        );
        height = Math.round(tableRect.height * 2 + TEXT_VIEW_CHROME_HEIGHT * 2 + TEXT_VIEW_PADDING_HEIGHT_BOTTOM);
      } else {
        // Contexte sans fenêtre : estimation par constantes
        const horizontalPadding = (displayXAxisLabels.length >= 15 ? 2 : TEXT_VIEW_PADDING_WIDTH_BASE);
        const tableWidth = tableEl.scrollWidth;
        width = tableWidth + horizontalPadding;
        const realTableHeight = tableEl.offsetHeight;
        if (easyViewMode) {
          height = realTableHeight + TEXT_VIEW_CHROME_HEIGHT + realTableHeight + TEXT_VIEW_CHROME_HEIGHT + TEXT_VIEW_PADDING_HEIGHT_BOTTOM;
        } else {
          height = realTableHeight + TEXT_VIEW_CHROME_HEIGHT + TEXT_VIEW_PADDING_HEIGHT_BOTTOM;
        }
      }

      lastAutoSizeRef.current = { w: width, h: height };
      hasCalculatedSizeRef.current = true; // Bloquer tous les futurs calculs

      onAutoSizeRef.current?.(width, height);
    });

    return () => cancelAnimationFrame(rafId);
  }, [viewMode, displayXAxisLabels.length, displayYAxisLabels.length, easyViewMode, mapData.address]);
  
  // Mise à l'échelle des cellules selon la taille du conteneur (vue texte),
  // pour les changements de taille PROGRAMMATIQUES (ouverture, clamp workspace).
  // Le redimensionnement manuel est entièrement piloté par la poignée custom
  // (handleResizeHandleMouseDown) qui est gelée ici pour éviter tout conflit.
  // EasyView split (tableau + 3D) : l'échelle des cellules est pilotée par la
  // LARGEUR uniquement (la hauteur de la fenêtre est absorbée par le panneau 3D).
  // Mêmes conditions que le rendu split : 3D possible (plusieurs lignes) ou map 1 ligne
  const isEasyViewSplit = easyViewMode &&
    (displayMapValues.length > 1 || (displayMapValues.length === 1 && (displayMapValues[0]?.length || 0) > 1));

  useEffect(() => {
    const showsTable = viewMode === 'text' || easyViewMode;
    if (!showsTable) return;
    const container = tableContainerRef.current;
    if (!container) return;

    const cols = displayMapValues[0]?.length || 1;
    const rows = displayMapValues.length || 1;

    const compute = () => {
      // Attendre que la taille initiale de la fenêtre (échelle 1, collée au
      // tableau) ait été mesurée et appliquée avant d'activer la mise à l'échelle
      if (!hasCalculatedSizeRef.current) return;
      // Pendant un resize custom, la poignée applique déjà les variables
      if (isCustomResizingRef.current) return;

      const rect = container.getBoundingClientRect();
      if (rect.width < 20) return;
      if (!isEasyViewSplit && rect.height < 20) return;

      // Taille naturelle du tableau : la mesure DOM réelle à l'échelle 1
      // (exacte), avec repli sur l'estimation par constantes
      const naturalW = naturalTableSizeRef.current?.w ?? (50 + cols * 58);
      const naturalH = naturalTableSizeRef.current?.h ?? (22 + rows * 22);

      // Petite map (tableau plus étroit que la largeur mini de fenêtre) :
      // la fenêtre est libre, sa taille ne pilote PAS l'échelle des cellules —
      // seule la poignée de resize la modifie (échelle incrémentale)
      if (naturalW + 4 < TEXT_WINDOW_MIN_WIDTH) {
        applyCellVars(container, cellScaleRef.current);
        return;
      }

      // Échelle uniforme : format rectangulaire des cellules conservé.
      // EasyView split : largeur seule (le conteneur du tableau est en h-fit,
      // sa hauteur dépend du tableau — l'utiliser créerait une boucle).
      const ratio = isEasyViewSplit
        ? (rect.width - 2) / naturalW
        : Math.min((rect.width - 2) / naturalW, (rect.height - CELL_BOTTOM_GAP) / naturalH);
      // Ne jamais AGRANDIR les cellules au-delà de l'échelle posée par la
      // poignée de resize : l'espace excédentaire de la fenêtre (largeur mini
      // titre/boutons) ne doit pas gonfler les cellules à l'ouverture
      const upperCap = Math.max(1, cellScaleRef.current);
      const s = Math.min(upperCap, Math.min(CELL_MAX_SCALE, Math.max(CELL_MIN_SCALE, ratio)));
      cellScaleRef.current = s;
      applyCellVars(container, s);
    };

    const ro = new ResizeObserver(compute);
    ro.observe(container);
    compute();
    return () => ro.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [viewMode, easyViewMode, isEasyViewSplit, displayMapValues.length, displayMapValues[0]?.length, applyCellVars]);

  // Poignée de redimensionnement custom (remplace le resize natif CSS).
  // À CHAQUE mouvement de souris : calcul de l'échelle depuis la position du
  // curseur, application aux cellules, puis la fenêtre est calée EXACTEMENT
  // sur le tableau (+ espace boutons en bas). Résultat :
  //  - le bord de la fenêtre est collé au tableau en permanence (droite ET bas)
  //  - les boutons Text/2D/3D ne peuvent JAMAIS recouvrir le tableau
  //  - au minimum COMME au maximum de l'échelle, la fenêtre se bloque net
  //    au bord du tableau pendant le drag
  // La fenêtre est manipulée en style direct (pas de re-render React) = fluide.
  const handleResizeHandleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();

    const windowEl = (e.currentTarget as HTMLElement).closest('[data-map-address]') as HTMLElement | null;
    if (!windowEl) return;

    const winRect = windowEl.getBoundingClientRect();
    const startX = e.clientX;
    const startY = e.clientY;
    const originW = winRect.width;
    const originH = winRect.height;

    const isTextScaling = viewMode === 'text' && !easyViewMode && !!tableContainerRef.current && !!tableRef.current;
    const isEasyScaling = isEasyViewSplit && !!tableContainerRef.current && !!tableRef.current;
    let chromeW = 0;
    let chromeH = 0;
    let naturalW = 1;
    let naturalH = 1;
    if (isTextScaling || isEasyScaling) {
      const contRect = tableContainerRef.current!.getBoundingClientRect();
      chromeW = winRect.width - contRect.width;
      chromeH = winRect.height - contRect.height;
      const cols = displayMapValues[0]?.length || 1;
      const rows = displayMapValues.length || 1;
      // Taille naturelle réelle mesurée à l'échelle 1 (repli : estimation)
      naturalW = naturalTableSizeRef.current?.w ?? (50 + cols * 58);
      naturalH = naturalTableSizeRef.current?.h ?? (22 + rows * 22);
    }

    isCustomResizingRef.current = true;
    onResizeActiveChange?.(true);

    // Échelle INCRÉMENTALE : le facteur est calculé par rapport à la taille de
    // la fenêtre au début du drag (et non par rapport au tableau en absolu).
    // Toucher le coin sans bouger ne change donc RIEN (facteur = 1), et les
    // cellules évoluent proportionnellement au geste — même ressenti pour
    // toutes les maps, y compris les petites dont la fenêtre est plus large
    // que le tableau (largeur mini titre/boutons).
    const scaleAtDragStart = cellScaleRef.current;

    // Petite map : le tableau est plus étroit que la largeur minimale de la
    // fenêtre (titre/boutons), il ne peut donc pas piloter le bord de la
    // fenêtre. Dans ce cas la FENÊTRE suit la souris (planchers garantis) et
    // les cellules suivent le geste via l'échelle incrémentale.
    const isSmallMap = (isTextScaling || isEasyScaling) && (naturalW + chromeW + 2) < TEXT_WINDOW_MIN_WIDTH;

    // Plafond de largeur pour les petites maps : la fenêtre peut s'élargir
    // jusqu'à afficher l'en-tête en entier — titre ET description (une carte
    // 1x1 ou 2x2 a un tableau minuscule mais peut porter une description
    // longue, qui restait tronquée sans moyen d'élargir) — ou le tableau
    // avec cellules au max si c'est plus large. Au-delà, blocage net.
    let smallMapMaxW = Infinity;
    if (isSmallMap) {
      const titleEl = windowEl.querySelector('h3');
      const dimsEl = titleEl?.nextElementSibling as HTMLElement | null;
      // Bouton d'inversion (sibling suivant du span dimensions) compté aussi
      const invertBtn = dimsEl?.nextElementSibling as HTMLElement | null;
      const invertW = invertBtn?.offsetWidth ? invertBtn.offsetWidth + 8 : 0;
      const titleNeeded = titleEl
        ? Math.ceil(titleEl.scrollWidth + (dimsEl?.offsetWidth || 0) + invertW + 72)
        : 0;
      // Description : <p> tronqué juste après la ligne de titre. On ne la
      // compte QUE si elle déborde réellement (scrollWidth > clientWidth) —
      // sinon scrollWidth vaut la largeur actuelle de l'élément et le plafond
      // grandirait à chaque redimensionnement (effet cliquet).
      const descEl = titleEl?.parentElement?.nextElementSibling as HTMLElement | null;
      const descNeeded =
        descEl && descEl.tagName === 'P' && descEl.scrollWidth > descEl.clientWidth + 1
          ? Math.ceil(windowEl.offsetWidth + (descEl.scrollWidth - descEl.clientWidth) + 8)
          : 0;
      smallMapMaxW = Math.max(
        TEXT_WINDOW_MIN_WIDTH,
        titleNeeded,
        descNeeded,
        Math.round(naturalW * CELL_MAX_SCALE + chromeW + 2)
      );
    }

    const onMove = (ev: MouseEvent) => {
      const rawW = originW + (ev.clientX - startX);
      const rawH = originH + (ev.clientY - startY);

      if (!isTextScaling && !isEasyScaling) {
        // Modes 2D/3D : redimensionnement libre avec minimums génériques
        windowEl.style.width = `${Math.max(240, Math.round(rawW))}px`;
        windowEl.style.height = `${Math.max(180, Math.round(rawH))}px`;
        return;
      }

      const container = tableContainerRef.current!;
      const tableEl = tableRef.current!;

      if (isEasyScaling) {
        // EasyView split : échelle pilotée par la largeur, collée au tableau ;
        // hauteur libre (le panneau 3D absorbe l'espace vertical)
        const factor = rawW / Math.max(1, originW);
        const s = Math.min(CELL_MAX_SCALE, Math.max(CELL_MIN_SCALE, scaleAtDragStart * factor));
        cellScaleRef.current = s;
        applyCellVars(container, s);
        const tableRect = tableEl.getBoundingClientRect();
        // Petite map : la fenêtre suit la souris (plafond = cellules au max
        // + titre complet) ; sinon collée au tableau
        const w = isSmallMap
          ? Math.min(smallMapMaxW, Math.max(TEXT_WINDOW_MIN_WIDTH, Math.round(rawW)))
          : Math.max(TEXT_WINDOW_MIN_WIDTH, Math.round(tableRect.width + chromeW + 2));
        // Hauteur libre, avec un plancher = tableau + un minimum pour le panneau 3D
        const h = Math.max(Math.round(tableRect.height) + 160, Math.round(rawH));
        windowEl.style.width = `${w}px`;
        windowEl.style.height = `${h}px`;
        return;
      }

      // Échelle des cellules :
      //  - map normale : facteur relatif au début du drag, limité par l'axe
      //    le plus contraignant (agrandir demande un geste en diagonale)
      //  - petite map : pilotée par la HAUTEUR en absolu (la fenêtre est
      //    collée au tableau en hauteur, donc pas de saut au contact) ;
      //    la largeur, elle, gère l'affichage du titre
      let s: number;
      if (isSmallMap) {
        s = Math.min(
          CELL_MAX_SCALE,
          Math.max(CELL_MIN_SCALE, (rawH - chromeH - CELL_BOTTOM_GAP) / naturalH)
        );
      } else {
        const factor = Math.min(rawW / Math.max(1, originW), rawH / Math.max(1, originH));
        s = Math.min(CELL_MAX_SCALE, Math.max(CELL_MIN_SCALE, scaleAtDragStart * factor));
      }
      cellScaleRef.current = s;
      applyCellVars(container, s);

      // Mesurer le tableau réel après application (reflow synchrone) puis
      // caler la fenêtre EXACTEMENT dessus. Le bord reste collé au tableau
      // en permanence : au minimum COMME au maximum de l'échelle, la fenêtre
      // se bloque net au bord du tableau.
      const tableRect = tableEl.getBoundingClientRect();
      const hugH = Math.round(tableRect.height + chromeH + CELL_BOTTOM_GAP);

      let w: number;
      let h: number;
      if (isSmallMap) {
        // Petite map — règle d'agrandissement en deux temps :
        //  1) la hauteur (collée au tableau) fait grossir les cellules
        //     jusqu'à leur taille maximum, puis se bloque
        //  2) la largeur peut continuer jusqu'à afficher le titre en entier,
        //     puis se bloque — au-delà, plus aucun agrandissement possible
        w = Math.min(smallMapMaxW, Math.max(TEXT_WINDOW_MIN_WIDTH, Math.round(rawW)));
        h = hugH;
      } else {
        w = Math.max(TEXT_WINDOW_MIN_WIDTH, Math.round(tableRect.width + chromeW + 2));
        h = hugH;
      }

      windowEl.style.width = `${w}px`;
      windowEl.style.height = `${h}px`;
    };

    const onUp = () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
      isCustomResizingRef.current = false;
      onResizeActiveChange?.(false);
      // Committer la taille finale dans le state React du parent
      const finalRect = windowEl.getBoundingClientRect();
      onAutoSizeRef.current?.(Math.round(finalRect.width), Math.round(finalRect.height));
    };

    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [viewMode, easyViewMode, displayMapValues.length, displayMapValues[0]?.length, applyCellVars]);

  // Position de la caméra pour cette map
  // Si déjà sauvegardée, on la restaure. Sinon, position par défaut (coin bas-gauche)
  // Calculé à chaque rendu pour toujours avoir la dernière position sauvegardée
  const getCameraPosition = () => {
    const mapKey = mapData.address;
    // Si position déjà sauvegardée, la retourner
    if (savedCameraPositions[mapKey]) {
      return savedCameraPositions[mapKey];
    }
    // Sinon, position par défaut : le coin BAS-GAUCHE du tableau face à la
    // caméra, quelle que soit la famille. Plotly trace toujours ses axes en
    // croissant ; selon le sens d'affichage des labels (EDC15P : Y descendant
    // vers le bas, EDC16/EDC15VM : axes croissants), le coin bas-gauche du
    // tableau n'est donc pas toujours au même endroit dans la scène — on
    // choisit le signe de l'œil en conséquence.
    const xs = displayXAxisLabels.map((l: string) => parseFloat(l));
    const ys = displayYAxisLabels.map((l: string) => parseFloat(l));
    const xAscending = xs.length < 2 || xs[0] <= xs[xs.length - 1];
    const bottomIsMin = ys.length < 2 || ys[ys.length - 1] <= ys[0];
    const defaultPosition = {
      // Caméra rapprochée (distance ~1.6 vs 2.74 d'origine) : même angle,
      // la surface remplit le canvas au lieu de flotter petite au centre
      // avec une grande bande vide au-dessus (EasyView/3D/preview).
      eye: { x: xAscending ? -1.05 : 1.05, y: bottomIsMin ? -1.05 : 1.05, z: 0.6 },
      // center.z négatif : la caméra vise sous le centre de la scène, ce qui
      // REMONTE la surface dans le cadre et résorbe la bande vide du haut
      center: { x: 0, y: 0, z: -0.15 },
      up: { x: 0, y: 0, z: 1 }
    };
    // Sauvegarder la position par défaut
    savedCameraPositions[mapKey] = defaultPosition;
    return defaultPosition;
  };

  // Calculer la position initiale de la caméra à chaque render
  // Si sauvegardée, on la restaure, sinon position par défaut
  const cameraPosition = getCameraPosition();

  // Sauvegarder la position de la caméra quand l'utilisateur interagit avec le graphique 3D
  const handlePlotlyRelayout = (event: any) => {
    if (event["scene.camera"]) {
      const camera = event["scene.camera"];
      // Sauvegarder la position complète de la caméra dans l'objet global
      const savedPosition = {
        eye: camera.eye || { x: -1.05, y: -1.05, z: 0.6 },
        center: camera.center || { x: 0, y: 0, z: 0 },
        up: camera.up || { x: 0, y: 0, z: 1 }
      };
      savedCameraPositions[mapData.address] = savedPosition;
    }
  };

  // Zoom programmatique (boutons +/− de l'EasyView) : on rapproche ou on
  // éloigne l'œil du centre de la scène, à partir de la DERNIÈRE position
  // utilisateur (savedCameraPositions, mise à jour à chaque rotation). Le
  // bump de révision force Plotly à appliquer la nouvelle caméra malgré
  // uirevision (qui fige sinon l'état UI).
  const [cameraZoomRev, setCameraZoomRev] = useState(0);
  const zoomCamera = (factor: number) => {
    const key = mapData.address;
    const cam = savedCameraPositions[key] || cameraPosition;
    const c = cam.center || { x: 0, y: 0, z: 0 };
    const eye = {
      x: c.x + (cam.eye.x - c.x) * factor,
      y: c.y + (cam.eye.y - c.y) * factor,
      z: c.z + (cam.eye.z - c.z) * factor,
    };
    // Bornes : ne pas traverser la surface ni partir à l'infini
    const dist = Math.hypot(eye.x - c.x, eye.y - c.y, eye.z - c.z);
    if (dist < 0.35 || dist > 8) return;
    savedCameraPositions[key] = { ...cam, eye };
    setCameraZoomRev((r) => r + 1);
  };

  // Wrapper pour onClose qui nettoie aussi la position sauvegardée
  const handleClose = () => {
    // Supprimer la position sauvegardée pour cette map
    delete savedCameraPositions[mapData.address];
    // Appeler le callback du parent
    onClose?.();
  };

  // Inversion de l'affichage (transpose lignes/colonnes). Le champ persisté
  // invertDisplay BASCULE le défaut de la map : true = inversé, false = défaut.
  // On alterne true<->false (jamais undefined) pour un toggle prévisible.
  const invertDisplayOn = displaySettings?.map?.invertDisplay === true;
  // Maps dont l'affichage PAR DÉFAUT est transposé par rapport au fichier
  // (l'orientation écrite dans la liste des maps) : le bouton est présenté
  // ACTIF par défaut — le désactiver ramène à l'orientation fichier.
  // Ex. Drivers wish MJD6 : fichier 16x8, affiché 8x16 par défaut.
  // Étendre cette liste quand d'autres maps recevront un défaut transposé.
  const DEFAULT_SWAPPED_DISPLAY_PATTERNS = ["drivers wish"];
  const hasDefaultSwappedDisplay = DEFAULT_SWAPPED_DISPLAY_PATTERNS.some(p =>
    mapNameLowerDisplay.includes(p)
  );
  // État VISUEL du bouton : "la vue est transposée par rapport au fichier"
  // (XOR entre le défaut transposé et l'override utilisateur). Le clic
  // continue de basculer invertDisplay — seule la présentation change.
  const invertButtonActive = hasDefaultSwappedDisplay !== invertDisplayOn;
  const handleToggleInvert = () => {
    onToggleInvertDisplay?.(mapData.address, !invertDisplayOn);
  };

  const getCellKey = (row: number, col: number) => `${row}-${col}`;

  // Gestionnaire pour la fin du drag
  const handleMouseUp = useCallback(() => {
    // Si Ctrl était enfoncé et qu'on n'a pas bougé, c'est un simple clic pour toggle
    if (isCtrlDragging && !hasMovedDuringDrag && dragStart) {
      const cellKey = getCellKey(dragStart.row, dragStart.col);
      setSelectedCells(prev => {
        const next = new Set(prev);
        if (next.has(cellKey)) {
          next.delete(cellKey);
        } else {
          next.add(cellKey);
        }
        return next;
      });
    }

    setIsDragging(false);
    setDragStart(null);
    setIsCtrlDragging(false);
    setInitialSelection(new Set());
    setHasMovedDuringDrag(false);

    // Arrêter le drag des axes
    setIsAxisDragging(false);
    setAxisDragStart(null);
  }, [isCtrlDragging, hasMovedDuringDrag, dragStart]);

  // Pr├®charger Plotly d├¿s le montage du composant pour ├®liminer la latence
  useEffect(() => {
    // Pr├®charger le module Plotly imm├®diatement
    import("react-plotly.js").then(() => {
      setPlotlyReady(true);
    });
  }, []);

  // Gestionnaire global pour mouseup (fin du drag)
  useEffect(() => {
    const handleGlobalMouseUp = () => {
      // Clear any existing timeout
      if (dragEndTimeoutRef.current) {
        clearTimeout(dragEndTimeoutRef.current);
      }
      // Set a timeout to re-enable auto-sizing after drag ends (50ms delay)
      // Short delay prevents auto-resize glitch while allowing quick interaction with 3D plot
      dragEndTimeoutRef.current = setTimeout(() => {
        setIsDraggingWindow(false);
      }, 50);

      handleMouseUp();
    };

    window.addEventListener('mouseup', handleGlobalMouseUp);
    return () => {
      window.removeEventListener('mouseup', handleGlobalMouseUp);
      if (dragEndTimeoutRef.current) {
        clearTimeout(dragEndTimeoutRef.current);
      }
    };
  }, [handleMouseUp]);

  // Ref pour stocker les dernières infos de sélection envoyées (éviter les boucles infinies)
  const lastSelectionInfoRef = useRef<string>('');

  // Notifier le parent des changements de sélection (pour le curseur global)
  useEffect(() => {
    if (!onSelectionChange) return;

    // Utiliser mapValues au lieu de displayMapValues pour éviter les re-renders
    const rows = mapValues.length;
    const cols = mapValues[0]?.length || 0;
    const dimensions = `${cols}x${rows}`;

    // Calculer les adresses et valeurs des cellules sélectionnées
    const selectedCellsInfo: Array<{ row: number; col: number; address: number; value: number }> = [];

    if (selectedCells.size > 0) {
      // Taille d'une cellule en bytes: calculé depuis la taille totale / nombre de cellules
      const totalCells = rows * cols;
      const cellSize = totalCells > 0 ? Math.max(1, Math.floor(mapData.size / totalCells)) : 2;
      const mapStartAddress = mapData.address;

      selectedCells.forEach(cellKey => {
        const [rowStr, colStr] = cellKey.split('-');
        const row = parseInt(rowStr, 10);
        const col = parseInt(colStr, 10);
        // Calcul de l'adresse: base + (row * cols + col) * cellSize
        const cellOffset = (row * cols + col) * cellSize;
        const cellAddress = mapStartAddress + cellOffset;
        selectedCellsInfo.push({ row, col, address: cellAddress, value: mapValues[row]?.[col] ?? 0 });
      });
    }

    // Créer une clé unique pour comparer avec la dernière valeur envoyée
    // (inclut la somme des valeurs pour rafraîchir la barre d'état après une édition)
    const valuesSignature = selectedCellsInfo.reduce((acc, c) => acc + c.value, 0);
    const infoKey = `${mapData.name}-${mapData.address}-${dimensions}-${selectedCells.size}-${Array.from(selectedCells).sort().join(',')}-${valuesSignature}`;

    // Ne notifier que si quelque chose a vraiment changé
    if (infoKey !== lastSelectionInfoRef.current) {
      lastSelectionInfoRef.current = infoKey;
      onSelectionChange({
        mapName: mapData.name,
        mapAddress: mapData.address,
        dimensions,
        selectedCount: selectedCells.size,
        selectedCells: selectedCellsInfo,
      });
    }
  }, [selectedCells, mapData.name, mapData.address, mapData.size, mapValues, onSelectionChange]);

  // Gestionnaires de raccourcis clavier pour + et -
  // Ne réagir que si cette map est active (au premier plan)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Only handle keyboard shortcuts if this map is the active one
      if (!isActive) return;

      // Don't intercept keys when user is typing in an input field
      const target = e.target as HTMLElement;
      if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT') return;

      const hasSelection = selectedCells.size > 0 || selectedXAxisCells.size > 0 || selectedYAxisCells.size > 0;

      // ── Navigation au clavier : flèches = déplacer la cellule active,
      //    Ctrl+flèche = étendre la sélection (comme le Ctrl+clic) ──
      const ARROWS: Record<string, [number, number]> = {
        ArrowUp: [-1, 0], ArrowDown: [1, 0], ArrowLeft: [0, -1], ArrowRight: [0, 1],
      };
      if (e.key in ARROWS && !e.altKey) {
        const totalRows = displayMapValues.length;
        const totalCols = displayMapValues[0]?.length ?? 0;
        if (totalRows === 0 || totalCols === 0) return;
        let cur = keyboardCursorRef.current;
        if (!cur) {
          // Pas de curseur (sélection faite autrement) : partir de la
          // première cellule sélectionnée
          const first = Array.from(selectedCells)[0];
          if (!first) return;
          const [mr, mc] = first.split('-').map(Number);
          cur = toDisplayCoords(mr, mc);
        }
        const [dr, dc] = ARROWS[e.key];
        const next = {
          row: Math.min(totalRows - 1, Math.max(0, cur.row + dr)),
          col: Math.min(totalCols - 1, Math.max(0, cur.col + dc)),
        };
        e.preventDefault();
        keyboardCursorRef.current = next;
        const { row, col } = toMapCoords(next.row, next.col);
        const key = getCellKey(row, col);
        setSelectedXAxisCells(new Set());
        setSelectedYAxisCells(new Set());
        if (e.ctrlKey || e.metaKey) {
          setSelectedCells(prev => new Set(prev).add(key));
        } else {
          setSelectedCells(new Set([key]));
        }
        return;
      }

      if (!hasSelection) return;

      // ── Ctrl+C / Ctrl+V : mêmes règles que Copier / Coller du menu ──
      if ((e.ctrlKey || e.metaKey) && (e.key === 'c' || e.key === 'C')) {
        e.preventDefault();
        if (selectedCells.size > 0) {
          const cellsArray = Array.from(selectedCells).map(k => {
            const [row, col] = k.split('-').map(Number);
            return { row, col };
          });
          const minRow = Math.min(...cellsArray.map(c => c.row));
          const maxRow = Math.max(...cellsArray.map(c => c.row));
          const minCol = Math.min(...cellsArray.map(c => c.col));
          const maxCol = Math.max(...cellsArray.map(c => c.col));
          const values: string[][] = [];
          for (let r = minRow; r <= maxRow; r++) {
            const rowValues: string[] = [];
            for (let c = minCol; c <= maxCol; c++) {
              const v = selectedCells.has(getCellKey(r, c)) ? mapValues[r]?.[c] : undefined;
              rowValues.push(v !== undefined ? v.toString() : '');
            }
            values.push(rowValues);
          }
          writeClipboard({ values, type: 'cell', rows: maxRow - minRow + 1, cols: maxCol - minCol + 1 });
          toast({ title: t.mapViewer.copy, description: `${cellsArray.length} value(s) copied` });
        } else if (selectedXAxisCells.size > 0) {
          const indices = Array.from(selectedXAxisCells).sort((a, b) => a - b);
          const values = indices.map(index => [readSourceAxis(displayXAxisToSource(index)) || '']);
          writeClipboard({ values, type: 'xAxis', rows: indices.length, cols: 1 });
          toast({ title: t.mapViewer.copy, description: `${indices.length} value(s) copied` });
        } else if (selectedYAxisCells.size > 0) {
          const indices = Array.from(selectedYAxisCells).sort((a, b) => a - b);
          const values = indices.map(index => [readSourceAxis(displayYAxisToSource(index)) || '']);
          writeClipboard({ values, type: 'yAxis', rows: indices.length, cols: 1 });
          toast({ title: t.mapViewer.copy, description: `${indices.length} value(s) copied` });
        }
        return;
      }
      if ((e.ctrlKey || e.metaKey) && (e.key === 'v' || e.key === 'V')) {
        e.preventDefault();
        const clip = readClipboard();
        if (!clip) return;
        let pastedCount = 0;
        if (selectedCells.size > 0) {
          const cellsArray = Array.from(selectedCells).map(k => {
            const [row, col] = k.split('-').map(Number);
            return { row, col };
          });
          const startRow = Math.min(...cellsArray.map(c => c.row));
          const startCol = Math.min(...cellsArray.map(c => c.col));
          clip.values.forEach((rowValues, rowOffset) => {
            rowValues.forEach((value, colOffset) => {
              if (value === '') return;
              const targetRow = startRow + rowOffset;
              const targetCol = startCol + colOffset;
              if (targetRow < mapValues.length && targetCol < (mapValues[0]?.length || 0)) {
                const parsed = Number(value.replace(',', '.'));
                if (!Number.isNaN(parsed)) {
                  updateCellValue(targetRow, targetCol, parsed);
                  pastedCount++;
                }
              }
            });
          });
        } else if (selectedXAxisCells.size > 0) {
          const startIndex = Array.from(selectedXAxisCells).sort((a, b) => a - b)[0] ?? 0;
          clip.values.forEach((rowValues, offset) => {
            const value = rowValues[0];
            const displayIdx = startIndex + offset;
            if (value && displayIdx < displayXAxisLabels.length) {
              mutateDisplayXAxis(displayIdx, () => value);
              pastedCount++;
            }
          });
        } else if (selectedYAxisCells.size > 0) {
          const startIndex = Array.from(selectedYAxisCells).sort((a, b) => a - b)[0] ?? 0;
          clip.values.forEach((rowValues, offset) => {
            const value = rowValues[0];
            const displayIdx = startIndex + offset;
            if (value && displayIdx < displayYAxisLabels.length) {
              mutateDisplayYAxis(displayIdx, () => value);
              pastedCount++;
            }
          });
        }
        toast({ title: t.mapViewer.paste, description: `${pastedCount} value(s) pasted` });
        return;
      }

      if (e.key === '+' || e.key === '=' || e.key === 'Add') {
        e.preventDefault();

        // Modifier les cellules de données sélectionnées
        if (selectedCells.size > 0) {
          setMapValues((prevMapValues) => {
            const next = prevMapValues.map(row => [...row]);
            selectedCells.forEach(cellKey => {
              const [rowIdx, colIdx] = cellKey.split('-').map(Number);
              if (next[rowIdx]?.[colIdx] !== undefined) {
                next[rowIdx][colIdx] += incrementValue;
              }
            });
            setChangedCells((prevChangedCells) => {
              const nextChanged = { ...prevChangedCells };
              selectedCells.forEach(cellKey => {
                const [rowIdx, colIdx] = cellKey.split('-').map(Number);
                const original = originalValuesRef.current?.[rowIdx]?.[colIdx];
                if (original !== undefined && next[rowIdx]?.[colIdx] !== undefined) {
                  const newValue = next[rowIdx][colIdx];
                  if (Math.abs(original - newValue) < 1e-6) {
                    delete nextChanged[cellKey];
                  } else {
                    nextChanged[cellKey] = newValue;
                  }
                }
              });
              return nextChanged;
            });
            return next;
          });
        }

        // Axes X/Y : traduire les index AFFICHÉS vers l'axe source (transpose+mirror)
        if (selectedXAxisCells.size > 0) {
          selectedXAxisCells.forEach(idx => {
            mutateDisplayXAxis(idx, cur => {
              const v = parseFloat(cur);
              return isNaN(v) ? cur : String(v + incrementValue);
            });
          });
        }
        if (selectedYAxisCells.size > 0) {
          selectedYAxisCells.forEach(idx => {
            mutateDisplayYAxis(idx, cur => {
              const v = parseFloat(cur);
              return isNaN(v) ? cur : String(v + incrementValue);
            });
          });
        }
      } else if (e.key === '-' || e.key === '_' || e.key === 'Subtract') {
        e.preventDefault();

        // Modifier les cellules de données sélectionnées
        if (selectedCells.size > 0) {
          setMapValues((prevMapValues) => {
            const next = prevMapValues.map(row => [...row]);
            selectedCells.forEach(cellKey => {
              const [rowIdx, colIdx] = cellKey.split('-').map(Number);
              if (next[rowIdx]?.[colIdx] !== undefined) {
                next[rowIdx][colIdx] -= incrementValue;
              }
            });
            setChangedCells((prevChangedCells) => {
              const nextChanged = { ...prevChangedCells };
              selectedCells.forEach(cellKey => {
                const [rowIdx, colIdx] = cellKey.split('-').map(Number);
                const original = originalValuesRef.current?.[rowIdx]?.[colIdx];
                if (original !== undefined && next[rowIdx]?.[colIdx] !== undefined) {
                  const newValue = next[rowIdx][colIdx];
                  if (Math.abs(original - newValue) < 1e-6) {
                    delete nextChanged[cellKey];
                  } else {
                    nextChanged[cellKey] = newValue;
                  }
                }
              });
              return nextChanged;
            });
            return next;
          });
        }

        // Axes X/Y : traduire les index AFFICHÉS vers l'axe source (transpose+mirror)
        if (selectedXAxisCells.size > 0) {
          selectedXAxisCells.forEach(idx => {
            mutateDisplayXAxis(idx, cur => {
              const v = parseFloat(cur);
              return isNaN(v) ? cur : String(v - incrementValue);
            });
          });
        }
        if (selectedYAxisCells.size > 0) {
          selectedYAxisCells.forEach(idx => {
            mutateDisplayYAxis(idx, cur => {
              const v = parseFloat(cur);
              return isNaN(v) ? cur : String(v - incrementValue);
            });
          });
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
    // displayTransposed/flips inclus : le handler doit voir l'orientation
    // courante pour écrire au bon axe source après un toggle d'inversion.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedCells, selectedXAxisCells, selectedYAxisCells, incrementValue, isActive, displayTransposed, displayColsFlipped, displayRowsFlipped, displayMapValues, mapValues, displayXAxisLabels, displayYAxisLabels]);

  // Handle modifyCommand from toolbar (Zap button)
  useEffect(() => {
    if (!modifyCommand) return;

    const hasSelection = selectedCells.size > 0 || selectedXAxisCells.size > 0 || selectedYAxisCells.size > 0;
    if (!hasSelection) return;

    const { operation, value } = modifyCommand;

    // Modify selected data cells
    if (selectedCells.size > 0) {
      setMapValues((prevMapValues) => {
        const next = prevMapValues.map(row => [...row]);
        selectedCells.forEach(cellKey => {
          const [rowIdx, colIdx] = cellKey.split('-').map(Number);
          if (next[rowIdx]?.[colIdx] !== undefined) {
            if (operation === 'add') {
              next[rowIdx][colIdx] += value;
            } else {
              // fill
              next[rowIdx][colIdx] = value;
            }
          }
        });
        setChangedCells((prevChangedCells) => {
          const nextChanged = { ...prevChangedCells };
          selectedCells.forEach(cellKey => {
            const [rowIdx, colIdx] = cellKey.split('-').map(Number);
            const original = originalValuesRef.current?.[rowIdx]?.[colIdx];
            if (original !== undefined && next[rowIdx]?.[colIdx] !== undefined) {
              const newValue = next[rowIdx][colIdx];
              if (Math.abs(original - newValue) < 1e-6) {
                delete nextChanged[cellKey];
              } else {
                nextChanged[cellKey] = newValue;
              }
            }
          });
          return nextChanged;
        });
        return next;
      });
    }

    // Axes X/Y : index AFFICHÉS -> axe source (transpose+mirror)
    if (selectedXAxisCells.size > 0) {
      selectedXAxisCells.forEach(idx => {
        mutateDisplayXAxis(idx, cur => {
          const v = parseFloat(cur);
          if (isNaN(v)) return cur;
          return operation === 'add' ? String(v + value) : String(value);
        });
      });
    }
    if (selectedYAxisCells.size > 0) {
      selectedYAxisCells.forEach(idx => {
        mutateDisplayYAxis(idx, cur => {
          const v = parseFloat(cur);
          if (isNaN(v)) return cur;
          return operation === 'add' ? String(v + value) : String(value);
        });
      });
    }
  }, [modifyCommand, displayTransposed, displayColsFlipped, displayRowsFlipped]);

  // If controlled, sync internal state with controlled value
  useEffect(() => {
    if (controlledViewMode !== undefined && controlledViewMode !== internalViewMode) {
      setInternalViewMode(controlledViewMode);
    }
  }, [controlledViewMode, internalViewMode]);
  
  
  // Extract axis units from description or use x_label/y_label fields if available
  // Format: "Z | X: X_desc (X_units) | Y: Y_desc (Y_units) | Axis IDs: X=0x... Y=0x..."
  const parseAxisUnits = () => {
    let xUnit = "degC";
    let yUnit = "rpm";
    let xLabel = "Load";
    let yLabel = "mbar";

    // Helper function to extract just the unit from a label like "Engine speed (rpm)" -> "rpm"
    const extractUnit = (label: string): string => {
      const match = label.match(/\(([^)]+)\)/);
      return match ? match[1].trim() : label;
    };

    // Priority 1: Use x_label/y_label fields if provided by backend
    // Empty string is a valid override (e.g., for 1xN maps that don't have X axis)
    if (mapData.x_label !== undefined && mapData.x_label !== null) {
      xLabel = mapData.x_label;
      xUnit = extractUnit(mapData.x_label);
    }
    if (mapData.y_label !== undefined && mapData.y_label !== null) {
      yLabel = mapData.y_label;
      yUnit = extractUnit(mapData.y_label);
    }

    // Priority 2: Fall back to parsing description
    if (mapData.description && (!mapData.x_label || !mapData.y_label)) {
      // Try to extract from description: "X: X_desc (X_units) | Y: Y_desc (Y_units)"
      const xMatch = mapData.description.match(/X:\s*([^(]+)\s*\(([^)]+)\)/);
      const yMatch = mapData.description.match(/Y:\s*([^(]+)\s*\(([^)]+)\)/);

      if (!mapData.x_label && xMatch && xMatch.length >= 3) {
        xLabel = xMatch[1].trim();
        xUnit = xMatch[2].trim();
      }
      if (!mapData.y_label && yMatch && yMatch.length >= 3) {
        yLabel = yMatch[1].trim();
        yUnit = yMatch[2].trim();
      }
    }

    const mapNameLower = (mapData.name || "").toLowerCase();
    const looksBoostTarget = mapNameLower.includes("boost target map");
    // For boost target we rely on swap logic; don't swap labels here to avoid double inversions

    // Le corner (unités/description) doit refléter le swap NET effectif des
    // axes d'affichage par rapport aux x_label/y_label bruts du détecteur.
    // Deux sources de swap se composent (XOR) :
    //  - le swap d'affichage par défaut des Drivers wish : stockées [pedal][rpm]
    //    et montrées par défaut RPM en lignes / pedal en colonnes, donc le
    //    corner doit lui aussi être inversé (RPM à gauche, % en haut) ;
    //  - l'inversion utilisateur (displayTransposed) qui re-transpose la vue,
    //    valable pour TOUTE map (le libellé suit les lignes/colonnes).
    // On restreint le swap par défaut aux Drivers wish (seul cas audité) pour
    // ne pas modifier le rendu des autres maps swappées (torque limiter, n75).
    const defaultCornerSwap = mapNameLower.includes("drivers wish");
    const cornerSwap = defaultCornerSwap !== displayTransposed;
    if (cornerSwap) {
      return { xUnit: yUnit, yUnit: xUnit, xLabel: yLabel, yLabel: xLabel };
    }

    return { xUnit, yUnit, xLabel, yLabel };
  };

  // Determine if axes should be swapped based on map name and description
  // Some maps like "IQ by MAP limiter" have axes swapped in the file
  const shouldSwapAxes = (mapData: MapViewerProps['mapData']): boolean => {
    const mapName = (mapData.name || "").toLowerCase();
    const description = (mapData.description || "").toLowerCase();
    
    // Maps that typically have axes swapped (Y in columns, X in rows)
    // Based on EDCsuite JSON: "IQ by MAP limiter" has X: "Airflow mg/stroke", Y: "Engine speed (rpm)"
    // But in the file, these are stored swapped
    const swappedMapPatterns: string[] = [];
    
    // Check if map name matches patterns that typically have swapped axes
    for (const pattern of swappedMapPatterns) {
      if (mapName.includes(pattern)) {
        return true;
      }
    }

    // Marelli MJD6 "Drivers wish": stored [pedal][rpm] (rows=8 pedal %, cols=16
    // RPM), the transpose of the EDC16 layout. Swap for display so it reads like
    // the EDC16 driver-wish maps — RPM down the left (rows), pedal % across the
    // top (cols). The transpose branch reads file[pedal][rpm] correctly.
    if (mapName.includes("drivers wish")) {
      return true;
    }

    
    // Special case: Torque limiter should be horizontal (wide map)
    // Expected: 21 columns (mbar) x 3 rows (rpm) - HORIZONTAL
    // Pattern: X axis = Engine speed (rpm) with 3 values, Y axis = Atm. pressure (mbar) with 21 values
    // If backend sends vertical orientation (21 rows x 3 cols), swap needed
    if (mapName.toLowerCase().includes("torque limiter")) {
      const apiRows = mapData.dimensions?.TwoDimensional?.rows || 0;
      const apiCols = mapData.dimensions?.TwoDimensional?.cols || 0;
      
      
      // Torque limiter should be horizontal (wide), not vertical (tall)
      // Expected display: 3 rows (RPM) x 21 cols (mbar) - HORIZONTAL
      // Backend might send: 21 rows x 3 cols - VERTICAL
      // But description says: X = Engine speed (rpm) with 3 values, Y = Atm. pressure (mbar) with 21 values
      // So backend dimensions 21x3 mean: 21 rows (Y=mbar), 3 cols (X=RPM)
      // To display horizontal: we need to swap dimensions to 3x21, and also swap which axis is which
      if (apiRows > apiCols && apiRows >= 15 && apiCols <= 5) {
        return true;
      }
    }
    
    // Special case: N75 duty cycle has dimensions potentially inverted
    // Expected: 16 rows (RPM) x 13 cols (IQ/mg/st) - should be 16x13
    // Pattern might send 13 rows x 16 cols, need to check and swap if needed
    if (mapName.toLowerCase().includes("n75")) {
      const apiRows = mapData.dimensions?.TwoDimensional?.rows || 0;
      const apiCols = mapData.dimensions?.TwoDimensional?.cols || 0;
      const xAxisDesc = description.match(/x:\s*([^(]+)/)?.[1]?.toLowerCase() || "";
      const yAxisDesc = description.match(/y:\s*([^(]+)/)?.[1]?.toLowerCase() || "";
      
      const xIsIQ = xAxisDesc.includes("iq") || xAxisDesc.includes("mg/st");
      const yIsRpm = yAxisDesc.includes("rpm") || yAxisDesc.includes("engine speed");
      
      // N75 should have: Y (RPM) = 16 values, X (IQ) = 13 values
      // If backend sends 13 rows and 16 cols, and description says X is IQ and Y is RPM,
      // then dimensions are likely inverted
      if (xIsIQ && yIsRpm && apiRows === 13 && apiCols === 16) {
        return true;
      }
    }
    
    // REMOVED: Generic axis swap detection - it was too aggressive and broke many maps
    // Only apply swaps for specific, well-identified cases above
    // If a map needs axis swapping, it should be explicitly listed in the patterns above
    
    return false;
  };

  // Utiliser useMemo pour m├®moriser les donn├®es extraites et ├®viter les recalculs
  const extractedData = useMemo(() => {
    // V├®rifier le cache d'abord
    const cacheKey = getCacheKey(mapData.address, projectName, fileName);
    const fileDataHash = getFileDataHash(fileData, mapData.address);
    const cached = mapDataCache.get(cacheKey);

    // Get current dimensions from mapData (handle both 2D and 1D maps)
    let currentApiRows: number;
    let currentApiCols: number;
    if (mapData.dimensions?.TwoDimensional) {
      currentApiRows = mapData.dimensions.TwoDimensional.rows;
      currentApiCols = mapData.dimensions.TwoDimensional.cols;
    } else if (mapData.dimensions?.OneDimensional) {
      currentApiRows = 1;
      currentApiCols = mapData.dimensions.OneDimensional.length;
    } else {
      currentApiRows = 1;
      currentApiCols = mapData.size / 2;
    }

    // Debug: Log hash comparison

    // Si les donn├®es sont en cache et que fileData n'a pas chang├®, utiliser le cache
    // CRITICAL: Also check dimensions match to invalidate cache when dimensions change (e.g., after applying solution)
    // AND the user's invert-display override (le cache stocke axesSwapped, qui
    // dépend de invertDisplay ; sans ça le toggle servirait une vue périmée).
    const cacheInvert = displaySettings?.map?.invertDisplay ?? null;
    if (cached && cached.fileDataHash === fileDataHash && cached.mapAddress === mapData.address && cached.version === CACHE_VERSION
        && cached.apiRows === currentApiRows && cached.apiCols === currentApiCols
        && cached.invert === cacheInvert) {
      return {
        mapValues: cached.mapValues,
        xAxisLabels: cached.xAxisLabels,
        yAxisLabels: cached.yAxisLabels,
        axesSwapped: cached.axesSwapped,
        rowsReversed: cached.rowsReversed,
        colsReversed: cached.colsReversed,
      };
    }


    const mapNameLower = (mapData.name || "").toLowerCase();
    const isInjectorDuration = mapNameLower.includes("injector duration") && !mapNameLower.includes("selector");
    const isInjectorDuration00 = isInjectorDuration && mapNameLower.includes("duration 00");
    const isInjectorDurationNon00 = isInjectorDuration && !isInjectorDuration00;
    // EDC16U34 names these maps "Duration NN" (without the "Injector" prefix).
    // We DON'T merge them into isInjectorDuration above to avoid changing axis
    // swap / read-order logic that's tuned for "Injector Duration"; instead we
    // expose a separate flag used only by display-layer ordering.
    const isU34DurationMap = /^duration \d+$/.test(mapNameLower);
    const isEgrMap = mapNameLower === "egr" || (mapNameLower.includes("egr") && !mapNameLower.includes("temperature"));
    const isIdleRpm = mapNameLower.includes("idle rpm");

    // Get real dimensions from API
    // Handle both 2D and 1D maps
    let apiRows: number;
    let apiCols: number;
    if (mapData.dimensions?.TwoDimensional) {
      apiRows = mapData.dimensions.TwoDimensional.rows;
      apiCols = mapData.dimensions.TwoDimensional.cols;
    } else if (mapData.dimensions?.OneDimensional) {
      // 1D map: display as a single row with N columns
      apiRows = 1;
      apiCols = mapData.dimensions.OneDimensional.length;
    } else {
      // Fallback: try to calculate from size (assuming 2 bytes per value)
      const totalCells = mapData.size / 2;
      apiRows = 1;
      apiCols = totalCells;
    }


    // CRITICAL: Determine if axes need to be swapped based on map description
    // Some maps store data with axes swapped (Y in columns, X in rows)
    // We need to check the description to determine the correct orientation
    let needsAxisSwap = shouldSwapAxes(mapData);
    // Injector duration 01-05: swap axes for display (RPM on X, IQ on Y)
    if (isInjectorDurationNon00) {
      needsAxisSwap = true;
    }
    // EGR: afficher 16x13 (RPM en Y, IQ en X) sans transposition (pas de swap)
    if (isEgrMap) {
      needsAxisSwap = false;
    }
    // Idle RPM: ne pas swap, on garde l'axe temp en X (déjà côté backend)
    if (isIdleRpm) {
      needsAxisSwap = false;
    }
    // Start IQ: handled by backend - no frontend swap needed
    // The backend swaps axes if needed and sends correct dimensions
    
    // If axes need swapping:
    // - rows become cols (vertical axis becomes horizontal)
    // - cols become rows (horizontal axis becomes vertical)
    // For Injector duration 01-05, force display dimensions to match WinOLS (rows = IQ count, cols = RPM count)
    //
    // EGR 2D : DEUX conventions backend coexistent — EDC15P et EDC16 émettent
    // 13x16 (dims transposées par rapport au layout fichier 16 lignes RPM x
    // 13 colonnes), l'EDC15VM émet 16x13 déjà dans le sens du fichier
    // (fix_scan_orientation). On ne re-transpose donc QUE la convention
    // 13x16 (rows < cols) ; re-transposer les 16x13 VM tronquait l'axe Y à
    // 13 valeurs, faisait déborder l'axe X dans les données et affichait la
    // grille en escalier.
    const egrDimsSwapped = isEgrMap && apiRows < apiCols;
    const rows = isEgrMap
      ? (egrDimsSwapped ? apiCols : apiRows) // 16 RPM
      : (needsAxisSwap && isInjectorDurationNon00 ? apiRows : (needsAxisSwap ? apiCols : apiRows));  // Display rows (vertical axis)
    const cols = isEgrMap
      ? (egrDimsSwapped ? apiRows : apiCols) // 13 IQ
      : (needsAxisSwap && isInjectorDurationNon00 ? apiCols : (needsAxisSwap ? apiRows : apiCols));  // Display cols (horizontal axis)

    // File column count for data reading (non-swapped)
    const fileCols = egrDimsSwapped ? apiRows : apiCols;


    const values: number[][] = [];
    const startAddress = mapData.address;
    
    // CRITICAL: Read axes from file - swap addresses if axes are swapped
    const xLabels = [];
    let xLabelsWereReversed = false; // Track if X labels were reversed to align columns later
    // Track row-level reversal so the parent can convert display coords back to file coords on export.
    let rowsReversedCount = 0;
    const flipRowsReversed = () => { rowsReversedCount++; };
    
    // CRITICAL: When swapping dimensions (needsAxisSwap), we need to understand:
    // - Backend sends dimensions as: apiRows x apiCols
    // - After swap, display dimensions are: rows = apiCols, cols = apiRows
    // - Backend also sends axis addresses: x_axis_address points to X axis data, y_axis_address points to Y axis data
    // - The number of values at each address corresponds to the ORIGINAL dimensions:
    //   * x_axis_address contains apiCols values (X axis length)
    //   * y_axis_address contains apiRows values (Y axis length)
    // 
    // After dimension swap:
    // - Display X axis (cols) needs apiRows values ÔåÆ use y_axis_address
    // - Display Y axis (rows) needs apiCols values ÔåÆ use x_axis_address
    // - BUT we also need to swap the semantic meaning (X becomes Y semantically, Y becomes X)
    
    // For dimension swap: swap addresses AND corrections
    // For axis swap (different): just swap the semantic meaning but keep dimensions
    
    // Special case: Start IQ - backend has axes swapped
    // From hardcoded_maps.rs: X_axis = 0x4D46E (Temp with correction 0.1/-273), Y_axis = 0x4D458 (RPM with correction 1.0)
    // But backend sends: x_axis_address = 0x4d458 (RPM), y_axis_address = 0x4d46e (Temp)
    // In EDCsuite display: X (columns) = RPM, Y (rows) = Temp
    // So we need: X should read from 0x4d458 (RPM) with correction 1.0, Y should read from 0x4d46e (Temp) with correction 0.1/-273.1
    // The backend has swapped the addresses, so we need to swap them back AND swap corrections
    const isStartIQ = mapData.name?.toLowerCase().includes("start iq");
    
    // For Start IQ: backend has axes swapped
    // Backend sends: x_axis_address = 0x4d458 (RPM values), y_axis_address = 0x4d46e (Temp values)
    // But in hardcoded_maps.rs: X_axis = 0x4D46E (Temp), Y_axis = 0x4D458 (RPM)
    // So backend has swapped the addresses!
    // For display: X (columns) = RPM, Y (rows) = Temp
    // So: X should read from 0x4d458 (backend x_axis) with correction 1.0 (RPM)
    //     Y should read from 0x4d46e (backend y_axis) with correction 0.1/-273.1 (Temp)
    // The backend is correct for display! We just need to NOT swap for Start IQ
    // For Start IQ: from hardcoded_maps.rs
    // X_axis = 0x4D46E (Temp with correction 0.1/-273.0)
    // Y_axis = 0x4D458 (RPM with correction 1.0)
    // But for EDCsuite display: X (columns) = RPM, Y (rows) = Temp
    // 
    // D'apr├¿s les logs: 
    // - Backend x_axis_address = 0x6d586 lit des valeurs brutes: 0, 200, 250, 280, 600, 650, 800, 1200, 1400 (RPM attendues!)
    // - Backend y_axis_address = 0x6d59c lit des valeurs brutes: 2431, 2531, 2631... (Temp attendues apr├¿s correction!)
    // 
    // V├®rification: 2431 * 0.1 - 273.1 = -30┬░C Ô£ô, 2531 * 0.1 - 273.1 = -20┬░C Ô£ô
    // 
    // Donc le backend a d├®j├á invers├® les adresses par rapport ├á hardcoded_maps.rs!
    // Pour l'affichage: X = RPM, Y = Temp
    // Donc: X doit lire ├á backend x_axis_address (0x6d586) avec correction 1.0 (RPM)
    //       Y doit lire ├á backend y_axis_address (0x6d59c) avec correction 0.1/-273.1 (Temp)
    // 
    // Les corrections du backend sont aussi invers├®es:
    // - backend x_axis_correction = 1.0 (RPM) Ô£ô
    // - backend y_axis_correction = 0.1 (Temp) Ô£ô
    // 
    // ATTENTION: D'apr├¿s les logs, le backend envoie:
    // - x_axis_address = 0x6d586 avec correction 1.0 ÔåÆ lit des valeurs brutes: 0, 200, 250, 280, 600, 650, 800, 1200, 1400 (RPM attendues!)
    // - y_axis_address = 0x6d59c avec correction 0.1/-273.1 ÔåÆ lit des valeurs brutes: 2431, 2531, 2631... (Temp attendues apr├¿s correction!)
    // 
    // Mais les logs montrent que X lit ├á 0x6d59c (Temp) et Y lit ├á 0x6d586 (RPM) - les axes sont invers├®s!
    // 
    // Pour l'affichage: X (colonnes) = RPM, Y (lignes) = Temp
    // Donc: X doit lire ├á backend x_axis_address (0x6d586) avec correction 1.0 (RPM)
    //       Y doit lire ├á backend y_axis_address (0x6d59c) avec correction 0.1/-273.1 (Temp)
    // 
    // CRITICAL: Les axes sont invers├®s dans le backend!
    // D'apr├¿s les logs:
    // - backend x_axis_address = 0x6d586 lit des valeurs brutes: 0, 200, 250... (RPM attendues pour X!)
    // - backend y_axis_address = 0x6d59c lit des valeurs brutes: 2431, 2531... (Temp attendues pour Y apr├¿s correction!)
    // 
    // Pour l'affichage: X (colonnes) = RPM, Y (lignes) = Temp
    // Donc: X doit lire ├á backend x_axis_address (0x6d586) avec correction 1.0 (RPM)
    //       Y doit lire ├á backend y_axis_address (0x6d59c) avec correction 0.1/-273.1 (Temp)
    // 
    // CRITICAL: Les corrections sont invers├®es dans le backend!
    // D'apr├¿s les logs:
    // - backend x_axis_address = 0x6d586 lit des valeurs brutes: 0, 200, 250, 280, 600, 650, 800, 1200, 1400 (RPM attendues!)
    // - backend y_axis_address = 0x6d59c lit des valeurs brutes: 2431, 2531, 2631... (Temp attendues apr├¿s correction!)
    // 
    // Mais le backend envoie:
    // - x_axis_correction = 0.1, x_axis_offset = -273.1 (Temp correction) ÔØî
    // - y_axis_correction = 1.0, y_axis_offset = 0 (RPM correction) ÔØî
    // 
    // Pour l'affichage: X (colonnes) = RPM, Y (lignes) = Temp
    // Donc: X doit lire ├á backend x_axis_address (0x6d586) avec correction 1.0 (RPM) Ô£ô
    //       Y doit lire ├á backend y_axis_address (0x6d59c) avec correction 0.1/-273.1 (Temp) Ô£ô
    // 
    // CRITICAL: Pour Start IQ, d'apr├¿s le JSON EDCsuite:
    // - X (colonnes) = Temp├®rature ├á 0x4D46E avec correction 0.1/-273
    // - Y (lignes) = RPM ├á 0x4D458 avec correction 1.0
    // 
    // D'apr├¿s les logs de debug, le backend envoie:
    // - backend x_axis_address = 0x4d458 avec correction 0.1/-273.1 (Temp correction) ÔØî
    // - backend y_axis_address = 0x4d46e avec correction 1.0 (RPM correction) ÔØî
    // 
    // Le backend a invers├® les adresses ET les corrections par rapport ├á EDCsuite!
    // 
    // Pour l'affichage EDCsuite: X (colonnes) = Temp, Y (lignes) = RPM
    // Donc: X doit lire ├á backend y_axis_address (0x4d46e) avec backend y_axis_correction (1.0) ÔåÆ NON!
    //       X doit lire ├á backend y_axis_address (0x4d46e) avec correction 0.1/-273.1 ÔåÆ Temp Ô£ô
    //       Y doit lire ├á backend x_axis_address (0x4d458) avec correction 1.0 ÔåÆ RPM Ô£ô
    // 
    // Il faut ├®changer les adresses MAIS PAS les corrections pour Start IQ!
    // Les corrections doivent ├¬tre ├®chang├®es car le backend les a invers├®es!
    const isBoostTarget = mapNameLower.includes("boost target map");
    const isBoostTarget280 = isBoostTarget && mapData.size === 280;

    // Boost target 280: backend NOW handles axis swap correctly
    // DO NOT swap again in frontend
    // Other maps: use needsAxisSwap as before
    const boostNeedsAxisSwap = isBoostTarget ? false : needsAxisSwap;
    if (isEgrMap) {
      needsAxisSwap = false; // EGR: pas de transposition, on lit dans l'ordre backend
    }
    if (isBoostTarget) {
      needsAxisSwap = false; // Boost target: backend handles swap, no frontend swap needed
    }

    // NOTE: le bouton d'inversion du header (displaySettings.map.invertDisplay)
    // n'agit PAS ici. Toucher needsAxisSwap changerait la formule de lecture
    // (offset col*apiCols+row vs row*apiCols+col) et casserait les valeurs des
    // maps stockées transposées (ex. Drivers wish MJD6 = [pedal][rpm]).
    // L'inversion se fait plus haut comme une transposition pure du résultat
    // (displayMapValues + swap des labels), au même titre que le mirror.

    const egrSwapAddresses = false; // pour EGR, garder X = x_axis (IQ), Y = y_axis (RPM)

    // Standard address assignment - backend handles any swapping needed
    let xAxisAddr = egrSwapAddresses
      ? (mapData.y_axis_address || 0)
      : (boostNeedsAxisSwap ? (mapData.y_axis_address || 0) : (mapData.x_axis_address || 0));

    let yAxisAddr = egrSwapAddresses
      ? (mapData.x_axis_address || 0)
      : (boostNeedsAxisSwap ? (mapData.x_axis_address || 0) : (mapData.y_axis_address || 0));

    // GARDE Boost target (EDC15, little-endian) : les détections antérieures au
    // fix du détecteur émettaient les adresses croisées (X → axe RPM 16 valeurs,
    // Y → axe IQ 10 valeurs), d'où un axe X = RPM×0.01 et un axe Y = IQ + débord
    // de 6 cellules de data (les « 198 »). La structure fichier des axes est
    // [ID u16][len u16 LE][valeurs] : on lit la longueur réelle à adresse-2 ;
    // si X pointe l'axe de `rows` valeurs et Y celui de `cols`, on échange les
    // adresses. Corrections/dims restent telles quelles (déjà en orientation
    // affichage : X=IQ 0.01, Y=RPM 1.0). No-op pour les détections correctes.
    if (isBoostTarget && !isBigEndianEcu(ecuType) && rows !== cols && xAxisAddr > 2 && yAxisAddr > 2) {
      const axisLenAt = (addr: number): number | null =>
        addr - 2 >= 0 && addr < fileData.length
          ? (fileData[addr - 2] | (fileData[addr - 1] << 8))
          : null;
      const xFileLen = axisLenAt(xAxisAddr);
      const yFileLen = axisLenAt(yAxisAddr);
      if (xFileLen === rows && yFileLen === cols) {
        const tmp = xAxisAddr;
        xAxisAddr = yAxisAddr;
        yAxisAddr = tmp;
      }
    }
    
    // Special case: Injector duration 01-04 need corrections swapped but NOT addresses
    // These maps have correct addresses but inverted correction factors from backend
    // Injector duration: swap corrections for 01-04 (backend factors inverted),
    // and fall back to heuristic for other duration maps when metadata looks inverted.
    const isInjectorDuration01to04 = isInjectorDuration &&
                                     !mapNameLower.includes("duration 00") &&
                                     !mapNameLower.includes("duration 05");
    const xLabelLower = (mapData.x_label || "").toLowerCase();
    const yLabelLower = (mapData.y_label || "").toLowerCase();
    const xLooksRpm = xLabelLower.includes("rpm") || xLabelLower.includes("engine speed");
    const yLooksRpm = yLabelLower.includes("rpm") || yLabelLower.includes("engine speed");
    const xLooksIQ = xLabelLower.includes("mg") || xLabelLower.includes("iq");
    const yLooksIQ = yLabelLower.includes("mg") || yLabelLower.includes("iq");
    const correctionsLookInverted =
      isInjectorDuration &&
      (
        (xLooksRpm && yLooksIQ && (mapData.x_axis_correction ?? 1) < (mapData.y_axis_correction ?? 1)) ||
        (xLooksIQ && yLooksRpm && (mapData.x_axis_correction ?? 1) > (mapData.y_axis_correction ?? 1))
      );
    
    // Start IQ: NO swap needed - backend now sends correct data:
    // - x_axis = Temp with x_axis_correction=0.1, x_axis_offset=-273.1
    // - y_axis = RPM with y_axis_correction=1.0, y_axis_offset=0.0
    // For Injector duration 01-04: swap corrections only (addresses are correct but corrections are inverted)
    
    const swapCorrectionsOnly = (isInjectorDuration00 || isInjectorDuration01to04 || correctionsLookInverted) && !boostNeedsAxisSwap;
    // Bases génériques (non-boost)
    const baseXAxisCorrection = mapData.x_axis_correction ?? 1.0;
    const baseYAxisCorrection = mapData.y_axis_correction ?? 1.0;
    const baseXAxisOffset = mapData.x_axis_offset ?? 0.0;
    const baseYAxisOffset = mapData.y_axis_offset ?? 0.0;
    // Boost target: backend NOW sends correct values, use them directly
    // Backend sends: x_axis = IQ (0.01 correction), y_axis = RPM (1.0 correction)
    const boostDefaultXAxisCorrection = mapData.x_axis_correction ?? 0.01;  // X = IQ
    const boostDefaultYAxisCorrection = mapData.y_axis_correction ?? 1.0;   // Y = RPM
    const boostDefaultXAxisOffset = mapData.x_axis_offset ?? 0.0;
    const boostDefaultYAxisOffset = mapData.y_axis_offset ?? 0.0;

    // Standard correction assignment - use backend values
    let xAxisCorrection = isIdleRpm
      ? (mapData.x_axis_correction ?? 0.1) // Idle RPM: Temp en X (axe X backend)
      : (isBoostTarget ? boostDefaultXAxisCorrection : (boostNeedsAxisSwap || swapCorrectionsOnly ? baseYAxisCorrection : baseXAxisCorrection));
    let xAxisOffset = isIdleRpm
      ? (mapData.x_axis_offset ?? -273.1)
      : (isBoostTarget ? boostDefaultXAxisOffset : (boostNeedsAxisSwap || swapCorrectionsOnly ? baseYAxisOffset : baseXAxisOffset));
    let yAxisCorrection = isIdleRpm
      ? (mapData.y_axis_correction ?? 1.0) // Idle RPM: valeur RPM en Y (axe Y backend)
      : (isBoostTarget ? boostDefaultYAxisCorrection : (boostNeedsAxisSwap || swapCorrectionsOnly ? baseXAxisCorrection : baseYAxisCorrection));
    let yAxisOffset = isIdleRpm
      ? (mapData.y_axis_offset ?? 0.0)
      : (isBoostTarget ? boostDefaultYAxisOffset : (boostNeedsAxisSwap || swapCorrectionsOnly ? baseXAxisOffset : baseYAxisOffset));

    // Hard override for Boost target already applied above via isBoostTarget flag
    // Idle RPM: force temperature scaling on X (0.1, -273.1) to display °C correctly
    if (isIdleRpm) {
      xAxisCorrection = 0.1;
      xAxisOffset = -273.1;
    }

    // Per-project display overrides from the map Properties window
    // (WinOLS convention: displayed = raw * factor / divisor + offset).
    // Only present when the user explicitly saved settings for this map;
    // they win over the detected corrections, including the special cases
    // above (the Properties window shows the DISPLAYED axes).
    const dsAxisFactor = (ds?: { factor?: number; divisor?: number }): number | undefined => {
      if (!ds || typeof ds.factor !== 'number' || !isFinite(ds.factor)) return undefined;
      const div = typeof ds.divisor === 'number' && isFinite(ds.divisor) && ds.divisor !== 0 ? ds.divisor : 1;
      return ds.factor / div;
    };
    const dsXFactor = dsAxisFactor(displaySettings?.xAxis);
    if (dsXFactor !== undefined) xAxisCorrection = dsXFactor;
    if (typeof displaySettings?.xAxis?.offset === 'number' && isFinite(displaySettings.xAxis.offset)) {
      xAxisOffset = displaySettings.xAxis.offset;
    }
    const dsYFactor = dsAxisFactor(displaySettings?.yAxis);
    if (dsYFactor !== undefined) yAxisCorrection = dsYFactor;
    if (typeof displaySettings?.yAxis?.offset === 'number' && isFinite(displaySettings.yAxis.offset)) {
      yAxisOffset = displaySettings.yAxis.offset;
    }
    const dsPrecision = (v: number | undefined): number | undefined =>
      typeof v === 'number' && isFinite(v) && v >= 0 ? Math.min(6, Math.trunc(v)) : undefined;
    const xAxisDecimalsOverride = dsPrecision(displaySettings?.xAxis?.precision);
    const yAxisDecimalsOverride = dsPrecision(displaySettings?.yAxis?.precision);
    
    // CRITICAL: After swap, determine how many values to read from each address
    // Original: x_axis_address has apiCols values, y_axis_address has apiRows values
    // After swap:
    //   - Display X axis (cols = apiRows) needs values from y_axis_address (which has apiRows values)
    //   - Display Y axis (rows = apiCols) needs values from x_axis_address (which has apiCols values)
    // So the number of values to read is correct: cols values from xAxisAddr, rows values from yAxisAddr
    
    // CRITICAL: Check if we're reading the correct number of values
    // When axes are swapped:
    // - xAxisAddr = y_axis_address_backend (which originally had apiRows values)
    // - yAxisAddr = x_axis_address_backend (which originally had apiCols values)
    // After swap for display:
    // - X axis (cols) needs apiRows values ÔåÆ read from xAxisAddr (which has apiRows values) Ô£ô
    // - Y axis (rows) needs apiCols values ÔåÆ read from yAxisAddr (which has apiCols values) Ô£ô
    // But wait: we read cols values from xAxisAddr and rows values from yAxisAddr
    // After swap: cols = apiRows, rows = apiCols, so this is correct!
    if (xAxisAddr > 0) {
      const tempXLabels = [];
      // After swap: cols = apiRows, so we read cols values from xAxisAddr (which has apiRows values)
      // Without swap: cols = apiCols, so we read cols values from xAxisAddr (which has apiCols values)
      const expectedXCount = cols; // X axis = columns
      for (let i = 0; i < expectedXCount; i++) {
        const offset = xAxisAddr + (i * 2);
        if (offset + 1 < fileData.length) {
          // Determine endianness for AXIS values
          // NOTE: is_little_endian flag only affects DATA values, not axis values
          // Axis values follow the ECU byte order (EDC16/MJD6 = Big-Endian),
          // even for SOI Selector
          const useBigEndian = isBigEndianEcu(ecuType);

          let rawValue: number;
          if (useBigEndian) {
            // BIG ENDIAN for EDC16/MJD6: high byte first, low byte second
            rawValue = (fileData[offset] << 8) | fileData[offset + 1];
          } else {
            // LITTLE ENDIAN for EDC15 and others (or maps with is_little_endian=true)
            rawValue = fileData[offset] | (fileData[offset + 1] << 8);
          }

          // Convert to signed 16-bit (i16) - axis values can be negative
          // (except Marelli MJD6: unsigned axes with >32767 RPM sentinels)
          if (rawValue > 32767 && !hasUnsignedAxes(ecuType)) {
            rawValue = rawValue - 65536;
          }
          // Apply correction AND offset for X axis (important for temperature conversions!)
          // CRITICAL: Handle null/undefined offsets properly
          // Formula: correctedValue = (rawValue * correction) + offset
          // For temperature: rawValue is stored in 0.1 Kelvin units
          // Example: rawValue=2431 ÔåÆ (2431 * 0.1) + (-273) = 243.1 - 273 = -29.9┬░C Ô£ô
          // Example: rawValue=2730 ÔåÆ (2730 * 0.1) + (-273) = 273.0 - 273 = 0┬░C Ô£ô
          // Example: rawValue=3730 ÔåÆ (3730 * 0.1) + (-273) = 373.0 - 273 = 100┬░C Ô£ô
          // From JSON: AxisX.Factor="0.100000", AxisX.Offset="-273"
          const correctedValue = (rawValue * xAxisCorrection) + xAxisOffset;
          // Format based on correction factor: if 0.01, show 2 decimals; if 1.0, show 0 decimals
          // Drivers wish MJD6 : axes % (0.004) et RPM à valeurs entières par
          // construction — forcer 0 décimale (sinon l'heuristique affiche
          // "0.00, 5.00, … 100.00" pour la pédale).
          const isMjdDriversWish = mapNameLower.includes("drivers wish") && hasUnsignedAxes(ecuType);
          const decimals = xAxisDecimalsOverride ?? (isMjdDriversWish ? 0 : (xAxisCorrection < 0.1 ? 2 : (xAxisCorrection < 1.0 ? 1 : 0)));
          tempXLabels.push(correctedValue.toFixed(decimals));
        } else {
          tempXLabels.push("0");
        }
      }
      // CRITICAL: EDCsuite displays X axis in ascending order (e.g., -273.1 left, -133.1 right)
      // The file stores X axis values, and we need to ensure they're in ascending order
      // Check if values are in descending order and reverse if needed
      if (tempXLabels.length > 1) {
        const first = parseFloat(tempXLabels[0]);
        const last = parseFloat(tempXLabels[tempXLabels.length - 1]);
        if (first > last) {
          // Values are descending in file, reverse to get ascending for display (EDCsuite format)
          xLabels.push(...tempXLabels.reverse());
          xLabelsWereReversed = true;
        } else {
          // Values are already ascending, use as is
          xLabels.push(...tempXLabels);
        }
      } else {
        xLabels.push(...tempXLabels);
      }
    } else {
      // Fallback to generated labels
      for (let i = 0; i < cols; i++) {
        xLabels.push((i * 5).toFixed(2));
      }
    }
    
    // CRITICAL FIX: Read Y axis from file for Y display (vertical/rows)
    // According to JSON: AxisY.bBackwards = "1" means axis is backwards (stored descending)
    // User wants: 0 to 1400 rpm (croissant de bas en haut) - so 0 at bottom, 1400 at top
    // If bBackwards=1: file has [1400, 1200, ..., 0], we need to reverse to [0, 200, ..., 1400]
    // After reversing map values, row 0 = 0 rpm (top), row N = 1400 rpm (bottom)
    // But user wants 0 at bottom, 1400 at top, so labels should be [0, ..., 1400] from bottom to top
    // Which means [1400, ..., 0] from top to bottom in display
    // USER PROVIDED ADDRESSES: Y axis at 0x6D586
    const yLabels = [];
    let tempYLabels: string[] = []; // Declare outside if block so it's accessible later
    let originalFileOrderIsAscending = false; // Store original file order before any reversal
    // CRITICAL: Check if we're reading the correct number of values
    // Y axis should have 'rows' values
    if (yAxisAddr > 0) {
      tempYLabels = [];
      const expectedYCount = rows; // Y axis = rows (after swap: apiCols if swapped, apiRows if not)
      for (let i = 0; i < expectedYCount; i++) {
        const offset = yAxisAddr + (i * 2);
        if (offset + 1 < fileData.length) {
          // Determine endianness for AXIS values
          // NOTE: is_little_endian flag only affects DATA values, not axis values
          // Axis values follow the ECU byte order (EDC16/MJD6 = Big-Endian),
          // even for SOI Selector
          const useBigEndian = isBigEndianEcu(ecuType);

          let rawValue: number;
          if (useBigEndian) {
            // BIG ENDIAN for EDC16/MJD6: high byte first, low byte second
            rawValue = (fileData[offset] << 8) | fileData[offset + 1];
          } else {
            // LITTLE ENDIAN for EDC15 and others (or maps with is_little_endian=true)
            rawValue = fileData[offset] | (fileData[offset + 1] << 8);
          }

          // Convert to signed 16-bit (i16) - axis values can be negative
          // (except Marelli MJD6: unsigned axes with >32767 RPM sentinels)
          if (rawValue > 32767 && !hasUnsignedAxes(ecuType)) {
            rawValue = rawValue - 65536;
          }
          // Apply correction AND offset for Y axis
          // CRITICAL: Handle null/undefined offsets properly
          // Formula: correctedValue = (rawValue * correction) + offset
          // For RPM: usually correction=1.0, offset=0.0
          const correctedValue = (rawValue * yAxisCorrection) + yAxisOffset;
          tempYLabels.push(correctedValue.toFixed(yAxisDecimalsOverride ?? 0));
        } else {
          tempYLabels.push("0");
        }
      }
      // CRITICAL: Determine correct Y axis order based on map type
      // - RPM maps: descending order (largest at top, smallest at bottom) - EDCsuite standard
      // - Torque limiter and mbar/pressure maps: ascending order (smallest at bottom, largest at top)
      const isTorqueLimiter = mapData.name?.toLowerCase().includes("torque limiter");
      const isIQByMap = mapData.name?.toLowerCase().includes("iq by map");
      const isIQByMAF = mapData.name?.toLowerCase().includes("iq by maf");
      const isStartIQ = mapData.name?.toLowerCase().includes("start iq");
      
      if (tempYLabels.length > 1) {
        const firstY = parseFloat(tempYLabels[0]);
        const lastY = parseFloat(tempYLabels[tempYLabels.length - 1]);
        const fileOrderIsAscending = firstY < lastY;
        originalFileOrderIsAscending = fileOrderIsAscending; // Store for later
        
        if (isTorqueLimiter) {
          // Torque limiter: Y axis should be DESCENDING (1000 at top, 500 at bottom) - same as RPM maps
          if (fileOrderIsAscending) {
            // File has ascending order [500, 900, 1000], reverse to descending [1000, 900, 500]
            yLabels.push(...tempYLabels.reverse());
          } else {
            // File already has descending order [1000, 900, 500], use as is
            yLabels.push(...tempYLabels);
          }
        } else if (isIQByMap || isIQByMAF) {
          // IQ by MAP/MAF: Y axis (RPM) should be DESCENDING (5355 at top, 861 at bottom) - like EDCsuite
          const mapType = isIQByMAF ? "IQ by MAF" : "IQ by map";
          if (fileOrderIsAscending) {
            // File has ascending order [861, ..., 5355], reverse to descending [5355, ..., 861]
            yLabels.push(...tempYLabels.reverse());
          } else {
            // File already has descending order [5355, ..., 861], use as is
            yLabels.push(...tempYLabels);
          }
        } else if (isStartIQ) {
          // Start IQ: Y axis (RPM) should be DESCENDING (1400 at top, 0 at bottom) - like EDCsuite
          // Always force descending order for Start IQ (RPM axis)
          if (fileOrderIsAscending) {
            // File has ascending order [0, 200, ..., 1400], reverse to descending [1400, ..., 200, 0]
            yLabels.push(...tempYLabels.reverse());
          } else {
            // File already has descending order [1400, ..., 200, 0], but we need to ensure it's correct
            // Check if it's really descending (first > last)
            const firstY = parseFloat(tempYLabels[0]);
            const lastY = parseFloat(tempYLabels[tempYLabels.length - 1]);
            if (firstY > lastY) {
              // Already descending, use as is
              yLabels.push(...tempYLabels);
            } else {
              // Not descending, reverse it
              yLabels.push(...tempYLabels.reverse());
            }
          }
        } else {
          // Standard RPM maps: Y axis should be DESCENDING (largest at top, smallest at bottom)
          if (fileOrderIsAscending) {
            // File has ascending order [260, ..., 2820], reverse to descending for display
            yLabels.push(...tempYLabels.reverse());
          } else {
            // File already has descending order [2820, ..., 260], use as is
            yLabels.push(...tempYLabels);
          }
        }
      } else {
        yLabels.push(...tempYLabels);
      }
    } else {
      // Fallback to generated labels
      for (let i = 0; i < rows; i++) {
        yLabels.push(String(i * 100 + (i === 0 ? 0 : 21)));
      }
    }
    
    // CRITICAL: Read map data in row-major order
    // If axes are swapped, we need to transpose the data during reading
    // File stores data as: [row][col] where row = original Y, col = original X
    // If swapped, display[row][col] should read file[col][row]
    // File dimensions: apiRows x apiCols (e.g., 21 rows x 3 cols)
    // Display dimensions after swap: rows = apiCols (3), cols = apiRows (21)
    // 8-bit maps (UInt8/Int8: Marelli boost/VGT/rail request, EDC16 eByte
    // maps) store one byte per cell — the stride and decode must follow,
    // otherwise every cell reads two neighboring cells as one 16-bit value.
    const dataTypeStr = String(mapData.data_type || '');
    const cellBytes = dataTypeStr === 'UInt8' || dataTypeStr === 'Int8' ? 1 : 2;
    for (let row = 0; row < rows; row++) {
      const rowValues: number[] = [];
      for (let col = 0; col < cols; col++) {
        // Calculate offset based on whether axes are swapped
        let offset: number;
        if (needsAxisSwap) {
          // CRITICAL: When swapping dimensions, we need to transpose the data
          // The repeating pattern every 7 columns suggests the current formula is wrong
          // 
          // File structure: file[fileRow][fileCol] stored row-major
          //   offset = startAddress + (fileRow * apiCols + fileCol) * 2
          // 
          // For Torque limiter: file is 21 rows x 3 cols, display is 3 rows x 21 cols
          // The pattern repeats every 7 columns, which is 21/3 = 7
          // This suggests we might need to read column-major instead of transposing
          //
          // Let's try: display[row][col] reads file using column-major indexing
          // Or maybe the file stores data differently for this map type
          //
          // Alternative formula: offset = startAddress + (row * apiRows + col) * 2
          // This would read column-major: first all values of col 0, then all of col 1, etc.
          const isTorqueLimiter = mapData.name?.toLowerCase().includes("torque limiter");
          const isIQByMap = mapData.name?.toLowerCase().includes("iq by map");
          const isIQByMAF = mapData.name?.toLowerCase().includes("iq by maf");
          
          if (isTorqueLimiter) {
            // Try column-major reading for Torque limiter
            // display[row][col] reads from column-major storage
            offset = startAddress + (row * apiRows + col) * cellBytes;
          } else if (isIQByMap || isIQByMAF) {
            // For IQ by MAP/MAF: data is stored column-major in the file
            // File stores: all 13 values for RPM 0, then all 13 values for RPM 1, etc.
            // File dimensions: apiRows=13 (X axis), apiCols=16 (RPM)
            // Display dimensions after swap: rows=16 (RPM), cols=13 (X axis)
            //
            // Column-major storage means:
            // - Offset for RPM 0, col 0: startAddress + 0
            // - Offset for RPM 0, col 1: startAddress + 2
            // - Offset for RPM 0, col 2: startAddress + 4
            // - ...
            // - Offset for RPM 1, col 0: startAddress + (13 * 2) = startAddress + 26
            //
            // So: display[RPM][col] = file[RPM_index * 13 + col_index]
            // offset = startAddress + (row * apiRows + col) * cellBytes
            offset = startAddress + (row * apiRows + col) * cellBytes;

            // Enhanced debug logging for first row
            if (row === 0 && col < 3) {
              const mapType = isIQByMAF ? "IQ by MAF" : "IQ by map";
            }
          } else if (isInjectorDuration) {
            // Injector Duration maps (01-05): data is stored in row-major order
            // The swap is only for display orientation, NOT for data reading
            // File stores: [row][col] in row-major order with apiCols columns per row
            // We just need to read in normal row-major order
            offset = startAddress + (row * apiCols + col) * cellBytes;
          } else {
            // Standard transposition: display[row][col] reads file[col][row]
            offset = startAddress + (col * apiCols + row) * cellBytes;
          }
        } else {
          // Normal row-major order: file[row][col] at offset = startAddress + (row * fileCols + col) * cellBytes
          offset = startAddress + (row * fileCols + col) * cellBytes;
        }
        if (offset + cellBytes - 1 < fileData.length) {
          let rawValue: number;
          if (cellBytes === 1) {
            // 8-bit cells: no endianness, sign only for Int8
            rawValue = fileData[offset];
            if (dataTypeStr === 'Int8' && rawValue > 127) {
              rawValue = rawValue - 256;
            }
          } else {
            // Determine endianness: check map-specific flag first, then ECU type
            // EDC16 (all variants) and Marelli MJD6 use Big-Endian by default,
            // EDC15 and others use Little-Endian
            // Special case: SOI Selector is always Little-Endian (even if flag is missing from old detections)
            const isSOISelector = mapData.name?.toLowerCase().includes('soi selector');
            const mapIsLittleEndian = mapData.is_little_endian === true || isSOISelector;
            const useBigEndian = !mapIsLittleEndian && isBigEndianEcu(ecuType);

            if (useBigEndian) {
              // BIG ENDIAN for EDC16/MJD6: high byte first, low byte second
              rawValue = (fileData[offset] << 8) | fileData[offset + 1];
            } else {
              // LITTLE ENDIAN for EDC15 and others (or maps with is_little_endian=true)
              rawValue = fileData[offset] | (fileData[offset + 1] << 8);
            }

            // Convert to signed 16-bit (i16) if data_type is Int16, OR for maps
            // that are ALWAYS signed by nature: "Drivers wish" (MJD6/EDC16)
            // carries negative torque in its engine-brake cells (raw ~0xF6xx),
            // et "Driver wish"/"Inverse driver wish" (EDC15, WinOLS bSigned=1)
            // portent des IQ négatifs (raw 0xFFxx → -0.9, pas 654.5). Forcer le
            // signe couvre les detection_data antérieures taguées UInt16.
            // « EGR hysteresis » (EDC16) : seuils signés (0xFFFF = -1), les
            // detection_data antérieures à v31 les taguaient UInt16.
            const isAlwaysSignedMap =
              mapNameLower.includes('drivers wish') || mapNameLower.includes('driver wish') ||
              mapNameLower.includes('egr hysteresis');
            if ((mapData.data_type === 'Int16' || isAlwaysSignedMap) && rawValue > 32767) {
              rawValue = rawValue - 65536;
            }
          }
          // Apply correction factor and offset (per-project overrides from
          // the Properties window win over the detected values)
          const correction = dsAxisFactor(displaySettings?.map) ?? (mapData.correction_factor ?? 1.0);
          const offsetValue =
            typeof displaySettings?.map?.offset === 'number' && isFinite(displaySettings.map.offset)
              ? displaySettings.map.offset
              : (mapData.offset ?? 0.0);
          const correctedValue = (rawValue * correction) + offsetValue;
          rowValues.push(correctedValue);
        } else {
          rowValues.push(0);
        }
      }
      values.push(rowValues);
    }


    // CRITICAL FIX: Align values with axis labels for correct display
    // 
    // EDCsuite display format:
    // - Y axis (RPM): descending order [2820, ..., 260] from top to bottom
    // - X axis (Temperature): ascending order [-273.1, ..., -133.1] from left to right
    // 
    // File storage format (EDC15P):
    // - Y axis values: can be stored in ascending [260, ..., 2820] or descending [2820, ..., 260] order
    // - Map data: stored row-major, row 0 corresponds to first Y value, row N to last Y value
    // 
    // Our processing:
    // - yLabels: processed to be in descending order [2820, ..., 260] for display (EDCsuite format)
    // - values: read in file order [row 0 = first Y value, row N = last Y value]
    // 
    // To match EDCsuite:
    // - values[0] should correspond to yLabels[0] (highest RPM at top)
    // - values[N] should correspond to yLabels[N] (lowest RPM at bottom)
    // 
    // We need to check if the file order matches the display order:
    // - If file has Y values in ascending order [260, ..., 2820] and we reversed labels to [2820, ..., 260]
    //   Then we MUST reverse values so values[0] = 2820 RPM (matches yLabels[0])
    // - If file has Y values in descending order [2820, ..., 260] and labels are [2820, ..., 260]
    //   Then we DON'T reverse values (already aligned)
    if (yLabels.length > 1 && tempYLabels && tempYLabels.length > 1) {
      // Use the stored original file order (before any reversal happened)
      // Note: originalFileOrderIsAscending was set before tempYLabels was modified
      const fileOrderIsAscending = originalFileOrderIsAscending;
      
      // Check the display order (after processing)
      const displayFirstY = parseFloat(yLabels[0]);
      const displayLastY = parseFloat(yLabels[yLabels.length - 1]);
      const displayOrderIsDescending = displayFirstY > displayLastY;
      const displayOrderIsAscending = displayFirstY < displayLastY;
      
      // For Torque limiter and similar maps with mbar/pressure Y axis, display should be ASCENDING
      // (smallest at bottom, largest at top)
      // For RPM maps, display should be DESCENDING (largest at top, smallest at bottom)
      const isTorqueLimiter = mapData.name?.toLowerCase().includes("torque limiter");
      
      if (isTorqueLimiter) {
        // Torque limiter: Y axis should be descending (1000 at top, 500 at bottom) - same as RPM maps
        // If file order is ascending [500, 900, 1000] and display is descending [1000, 900, 500], reverse values
        if (fileOrderIsAscending && displayOrderIsDescending) {
          values.reverse(); flipRowsReversed();
        } else if (!fileOrderIsAscending && displayOrderIsDescending) {
          // File order is descending, display is descending - no reversal needed
        }
      } else {
        // Standard RPM maps: Y axis should be descending (largest at top, smallest at bottom)
        // This includes "Start IQ" which has RPM on Y axis
        const isStartIQ = mapData.name?.toLowerCase().includes("start iq");
        if (fileOrderIsAscending && displayOrderIsDescending) {
          values.reverse(); flipRowsReversed();
        } else if (!fileOrderIsAscending && displayOrderIsDescending) {
          // File order is descending, display is descending - no reversal needed
        } else if (isStartIQ && fileOrderIsAscending && !displayOrderIsDescending) {
          // Start IQ: file is ascending [0, 200, ..., 1400] but display should be descending [1400, ..., 200, 0]
          // This means labels were not reversed, so we need to reverse both labels and values
          yLabels.reverse();
          values.reverse(); flipRowsReversed();
        }
      }

      // Special handling for "IQ by MAP" and "IQ by MAF" - align values with reversed Y axis labels
      const isIQByMap = mapData.name?.toLowerCase().includes("iq by map");
      const isIQByMAF = mapData.name?.toLowerCase().includes("iq by maf");
      if ((isIQByMap || isIQByMAF) && values.length > 0 && yLabels.length > 0 && tempYLabels && tempYLabels.length > 0) {
        // IQ by MAP/MAF: Y axis (RPM) should be DESCENDING (5355 at top, 861 at bottom) like EDCsuite
        const mapType = isIQByMAF ? "IQ by MAF" : "IQ by map";
        const firstY = parseFloat(tempYLabels[0]);
        const lastY = parseFloat(tempYLabels[tempYLabels.length - 1]);
        const fileOrderIsAscending = firstY < lastY;

        // Check current display order
        const displayFirstY = parseFloat(yLabels[0]);
        const displayLastY = parseFloat(yLabels[yLabels.length - 1]);
        const displayOrderIsDescending = displayFirstY > displayLastY;

        // If labels were reversed (file ascending -> display descending), reverse values too
        if (fileOrderIsAscending && displayOrderIsDescending) {
          // Labels were reversed to get descending order, so reverse values to match
          values.reverse(); flipRowsReversed();
        }
      }

      // Special handling for "Start IQ" - the row with zeros should be at the top
      // BUT: Y axis (RPM) must ALWAYS be descending (1400 at top, 0 at bottom) - EDCsuite standard
      const isStartIQ = mapData.name?.toLowerCase().includes("start iq");
      if (isStartIQ && values.length > 0) {
        // Check if the last row (bottom) has zeros
        const lastRow = values[values.length - 1];
        const firstRow = values[0];
        const lastRowHasZeros = lastRow && lastRow.every(val => val === 0 || Math.abs(val) < 0.01);
        const firstRowHasZeros = firstRow && firstRow.every(val => val === 0 || Math.abs(val) < 0.01);

        // Check current Y axis order (should be descending: 1400 at top, 0 at bottom)
        const currentFirstY = yLabels.length > 0 ? parseFloat(yLabels[0]) : 0;
        const currentLastY = yLabels.length > 0 ? parseFloat(yLabels[yLabels.length - 1]) : 0;
        const yAxisIsDescending = currentFirstY > currentLastY;

        // If the last row has zeros but the first doesn't, reverse the rows
        // This ensures the row with zeros is at the top (as in EDCsuite)
        if (lastRowHasZeros && !firstRowHasZeros) {
          values.reverse(); flipRowsReversed();
          // Also reverse Y axis labels to match the reversed rows
          if (yLabels.length > 0) {
            yLabels.reverse();
          }
        } else if (firstRowHasZeros && !lastRowHasZeros) {
          // Already correct, no reversal needed
        }

        // CRITICAL: Ensure Y axis is ALWAYS descending (1400 at top, 0 at bottom) for Start IQ
        // This is the EDCsuite standard for RPM axes
        if (yLabels.length > 1) {
          const finalFirstY = parseFloat(yLabels[0]);
          const finalLastY = parseFloat(yLabels[yLabels.length - 1]);
          if (finalFirstY < finalLastY) {
            // Y axis is ascending (0 at top, 1400 at bottom) - must reverse to descending
            yLabels.reverse();
            values.reverse(); flipRowsReversed();
          } else {
          }
        }
      }
    }

    // CRITICAL: If X axis labels were reversed, we must also reverse the columns of all rows
    // This ensures that values[row][col] corresponds to xLabels[col]
    if (xLabelsWereReversed && values.length > 0) {
      for (let row = 0; row < values.length; row++) {
        values[row].reverse();
      }
    }

    // SPECIAL: Handle y_axis_inverted flag from backend
    // When set, display with small values at top (like WinOLS Selector for injector duration)
    if (mapData.y_axis_inverted && yLabels.length > 0 && values.length > 0) {
      // Check current order - if descending (large at top), reverse to ascending (small at top)
      const firstY = parseFloat(yLabels[0]);
      const lastY = parseFloat(yLabels[yLabels.length - 1]);
      if (firstY > lastY) {
        // Currently descending (27 at top, 0 at bottom), reverse to ascending (0 at top, 27 at bottom)
        yLabels.reverse();
        values.reverse(); flipRowsReversed();
      }
    }

    // EDC16U34 "Duration NN" maps: enforce row/label alignment by comparing
    // raw-byte signatures. The upstream branches sometimes leave rows
    // mirrored relative to the labels; this safety net detects the mismatch
    // structurally (no dependency on the correction factor or label values).
    //
    // Rule: the displayed top row must correspond to the file row that
    // matches the largest yLabel (typically max RPM). We figure out which
    // file row that is by comparing raw row sums in the file with the
    // displayed-then-decoded row sums.
    if (isU34DurationMap && values.length > 1 && tempYLabels.length === values.length) {
      const startAddressByte = mapData.address;
      const cellsPerRow = values[0]?.length ?? 0;
      const totalRows = values.length;
      const bytesPerRow = cellsPerRow * 2;

      const readSignedBE = (off: number): number => {
        if (off + 1 >= fileData.length) return 0;
        const raw = (fileData[off] << 8) | fileData[off + 1];
        return (mapData.data_type === 'Int16' && raw > 32767) ? raw - 65536 : raw;
      };
      const fileRowSum = (fileRowIdx: number): number => {
        let s = 0;
        for (let c = 0; c < cellsPerRow; c++) {
          s += readSignedBE(startAddressByte + fileRowIdx * bytesPerRow + c * 2);
        }
        return s;
      };
      const factor = dsAxisFactor(displaySettings?.map) ?? (mapData.correction_factor ?? 1.0);
      const offsetVal =
        typeof displaySettings?.map?.offset === 'number' && isFinite(displaySettings.map.offset)
          ? displaySettings.map.offset
          : (mapData.offset ?? 0.0);
      const displayRowSumAsRaw = (row: number[]): number => {
        let s = 0;
        for (const v of row) s += Math.round((v - offsetVal) / (factor || 1));
        return s;
      };

      const fileTopSum = fileRowSum(0);
      const fileBotSum = fileRowSum(totalRows - 1);
      const dispTopSum = displayRowSumAsRaw(values[0]);

      // Which end of the file does the displayed top match more closely?
      const topMatchesFileTop = Math.abs(dispTopSum - fileTopSum) <= Math.abs(dispTopSum - fileBotSum);

      // Which file row should be at the top? The one whose Y label equals the
      // currently-displayed top label.
      const displayTopLabel = parseFloat(yLabels[0]);
      const fileFirstLabel = parseFloat(tempYLabels[0]);
      const fileLastLabel = parseFloat(tempYLabels[tempYLabels.length - 1]);
      const desiredTopIsFileTop = Math.abs(displayTopLabel - fileFirstLabel)
        <= Math.abs(displayTopLabel - fileLastLabel);

      if (topMatchesFileTop !== desiredTopIsFileTop) {
        values.reverse();
        flipRowsReversed();
      }
    }

    // Net row reversal: each reverse() inverts the order, so an odd count means
    // display row 0 corresponds to file row N-1.
    const rowsReversed = (rowsReversedCount % 2) === 1;
    const colsReversed = xLabelsWereReversed;
     

    // Mettre en cache les r├®sultats
    const cacheData: CachedMapData = {
      mapValues: values,
      xAxisLabels: xLabels,
      yAxisLabels: yLabels,
      axesSwapped: needsAxisSwap,
      rowsReversed,
      colsReversed,
      mapAddress: mapData.address,
      fileDataHash: fileDataHash,
      version: CACHE_VERSION,
      apiRows: apiRows,
      apiCols: apiCols,
      invert: cacheInvert,
    };
    mapDataCache.set(cacheKey, cacheData);

    return {
      mapValues: values,
      xAxisLabels: xLabels,
      yAxisLabels: yLabels,
      axesSwapped: needsAxisSwap,
      rowsReversed,
      colsReversed,
    };
  }, [mapData, fileData, projectName, fileName, displaySettings]);

  // Reset data when map changes to prevent showing stale data
  useEffect(() => {
    if (mapData.address !== currentMapAddress) {
      setCurrentMapAddress(null); // Mark as loading
      // Ne pas réinitialiser changedCells ici car ils sont gérés par le parent via initialChangedCells
      setContextMenu(null);
    }
  }, [mapData.address, currentMapAddress]);

  // Notify the parent of this map's row/col flip state so exports can map
  // display coords back to file coords.
  useEffect(() => {
    if (!extractedData || !onAxesFlipChange) return;
    onAxesFlipChange(mapData.address, {
      rowsReversed: extractedData.rowsReversed,
      colsReversed: extractedData.colsReversed,
    });
  }, [extractedData, mapData.address, onAxesFlipChange]);
  
  // Mettre ├á jour les ├®tats avec les donn├®es extraites
  useEffect(() => {
    if (extractedData) {
      // D'abord stocker les valeurs originales
      originalValuesRef.current = extractedData.mapValues.map(row => [...row]);
      originalXAxisLabelsRef.current = [...extractedData.xAxisLabels];
      originalYAxisLabelsRef.current = [...extractedData.yAxisLabels];

      // Appliquer les modifications si elles existent
      let initialValues = extractedData.mapValues;
      if (initialChangedCells && Object.keys(initialChangedCells).length > 0) {
        initialValues = extractedData.mapValues.map((row, rowIndex) =>
          row.map((originalValue, colIndex) => {
            const cellKey = `${rowIndex}-${colIndex}`;
            return initialChangedCells[cellKey] !== undefined
              ? initialChangedCells[cellKey]
              : originalValue;
          })
        );
        setChangedCells(initialChangedCells);
      }

      setMapValues(initialValues);
      // Restore persisted axis labels when the parent has any for this map.
      // The parent stores labels in FILE order; convert to display order
      // using the extracted flip flags before placing them in state.
      const toDisplayOrder = (labels: string[], reversed: boolean): string[] =>
        reversed ? [...labels].reverse() : labels;
      const restoredX = initialXAxisLabels && initialXAxisLabels.length === extractedData.xAxisLabels.length
        ? toDisplayOrder(initialXAxisLabels, extractedData.colsReversed)
        : extractedData.xAxisLabels;
      const restoredY = initialYAxisLabels && initialYAxisLabels.length === extractedData.yAxisLabels.length
        ? toDisplayOrder(initialYAxisLabels, extractedData.rowsReversed)
        : extractedData.yAxisLabels;
      setXAxisLabels(restoredX);
      setYAxisLabels(restoredY);
      setAxesSwapped(extractedData.axesSwapped);
      setCurrentMapAddress(mapData.address); // Mark as ready
    }
    // Intentionally omit initialXAxisLabels / initialYAxisLabels from deps:
    // we don't want this effect to run when the parent clears its persisted
    // axis-label store after a save (which would overwrite the user's edits
    // with the original file values). Initial axis labels are picked up on
    // first mount / map switch from the freshly-bound prop above. A separate
    // effect below handles parent-pushed updates *after* mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [extractedData, mapData.address, initialChangedCells]);

  // Track which map's labels have been initialized so the post-mount sync
  // below only fires when the parent actively pushes a *new* set of labels.
  const lastAppliedInitialXRef = useRef<string>("");
  const lastAppliedInitialYRef = useRef<string>("");
  useEffect(() => {
    if (!extractedData) return;
    // Parent labels arrive in file order; convert to display order before
    // setting state, mirroring the conversion in the init effect above.
    const toDisplay = (labels: string[], reversed: boolean): string[] =>
      reversed ? [...labels].reverse() : labels;
    const xSig = initialXAxisLabels ? initialXAxisLabels.join('|') : '';
    if (xSig && xSig !== lastAppliedInitialXRef.current
        && initialXAxisLabels!.length === extractedData.xAxisLabels.length) {
      lastAppliedInitialXRef.current = xSig;
      setXAxisLabels(toDisplay(initialXAxisLabels!, extractedData.colsReversed));
    }
    const ySig = initialYAxisLabels ? initialYAxisLabels.join('|') : '';
    if (ySig && ySig !== lastAppliedInitialYRef.current
        && initialYAxisLabels!.length === extractedData.yAxisLabels.length) {
      lastAppliedInitialYRef.current = ySig;
      setYAxisLabels(toDisplay(initialYAxisLabels!, extractedData.rowsReversed));
    }
  }, [initialXAxisLabels, initialYAxisLabels, extractedData]);

  // Ref pour éviter la boucle infinie
  const isUpdatingFromParent = useRef(false);
  const lastNotifiedChanges = useRef<string>('{}');
  const hasMountedRef = useRef(false);
  const lastProcessedInitialChanges = useRef<string>('{}');

  // Mettre à jour changedCells quand initialChangedCells change (vient du parent) APRÈS le montage initial
  useEffect(() => {
    // Ne s'exécute que si on a déjà monté le composant (pour éviter de dupliquer le travail du premier useEffect)
    if (!hasMountedRef.current) {
      hasMountedRef.current = true;
      return;
    }

    // Comparer avec la dernière valeur traitée pour éviter les re-renders inutiles
    const initialChangesStr = JSON.stringify(initialChangedCells || {});

    // Si le contenu est identique à ce qu'on a déjà traité, ne rien faire
    if (initialChangesStr === lastProcessedInitialChanges.current) {
      return;
    }


    // Mettre à jour la ref avec la nouvelle valeur
    lastProcessedInitialChanges.current = initialChangesStr;

    const currentChangesStr = JSON.stringify(changedCells);

    // Seulement mettre à jour si c'est vraiment différent de l'état local actuel
    if (initialChangesStr !== currentChangesStr) {
      isUpdatingFromParent.current = true;
      setChangedCells(initialChangedCells || {});

      // Appliquer les modifications aux valeurs affichées
      if (originalValuesRef.current) {
        if (initialChangedCells && Object.keys(initialChangedCells).length > 0) {
          const updatedMapValues = originalValuesRef.current.map((row, rowIndex) =>
            row.map((originalValue, colIndex) => {
              const cellKey = `${rowIndex}-${colIndex}`;
              return initialChangedCells[cellKey] !== undefined
                ? initialChangedCells[cellKey]
                : originalValue;
            })
          );
          setMapValues(updatedMapValues);
        } else {
          // Si pas de modifications, revenir aux valeurs originales
          setMapValues(originalValuesRef.current.map(row => [...row]));
        }
      }

      setTimeout(() => {
        isUpdatingFromParent.current = false;
      }, 0);
    }
  }, [initialChangedCells, mapData.address]);

  // Notifier le parent quand les modifications changent (mais pas si ça vient du parent)
  useEffect(() => {
    const changesStr = JSON.stringify(changedCells);

    if (onModificationsChange && !isUpdatingFromParent.current && changesStr !== lastNotifiedChanges.current) {
      lastNotifiedChanges.current = changesStr;
      onModificationsChange(changedCells);
    }
  }, [changedCells, onModificationsChange]);

  // Notify the parent of axis-label edits so they survive map close/reopen.
  // We always send labels in FILE order (not display order). Display order
  // can be reversed by extractedData (e.g. RPM descending), so we undo that
  // reversal here. The parent stores file-ordered labels, which lets the
  // exporter write them back without needing to know about display flips.
  const lastNotifiedAxisRef = useRef<string>("");
  useEffect(() => {
    if (!onAxisLabelsChange) return;
    if (xAxisLabels.length === 0 && yAxisLabels.length === 0) return;
    const origX = originalXAxisLabelsRef.current;
    const origY = originalYAxisLabelsRef.current;
    if (origX.length === 0 && origY.length === 0) return;

    const xChanged = xAxisLabels.length === origX.length
      && xAxisLabels.some((v, i) => v !== origX[i]);
    const yChanged = yAxisLabels.length === origY.length
      && yAxisLabels.some((v, i) => v !== origY[i]);

    // Convert display-ordered labels back to file order using extractedData's
    // flip flags. If a flag is true, display index 0 corresponds to file
    // index N-1, so we reverse the array.
    const toFileOrder = (labels: string[], reversed: boolean): string[] =>
      reversed ? [...labels].reverse() : labels;
    const xFileOrder = extractedData?.colsReversed
      ? toFileOrder(xAxisLabels, true)
      : xAxisLabels;
    const yFileOrder = extractedData?.rowsReversed
      ? toFileOrder(yAxisLabels, true)
      : yAxisLabels;

    const payload: { x?: string[]; y?: string[] } = {};
    if (xChanged) payload.x = xFileOrder;
    if (yChanged) payload.y = yFileOrder;

    const sig = `${xChanged ? xFileOrder.join('|') : ''}::${yChanged ? yFileOrder.join('|') : ''}`;
    if (sig === lastNotifiedAxisRef.current) return;
    lastNotifiedAxisRef.current = sig;
    onAxisLabelsChange(payload);
  }, [xAxisLabels, yAxisLabels, onAxisLabelsChange, extractedData]);

  // Ajuster la position du menu contextuel pour qu'il reste dans l'écran
  useLayoutEffect(() => {
    if (!contextMenu || !contextMenuRef.current) {
      setAdjustedContextMenuPos(null);
      return;
    }

    const menu = contextMenuRef.current;
    const menuRect = menu.getBoundingClientRect();
    const padding = 8; // Marge avec le bord de l'écran

    let adjustedX = contextMenu.x;
    let adjustedY = contextMenu.y;

    // Vérifier le débordement à droite
    if (contextMenu.x + menuRect.width + padding > window.innerWidth) {
      adjustedX = window.innerWidth - menuRect.width - padding;
    }

    // Vérifier le débordement en bas
    if (contextMenu.y + menuRect.height + padding > window.innerHeight) {
      adjustedY = window.innerHeight - menuRect.height - padding;
    }

    // Vérifier le débordement à gauche
    if (adjustedX < padding) {
      adjustedX = padding;
    }

    // Vérifier le débordement en haut
    if (adjustedY < padding) {
      adjustedY = padding;
    }

    setAdjustedContextMenuPos({ x: adjustedX, y: adjustedY });
  }, [contextMenu]);

  // Fermer le menu contextuel lors d'un clic n'importe où (sauf sur le menu lui-même)
  useEffect(() => {
    if (!contextMenu) return;

    const handleGlobalClick = (e: MouseEvent) => {
      // Vérifier si le clic est sur le menu contextuel lui-même
      if (contextMenuRef.current && contextMenuRef.current.contains(e.target as Node)) {
        return; // Ne pas fermer si on clique sur le menu
      }
      setContextMenu(null);
    };

    // Utiliser capture pour intercepter le clic avant qu'il ne soit traité par d'autres éléments
    document.addEventListener('mousedown', handleGlobalClick, true);

    return () => {
      document.removeEventListener('mousedown', handleGlobalClick, true);
    };
  }, [contextMenu]);

  const handleCellMouseDown = (displayRow: number, displayCol: number, e: React.MouseEvent) => {
    e.preventDefault();
    // Convert display coords (from DOM event) into mapValues coords so
    // selectedCells / dragStart / etc. stay in the same space as mapValues.
    const { row, col } = toMapCoords(displayRow, displayCol);
    const cellKey = getCellKey(row, col);
    keyboardCursorRef.current = { row: displayRow, col: displayCol };

    // Clic droit: ne pas modifier la sélection si la cellule est déjà sélectionnée
    // (le menu contextuel sera géré par handleContextMenu)
    if (e.button === 2) {
      if (selectedCells.has(cellKey)) {
        return; // Garder la sélection actuelle
      }
      // Si la cellule n'est pas sélectionnée, la sélectionner seule
      setSelectedCells(new Set([cellKey]));
      return;
    }

    // Désélectionner les cellules d'axes quand on clique sur une cellule de données
    setSelectedXAxisCells(new Set());
    setSelectedYAxisCells(new Set());

    if (e.ctrlKey || e.metaKey) {
      // Ctrl+clic: mode sélection additive avec glissement
      setIsCtrlDragging(true);
      setInitialSelection(new Set(selectedCells)); // Sauvegarder la sélection actuelle
      setIsDragging(true);
      setDragStart({ row, col });
      setHasMovedDuringDrag(false); // Réinitialiser le flag de mouvement
    } else {
      // Clic simple: démarrer une nouvelle sélection
      setIsCtrlDragging(false);
      setSelectedCells(new Set([cellKey]));
      setIsDragging(true);
      setDragStart({ row, col });
      setHasMovedDuringDrag(false);
    }
  };

  const handleCellMouseEnter = (displayRow: number, displayCol: number) => {
    if (!isDragging || !dragStart) return;
    const { row, col } = toMapCoords(displayRow, displayCol);

    // Vérifier si on a bougé de la cellule de départ
    const hasMoved = dragStart.row !== row || dragStart.col !== col;

    if (hasMoved) {
      setHasMovedDuringDrag(true);
    }

    // Si mode Ctrl et qu'on n'a pas encore bougé, ne rien faire
    // (on attendra le mouseUp pour toggle la cellule)
    if (isCtrlDragging && !hasMoved) {
      return;
    }

    // Calculer la zone rectangulaire entre dragStart et la cellule actuelle
    const minRow = Math.min(dragStart.row, row);
    const maxRow = Math.max(dragStart.row, row);
    const minCol = Math.min(dragStart.col, col);
    const maxCol = Math.max(dragStart.col, col);

    const newSelection = new Set<string>();
    for (let r = minRow; r <= maxRow; r++) {
      for (let c = minCol; c <= maxCol; c++) {
        newSelection.add(getCellKey(r, c));
      }
    }

    // Si mode Ctrl, fusionner avec la sélection initiale
    if (isCtrlDragging) {
      const combined = new Set(initialSelection);
      newSelection.forEach(key => combined.add(key));
      setSelectedCells(combined);
    } else {
      setSelectedCells(newSelection);
    }
  };

  const handleContextMenu = (e: React.MouseEvent, displayRow: number, displayCol: number, value: number) => {
    e.preventDefault();
    const { row, col } = toMapCoords(displayRow, displayCol);
    const cellKey = getCellKey(row, col);

    // Si la cellule n'est pas dans la sélection, la sélectionner seule
    if (!selectedCells.has(cellKey)) {
      setSelectedCells(new Set([cellKey]));
    }

    // Positionner le menu juste en dessous de la cellule
    const cellRect = e.currentTarget.getBoundingClientRect();
    setContextMenu({ x: cellRect.left, y: cellRect.bottom, type: 'cell', row, col, value });
  };

  // Context menu pour l'axe X
  const handleXAxisContextMenu = (e: React.MouseEvent, index: number) => {
    e.preventDefault();
    if (!selectedXAxisCells.has(index)) {
      setSelectedXAxisCells(new Set([index]));
    }
    // Positionner le menu juste en dessous de la cellule
    const cellRect = e.currentTarget.getBoundingClientRect();
    setContextMenu({ x: cellRect.left, y: cellRect.bottom, type: 'xAxis', index });
  };

  // Context menu pour l'axe Y
  const handleYAxisContextMenu = (e: React.MouseEvent, index: number) => {
    e.preventDefault();
    if (!selectedYAxisCells.has(index)) {
      setSelectedYAxisCells(new Set([index]));
    }
    // Positionner le menu juste en dessous de la cellule
    const cellRect = e.currentTarget.getBoundingClientRect();
    setContextMenu({ x: cellRect.left, y: cellRect.bottom, type: 'yAxis', index });
  };

  const updateCellValue = (row: number, col: number, newValue: number) => {
    setMapValues((prev) => {
      const next = prev.map((r, rIdx) =>
        r.map((v, cIdx) => (rIdx === row && cIdx === col ? newValue : v)),
      );
      return next;
    });
    setChangedCells((prev) => {
      const key = `${row}-${col}`;
      const next = { ...prev };
      const original = originalValuesRef.current?.[row]?.[col];
      if (original !== undefined && Math.abs(original - newValue) < 1e-6) {
        delete next[key];
      } else {
        next[key] = newValue;
      }
      return next;
    });
  };

  const handlePromptEdit = (displayRow: number, displayCol: number, value: number) => {
    setValuePrompt({
      title: t.mapViewer.editCellTitle,
      value: value.toFixed(2),
      onSubmit: (input: string) => {
        const parsed = Number(input.replace(",", "."));
        if (Number.isNaN(parsed)) {
          toast({ title: t.errors.invalidValue, description: t.errors.invalidValueDescription, variant: "destructive" });
          return;
        }
        const { row, col } = toMapCoords(displayRow, displayCol);
        updateCellValue(row, col, parsed);
      },
    });
  };

  // Handlers pour la sélection des cellules d'axes X
  const handleXAxisMouseDown = (index: number, e: React.MouseEvent) => {
    e.preventDefault();

    // Clic droit: ne pas modifier la sélection si la cellule est déjà sélectionnée
    if (e.button === 2) {
      if (selectedXAxisCells.has(index)) {
        return; // Garder la sélection actuelle
      }
      // Si la cellule n'est pas sélectionnée, la sélectionner seule
      setSelectedCells(new Set());
      setSelectedYAxisCells(new Set());
      setSelectedXAxisCells(new Set([index]));
      return;
    }

    // Désélectionner les cellules normales et l'axe Y
    setSelectedCells(new Set());
    setSelectedYAxisCells(new Set());

    if (e.ctrlKey || e.metaKey) {
      // Mode additive
      setSelectedXAxisCells(prev => {
        const next = new Set(prev);
        if (next.has(index)) {
          next.delete(index);
        } else {
          next.add(index);
        }
        return next;
      });
    } else {
      setSelectedXAxisCells(new Set([index]));
    }
    setIsAxisDragging(true);
    setAxisDragStart({ axis: 'x', index });
  };

  const handleXAxisMouseEnter = (index: number) => {
    if (!isAxisDragging || !axisDragStart || axisDragStart.axis !== 'x') return;

    const minIdx = Math.min(axisDragStart.index, index);
    const maxIdx = Math.max(axisDragStart.index, index);
    const newSelection = new Set<number>();
    for (let i = minIdx; i <= maxIdx; i++) {
      newSelection.add(i);
    }
    setSelectedXAxisCells(newSelection);
  };

  // Handlers pour la sélection des cellules d'axes Y
  const handleYAxisMouseDown = (index: number, e: React.MouseEvent) => {
    e.preventDefault();

    // Clic droit: ne pas modifier la sélection si la cellule est déjà sélectionnée
    if (e.button === 2) {
      if (selectedYAxisCells.has(index)) {
        return; // Garder la sélection actuelle
      }
      // Si la cellule n'est pas sélectionnée, la sélectionner seule
      setSelectedCells(new Set());
      setSelectedXAxisCells(new Set());
      setSelectedYAxisCells(new Set([index]));
      return;
    }

    // Désélectionner les cellules normales et l'axe X
    setSelectedCells(new Set());
    setSelectedXAxisCells(new Set());

    if (e.ctrlKey || e.metaKey) {
      // Mode additive
      setSelectedYAxisCells(prev => {
        const next = new Set(prev);
        if (next.has(index)) {
          next.delete(index);
        } else {
          next.add(index);
        }
        return next;
      });
    } else {
      setSelectedYAxisCells(new Set([index]));
    }
    setIsAxisDragging(true);
    setAxisDragStart({ axis: 'y', index });
  };

  const handleYAxisMouseEnter = (index: number) => {
    if (!isAxisDragging || !axisDragStart || axisDragStart.axis !== 'y') return;

    const minIdx = Math.min(axisDragStart.index, index);
    const maxIdx = Math.max(axisDragStart.index, index);
    const newSelection = new Set<number>();
    for (let i = minIdx; i <= maxIdx; i++) {
      newSelection.add(i);
    }
    setSelectedYAxisCells(newSelection);
  };

  // Number of decimals to display for an axis label, matching the format
  // used when the labels were first read from the file (see tempXLabels /
  // tempYLabels construction in the extractedData useMemo).
  const axisDecimals = (correction: number | undefined, precisionOverride?: number): number => {
    if (typeof precisionOverride === 'number' && isFinite(precisionOverride) && precisionOverride >= 0) {
      return Math.min(6, Math.trunc(precisionOverride));
    }
    // Drivers wish MJD6 : axes % et RPM entiers par construction — jamais de
    // ".00" (même règle que le formatage de lecture dans extractedData).
    if (mapNameLowerDisplay.includes("drivers wish") && hasUnsignedAxes(ecuType)) {
      return 0;
    }
    const c = correction ?? 1.0;
    return c < 0.1 ? 2 : (c < 1.0 ? 1 : 0);
  };

  // Effective display corrections/precisions: per-project overrides from the
  // map Properties window win over the detected values (same rules as the
  // decoding in the extractedData useMemo above).
  const effAxisFactor = (ds?: { factor?: number; divisor?: number }): number | undefined => {
    if (!ds || typeof ds.factor !== 'number' || !isFinite(ds.factor)) return undefined;
    const div = typeof ds.divisor === 'number' && isFinite(ds.divisor) && ds.divisor !== 0 ? ds.divisor : 1;
    return ds.factor / div;
  };
  // Corrections des axes SOURCE (xAxisLabels/yAxisLabels) : la lecture échange
  // les corrections backend quand axesSwapped (ex. Drivers wish stockée
  // [pedal][rpm] → l'axe X source contient la pédale). Sans cet échange, la
  // re-normalisation appliquait la correction du mauvais axe (RPM formaté avec
  // le facteur pédale 0.004 → heuristique "2 décimales" → "5700.00").
  const effXAxisCorrection = effAxisFactor(displaySettings?.xAxis)
    ?? (axesSwapped ? mapData.y_axis_correction : mapData.x_axis_correction);
  const effYAxisCorrection = effAxisFactor(displaySettings?.yAxis)
    ?? (axesSwapped ? mapData.x_axis_correction : mapData.y_axis_correction);
  const xAxisPrecisionOverride = displaySettings?.xAxis?.precision;
  const yAxisPrecisionOverride = displaySettings?.yAxis?.precision;
  // Axes AFFICHÉS : la transposition (bouton d'inversion) échange les tableaux
  // de labels X/Y — les corrections/précisions de formatage suivent.
  const effDisplayXCorrection = displayTransposed ? effYAxisCorrection : effXAxisCorrection;
  const effDisplayYCorrection = displayTransposed ? effXAxisCorrection : effYAxisCorrection;
  const displayXPrecisionOverride = displayTransposed ? yAxisPrecisionOverride : xAxisPrecisionOverride;
  const displayYPrecisionOverride = displayTransposed ? xAxisPrecisionOverride : yAxisPrecisionOverride;
  // Précision par défaut selon le pas de la carte : un facteur très fin
  // (ex. pompe N146 en volts, 0.001221/bit) demande 2 décimales — à une
  // seule, 1.99 V s'affichait « 1.9 ».
  const cellFactorAbs = Math.abs(mapData?.correction_factor ?? 1);
  const defaultCellDecimals = cellFactorAbs > 0 && cellFactorAbs < 0.01 ? 2 : 1;
  const cellDecimals =
    typeof displaySettings?.map?.precision === 'number' &&
    isFinite(displaySettings.map.precision) &&
    displaySettings.map.precision >= 0
      ? Math.min(6, Math.trunc(displaySettings.map.precision))
      : defaultCellDecimals;

  // Format a user-entered axis label so it matches the surrounding labels
  // (e.g. "55" -> "55.00" when other labels are "10.00", "15.00", ...).
  // Falls back to the raw input if it's not a number.
  const formatAxisInput = (input: string, correction: number | undefined, precisionOverride?: number): string => {
    const trimmed = input.trim().replace(',', '.');
    const n = Number(trimmed);
    if (!Number.isFinite(n)) return input;
    return n.toFixed(axisDecimals(correction, precisionOverride));
  };

  // Normalize any axis label for display, regardless of which code path
  // wrote it (initial read from file, +/-1 increments, paste, absolute
  // change, etc.). This guarantees consistent decimals across all entries.
  const formatAxisDisplay = (label: string, correction: number | undefined, precisionOverride?: number): string => {
    if (label == null) return label;
    const n = Number(label.trim().replace(',', '.'));
    if (!Number.isFinite(n)) return label;
    return n.toFixed(axisDecimals(correction, precisionOverride));
  };

  // Édition des valeurs d'axe X (double-clic)
  const handleEditXAxisLabel = (index: number, currentValue: string) => {
    setValuePrompt({
      title: t.mapViewer.editXAxisTitle,
      value: currentValue,
      onSubmit: (input: string) => {
        const formatted = formatAxisInput(input, effXAxisCorrection, xAxisPrecisionOverride);
        setXAxisLabels(prev => {
          const next = [...prev];
          next[index] = formatted;
          return next;
        });
      },
    });
  };

  // Édition des valeurs d'axe Y (double-clic)
  const handleEditYAxisLabel = (index: number, currentValue: string) => {
    setValuePrompt({
      title: t.mapViewer.editYAxisTitle,
      value: currentValue,
      onSubmit: (input: string) => {
        const formatted = formatAxisInput(input, effYAxisCorrection, yAxisPrecisionOverride);
        setYAxisLabels(prev => {
          const next = [...prev];
          next[index] = formatted;
          return next;
        });
      },
    });
  };

  // Édition d'un libellé d'axe depuis la position AFFICHÉE. Quand la vue est
  // transposée (bouton d'inversion), l'en-tête de colonne affichée pointe en
  // réalité vers l'axe Y source et inversement. On traduit via
  // displayXAxisToSource / displayYAxisToSource pour éditer le bon axe au bon
  // index source (transpose + miroirs pris en compte).
  const handleEditDisplayXAxis = (displayCol: number) => {
    const tgt = displayXAxisToSource(displayCol);
    if (tgt.axis === 'x') handleEditXAxisLabel(tgt.index, xAxisLabels[tgt.index]);
    else handleEditYAxisLabel(tgt.index, yAxisLabels[tgt.index]);
  };
  const handleEditDisplayYAxis = (displayRow: number) => {
    const tgt = displayYAxisToSource(displayRow);
    if (tgt.axis === 'x') handleEditXAxisLabel(tgt.index, xAxisLabels[tgt.index]);
    else handleEditYAxisLabel(tgt.index, yAxisLabels[tgt.index]);
  };

  const handleRestoreOriginal = (row: number, col: number) => {
    const original = originalValuesRef.current?.[row]?.[col];
    if (original === undefined) return;
    updateCellValue(row, col, original);
  };

  const handleSaveEdits = async () => {
    const diffEntries = Object.entries(changedCells).map(([key, value]) => {
      const [row, col] = key.split("-").map(Number);
      return { row, col, value };
    });
    if (diffEntries.length === 0) {
      toast({ title: t.errors.noModifications, description: t.errors.noModificationsDescription });
      return;
    }
    if (!currentVersionId) {
      toast({
        title: t.errors.missingVersion,
        description: t.errors.missingVersion,
        variant: "destructive",
      });
      return;
    }
    try {
      await axios.post("/api/versioning/map-edits", {
        versionId: currentVersionId,
        mapAddress: mapData.address,
        payload: {
          changedCells: diffEntries,
          xAxisLabels,
          yAxisLabels,
        },
      });
      toast({ title: t.errors.modificationsRecorded, description: `${diffEntries.length} ${t.errors.cellsSaved}` });
    } catch (error: any) {
      console.error("❌ [FRONTEND] save edits error", error?.message || error);
      toast({
        title: t.errors.saveFailed2,
        description: t.errors.saveFailed,
        variant: "destructive",
      });
    }
  };
  
  // Check if map is single-line (1D) - exactly 1 row with multiple columns
  // 2D view is ONLY available for single-line maps
  const isSingleLineMap = displayMapValues.length === 1 && (displayMapValues[0]?.length || 0) > 1;
  
  // 3D view is available for multi-line maps (more than 1 row)
  const canShow3D = displayMapValues.length > 1;
  const modificationsCount = Object.keys(changedCells).length;
  
  // Handle view mode change - notify parent if controlled
  const handleViewModeChange = (newMode: ViewMode) => {
    // Block 3D view for single-line maps
    if (newMode === "3d" && !canShow3D) return;
    // Block 2D view for non single-line maps
    if (newMode === "2d" && !isSingleLineMap) return;
    setViewMode(newMode);
  };

  // Mémoriser les données 3D pour éviter les recalculs coûteux
  const plot3DData = useMemo(() => {
    // Check if Y axis is descending and needs to be reversed for 3D display
    const needsYReverse = displayYAxisLabels.length > 0 && parseFloat(displayYAxisLabels[0]) > parseFloat(displayYAxisLabels[displayYAxisLabels.length - 1]);
    const processedY = needsYReverse
      ? [...displayYAxisLabels].reverse().map((label: string) => parseFloat(label))
      : displayYAxisLabels.map((label: string) => parseFloat(label));
    const processedZ = needsYReverse
      ? [...displayMapValues].reverse()
      : displayMapValues;
    const xValues = displayXAxisLabels.map((label: string) => parseFloat(label));

    // Coordonnées = INDICES de cellule (espacement UNIFORME façon WinOLS).
    // Avec les vraies valeurs en coordonnées, un axe qui saute (ex. RPM
    // 21 → 1008) créait une bande géante entre deux lignes du maillage.
    // Les vraies valeurs restent visibles : étiquettes d'axes (ticktext,
    // voir buildPlot3DTicks) et infobulle (text).
    const xIdx = xValues.map((_: number, i: number) => i);
    const yIdx = processedY.map((_: number, i: number) => i);
    const units = parseAxisUnits();
    const hoverText = processedZ.map((row: number[], yi: number) =>
      row.map((_: number, xi: number) => `${units.xLabel}: ${xValues[xi]}<br>${units.yLabel}: ${processedY[yi]}`)
    );

    // Calculer un offset pour placer les lignes légèrement au-dessus de la surface
    const allValues = processedZ.flat();
    const zRange = Math.max(...allValues) - Math.min(...allValues);
    const zOffset = zRange * 0.001;

    return [
      // Surface principale
      {
        z: processedZ,
        x: xIdx,
        y: yIdx,
        text: hoverText,
        type: "surface" as const,
        colorscale: disableGraphColors
          ? [
              [0, theme === 'light' ? 'rgb(180, 180, 180)' : 'rgb(60, 60, 60)'],
              [1, theme === 'light' ? 'rgb(100, 100, 100)' : 'rgb(140, 140, 140)']
            ]
          : [
              [0, transformColorForTheme(0, 55, 240, theme)],      // Bleu - identique au tableau
              [0.25, transformColorForTheme(0, 185, 0, theme)],    // Vert - identique au tableau
              [0.5, transformColorForTheme(200, 165, 0, theme)],   // Jaune - identique au tableau
              [0.75, transformColorForTheme(220, 120, 0, theme)],  // Orange - identique au tableau
              [1, transformColorForTheme(250, 0, 0, theme)]        // Rouge - identique au tableau
            ],
        hovertemplate: `%{text}<br>Value: %{z:.2f}<extra></extra>`,
        showscale: false,
        contours: { z: { show: false } },
      },
      // Lignes blanches pour chaque valeur de Y (lignes horizontales)
      ...(displayMapValues.length > 0 && displayMapValues[0].length > 0
        ? processedY.map((_yVal: number, yi: number) => ({
            x: xIdx,
            y: new Array(xIdx.length).fill(yi),
            z: processedZ[yi].map((v: number) => v + zOffset),
            type: "scatter3d" as const,
            mode: "lines" as const,
            line: {
              color: "#ffffff",
              width: 1.3,
            },
            showlegend: false,
            hoverinfo: "skip" as const,
            connectgaps: true,
          }))
        : []),
      // Lignes blanches pour chaque valeur de X (lignes verticales)
      ...(displayMapValues.length > 0 && displayMapValues[0].length > 0
        ? xValues.map((_xVal: number, xi: number) => ({
            x: new Array(yIdx.length).fill(xi),
            y: yIdx,
            z: processedZ.map((row: number[]) => row[xi] + zOffset),
            type: "scatter3d" as const,
            mode: "lines" as const,
            line: {
              color: "#ffffff",
              width: 1.3,
            },
            showlegend: false,
            hoverinfo: "skip" as const,
            connectgaps: true,
          }))
        : []),
    ];
  }, [displayMapValues, displayXAxisLabels, displayYAxisLabels, theme, disableGraphColors]);

  // Étiquettes d'axes (indices → vraies valeurs) pour les deux layouts 3D
  const plot3DTicks = useMemo(
    () => buildPlot3DTicks(displayXAxisLabels, displayYAxisLabels),
    [displayXAxisLabels, displayYAxisLabels]
  );

  // Notifier le parent des données 3D quand elles changent (pour Preview window).
  // We must NOT depend on `plot3DData`/`displayXAxisLabels`/`displayYAxisLabels`
  // directly — those are fresh array references on every render of MapViewer,
  // and the parent's handler bumps a state that re-renders us, which would
  // create an infinite update loop and break the 3D preview's mouse
  // controls. Instead, derive a content signature and only fire when it
  // actually changes.
  const lastPlot3DSignatureRef = useRef<string>("");
  const plot3DSignature = useMemo(() => {
    if (!onPlot3DDataChange) return "";
    const xs = displayXAxisLabels.join("|");
    const ys = displayYAxisLabels.join("|");
    // Hash every cell so single-cell edits invalidate the signature.
    // FNV-1a 32-bit on the row-major float bit pattern: cheap, ~10ns per
    // cell on a hot path, no allocations.
    let hash = 0x811c9dc5 >>> 0;
    const buf = new ArrayBuffer(8);
    const floats = new Float64Array(buf);
    const ints = new Uint32Array(buf);
    let rows = 0;
    let cols = 0;
    if (displayMapValues.length > 0) {
      rows = displayMapValues.length;
      cols = displayMapValues[0]?.length ?? 0;
      for (let r = 0; r < rows; r++) {
        const row = displayMapValues[r];
        if (!row) continue;
        for (let c = 0; c < cols; c++) {
          floats[0] = row[c] ?? 0;
          hash = Math.imul(hash ^ ints[0], 0x01000193);
          hash = Math.imul(hash ^ ints[1], 0x01000193);
        }
      }
    }
    // theme et disableGraphColors changent la colorscale du plot3DData émis :
    // sans eux dans la signature, la Preview gardait l'ancienne surface
    // colorée quand on basculait « Disable 3D colors » (ou de thème).
    return `${mapData.address}|${rows}x${cols}|${xs}|${ys}|${hash.toString(16)}|${theme}|${disableGraphColors ? 1 : 0}`;
  }, [mapData.address, displayXAxisLabels, displayYAxisLabels, displayMapValues, onPlot3DDataChange, theme, disableGraphColors]);

  useEffect(() => {
    if (!onPlot3DDataChange) return;
    if (lastPlot3DSignatureRef.current === plot3DSignature) return;
    lastPlot3DSignatureRef.current = plot3DSignature;

    onPlot3DDataChange(mapData.address, {
      plot3DData,
      xAxisLabels: displayXAxisLabels,
      yAxisLabels: displayYAxisLabels,
      canShow3D: displayMapValues.length > 1,
    });
    // plot3DData / display* are intentionally NOT in the deps: the signature
    // above gates whether the notification needs to happen.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [plot3DSignature, mapData.address, onPlot3DDataChange]);

  // Mode 3D: structure alignée au layout principal
  if (viewMode === "3d") {
    // Calcul du centre de rotation (milieu des axes et des valeurs)
    const center = {
      x: xAxisLabels.length
        ? (parseFloat(xAxisLabels[0]) + parseFloat(xAxisLabels[xAxisLabels.length - 1])) / 2
        : 0,
      y: yAxisLabels.length
        ? (parseFloat(yAxisLabels[0]) + parseFloat(yAxisLabels[yAxisLabels.length - 1])) / 2
        : 0,
      z:
        displayMapValues.length > 0 && displayMapValues[0].length > 0
          ? (() => {
              let min = displayMapValues[0][0];
              let max = displayMapValues[0][0];
              for (const row of displayMapValues) {
                for (const v of row) {
                  if (v < min) min = v;
                  if (v > max) max = v;
                }
              }
              return (min + max) / 2;
            })()
          : 0,
    };

    return (
      <div className="flex flex-col h-full bg-transparent relative">
        {/* Poignée de redimensionnement custom (coin bas-droit) */}
        <div
          className="absolute bottom-0 right-0 w-4 h-4 cursor-se-resize z-30"
          style={{ background: 'linear-gradient(135deg, transparent 55%, rgba(128, 128, 128, 0.55) 55%)' }}
          onMouseDown={handleResizeHandleMouseDown}
          title="Redimensionner"
        />
          {/* Map Title Bar - Largeur du contenu uniquement */}
      <div
        className="w-full px-4 py-0.5 pl-[10px] relative cursor-move select-none"
        style={{
          background: getWindowHeaderBg(),
          borderBottom: `1px solid ${getCellBorderColor()}`
        }}
        onMouseDown={(e) => {
          if (e.button !== 0) return; // drag uniquement au clic gauche
          onDragStart?.(e);
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          setHeaderContextMenu({ x: e.clientX, y: e.clientY });
        }}
      >
        <div className="flex items-center gap-2 pr-8 min-w-0">
          <h3 className="text-[13px] font-medium truncate" title={mapData.codeblock_id != null ? `${mapData.name} [codeblock ${mapData.codeblock_id}]` : mapData.name} style={{ color: getWindowHeaderTextColor() }}>
            {mapData.name}
            {mapData.codeblock_id != null && (
              <span className="font-normal opacity-60"> [codeblock {mapData.codeblock_id}]</span>
            )}
          </h3>
          <span className="text-[11px] pl-[10px] whitespace-nowrap flex-shrink-0" style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.6)' : 'rgba(255, 255, 255, 0.6)' }}>
            {displayXAxisLabels.length}x{displayYAxisLabels.length}
          </span>
          {onToggleInvertDisplay && (
            <button
              className={`h-6 w-6 p-0 flex items-center justify-center rounded flex-shrink-0 transition-all ${invertButtonActive ? 'bg-gradient-to-r from-red-600/50 via-red-500/50 to-orange-500/50 text-white border border-red-500/40 shadow-sm shadow-red-500/30' : 'hover:bg-white/10'}`}
              style={!invertButtonActive ? { color: theme === 'light' ? 'rgba(0, 0, 0, 0.55)' : 'rgba(255, 255, 255, 0.55)' } : undefined}
              onClick={handleToggleInvert}
              onMouseDown={(e) => e.stopPropagation()}
              title={invertButtonActive ? "Rétablir l'orientation du fichier (lignes ↔ colonnes)" : "Inverser l'affichage (lignes ↔ colonnes)"}
            >
              <Repeat2 className="w-4 h-4" strokeWidth={2.5} />
            </button>
          )}
        </div>
        {mapData.description && (
          <p className="text-[11px] mt-0.5 truncate pr-8" style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)' }}>{cleanDescription(mapData.description)}</p>
        )}
        {onClose && (
          <Button
            variant="ghost"
            size="sm"
            className="h-6 w-6 p-0 flex items-center justify-center hover:bg-red-600/20 hover:text-red-400 absolute right-3 top-1/2 -translate-y-1/2"
            style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.7)' : 'rgba(255, 255, 255, 0.7)' }}
            onClick={handleClose}
            onMouseDown={(e) => e.stopPropagation()}
            title="Fermer cette map"
          >
            <X className="w-4 h-4" />
          </Button>
        )}

        {/* Header Context Menu */}
        {headerContextMenu && (
          <div
            ref={headerContextMenuRef}
            style={{
              position: 'fixed',
              left: headerContextMenu.x,
              top: headerContextMenu.y,
              background: 'rgba(22, 25, 34, 0.92)',
              border: '1px solid rgba(255, 255, 255, 0.1)',
              backdropFilter: 'blur(18px) saturate(140%)',
              WebkitBackdropFilter: 'blur(18px) saturate(140%)',
              color: '#ffffff',
              zIndex: 9999,
            }}
            className="rounded-lg shadow-lg p-1.5 flex flex-col gap-0.5 text-[12px] min-w-[160px]"
            onClick={(e) => e.stopPropagation()}
            onMouseDown={(e) => e.stopPropagation()}
          >
            {onViewInHexdump && (
              <button
                className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors flex items-center gap-2"
                onClick={() => {
                  onViewInHexdump();
                  setHeaderContextMenu(null);
                }}
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
                </svg>
                {t.mapViewer.viewInHexdump || 'View in Hexdump'}
              </button>
            )}
            {onOpenProperties && (
              <button
                className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors flex items-center gap-2"
                onClick={() => {
                  onOpenProperties();
                  setHeaderContextMenu(null);
                }}
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                </svg>
                {t.mapViewer.properties || 'Properties'}
              </button>
            )}
            {onClose && (
              <button
                className="px-3 py-1.5 text-left rounded hover:bg-red-600/20 hover:text-red-400 transition-colors flex items-center gap-2"
                onClick={() => {
                  handleClose();
                  setHeaderContextMenu(null);
                }}
              >
                <X className="w-4 h-4" />
                {t.common.close || 'Close'}
              </button>
            )}
          </div>
        )}
      </div>

          {/* Content Area */}
          <div className="flex-1 overflow-hidden bg-transparent relative">
            {currentMapAddress !== mapData.address ? (
              <div className="w-full h-full flex items-center justify-center">
                <div style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)' }}>{t.common.loading}</div>
              </div>
          ) : (
            <div className="absolute inset-0 w-full h-full">
              {/* @ts-ignore */}
              <Plot
                key={`3d-${mapData.address}`}
                data={plot3DData}
                layout={{
                  paper_bgcolor: "transparent",
                  plot_bgcolor: "transparent",
                  scene: {
                    xaxis: {
                      title: parseAxisUnits().xLabel + " (" + parseAxisUnits().xUnit + ")",
                      backgroundcolor: "transparent",
                      gridcolor: "#374151",
                      showbackground: true,
                      color: "#9ca3af",
                      tickmode: "array" as const,
                      tickvals: plot3DTicks.xTickVals,
                      ticktext: plot3DTicks.xTickText,
                    },
                    yaxis: {
                      title: parseAxisUnits().yLabel + " (" + parseAxisUnits().yUnit + ")",
                      backgroundcolor: "transparent",
                      gridcolor: "#374151",
                      showbackground: true,
                      color: "#9ca3af",
                      tickmode: "array" as const,
                      tickvals: plot3DTicks.yTickVals,
                      ticktext: plot3DTicks.yTickText,
                    },
                    zaxis: {
                      title: "Value",
                      backgroundcolor: "transparent",
                      gridcolor: "#374151",
                      showbackground: true,
                      color: "#9ca3af",
                    },
                    // Position de la caméra restaurée depuis savedCameraPositions (persiste entre les montages)
                    camera: cameraPosition,
                    // dragmode: Orbit par défaut (Plotly)
                    aspectmode: "manual",
                    aspectratio: { x: 1, y: 1, z: 0.7 },
                  },
                  margin: { t: 50, r: 0, b: 0, l: 0 },
                  autosize: true,
                  uirevision: mapData.address, // Préserver l'état UI (dont la caméra) tant que la map ne change pas
                }}
                config={{
                  displayModeBar: false,
                  displaylogo: false,
                  staticPlot: isDraggingWindow, // Freeze plot interactions during window drag for better performance
                }}
                style={{ width: "100%", height: "100%" }}
                useResizeHandler={true}
                onRelayout={handlePlotlyRelayout}
              />
            </div>
            )}

          {/* View Mode Tabs - alignés sur le conteneur principal */}
          <div
            className="absolute bottom-0 left-0 z-30 flex"
            style={{
              borderTop: `1px solid ${getCellBorderColor()}`,
              background: theme === 'light' ? '#f1f3f5' : '#1a1a1a'
            }}
          >
            <button
              onClick={() => handleViewModeChange("text")}
              className="px-3.5 py-1 text-[11px] leading-[14px] font-medium transition-colors"
              style={{
                borderRight: `1px solid ${getCellBorderColor()}`,
                background: getViewButtonBg(),
                color: getCellTextColor()
              }}
              onMouseEnter={(e) => e.currentTarget.style.background = getViewButtonBgHover()}
              onMouseLeave={(e) => e.currentTarget.style.background = getViewButtonBg()}
            >
              Text
            </button>
            <button
              onClick={() => handleViewModeChange("2d")}
              disabled={!isSingleLineMap}
              className="px-3.5 py-1 text-[11px] leading-[14px] font-medium transition-colors"
              style={{
                borderRight: `1px solid ${getCellBorderColor()}`,
                background: !isSingleLineMap ? (theme === 'light' ? '#e9ecef' : '#1a1a1a') : getViewButtonBg(),
                color: !isSingleLineMap ? (theme === 'light' ? 'rgba(0, 0, 0, 0.3)' : 'rgba(255, 255, 255, 0.3)') : getCellTextColor(),
                cursor: !isSingleLineMap ? 'not-allowed' : 'pointer'
              }}
              onMouseEnter={(e) => {
                if (isSingleLineMap) e.currentTarget.style.background = getViewButtonBgHover();
              }}
              onMouseLeave={(e) => {
                if (isSingleLineMap) e.currentTarget.style.background = getViewButtonBg();
              }}
            >
              2D
            </button>
            <button
              onClick={() => handleViewModeChange("3d")}
              disabled={!canShow3D}
              className="px-3.5 py-1 text-[11px] leading-[14px] font-medium transition-colors"
              style={{
                background: !canShow3D ? (theme === 'light' ? '#e9ecef' : '#1a1a1a') : 'linear-gradient(90deg, #dc2626, #ef4444, #f97316)',
                color: !canShow3D ? (theme === 'light' ? 'rgba(0, 0, 0, 0.3)' : 'rgba(255, 255, 255, 0.3)') : '#ffffff',
                cursor: !canShow3D ? 'not-allowed' : 'pointer'
              }}
            >
              3D
            </button>
          </div>
        </div>
      </div>
    );
  }

  // Modes Text et 2D: Structure normale
  return (
    <div className="flex flex-col h-full bg-transparent relative">
      {/* Poignée de redimensionnement custom (coin bas-droit) */}
      <div
        className="absolute bottom-0 right-0 w-4 h-4 cursor-se-resize z-30"
        style={{ background: 'linear-gradient(135deg, transparent 55%, rgba(128, 128, 128, 0.55) 55%)' }}
        onMouseDown={handleResizeHandleMouseDown}
        title="Redimensionner"
      />
      {/* Map Title Bar - Largeur du contenu uniquement */}
      <div
        className="w-full px-4 py-0.5 pl-[10px] relative cursor-move select-none"
        style={{
          background: getWindowHeaderBg(),
          borderBottom: `1px solid ${getCellBorderColor()}`
        }}
        onMouseDown={(e) => {
          if (e.button !== 0) return; // drag uniquement au clic gauche
          // Clear any existing timeout and immediately disable auto-sizing
          if (dragEndTimeoutRef.current) {
            clearTimeout(dragEndTimeoutRef.current);
            dragEndTimeoutRef.current = null;
          }
          setIsDraggingWindow(true);
          onDragStart?.(e);
        }}
      >
        <div className="flex items-center gap-2 pr-8 min-w-0">
          <h3 className="text-[13px] font-medium truncate" title={mapData.codeblock_id != null ? `${mapData.name} [codeblock ${mapData.codeblock_id}]` : mapData.name} style={{ color: getWindowHeaderTextColor() }}>
            {mapData.name}
            {mapData.codeblock_id != null && (
              <span className="font-normal opacity-60"> [codeblock {mapData.codeblock_id}]</span>
            )}
          </h3>
          <span className="text-[11px] pl-[10px] whitespace-nowrap flex-shrink-0" style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.6)' : 'rgba(255, 255, 255, 0.6)' }}>
            {displayMapValues[0]?.length || 0}x{displayMapValues.length}
          </span>
          {onToggleInvertDisplay && (
            <button
              className={`h-6 w-6 p-0 flex items-center justify-center rounded flex-shrink-0 transition-all ${invertButtonActive ? 'bg-gradient-to-r from-red-600/50 via-red-500/50 to-orange-500/50 text-white border border-red-500/40 shadow-sm shadow-red-500/30' : 'hover:bg-white/10'}`}
              style={!invertButtonActive ? { color: theme === 'light' ? 'rgba(0, 0, 0, 0.55)' : 'rgba(255, 255, 255, 0.55)' } : undefined}
              onClick={handleToggleInvert}
              onMouseDown={(e) => e.stopPropagation()}
              title={invertButtonActive ? "Rétablir l'orientation du fichier (lignes ↔ colonnes)" : "Inverser l'affichage (lignes ↔ colonnes)"}
            >
              <Repeat2 className="w-4 h-4" strokeWidth={2.5} />
            </button>
          )}
        </div>
        {mapData.description && (
          <p className="text-[11px] mt-0.5 truncate pr-8" style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)' }}>{cleanDescription(mapData.description)}</p>
        )}
        {onClose && (
          <Button
            variant="ghost"
            size="sm"
            className="h-6 w-6 p-0 flex items-center justify-center hover:bg-red-600/20 hover:text-red-400 absolute right-3 top-1/2 -translate-y-1/2"
            style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.7)' : 'rgba(255, 255, 255, 0.7)' }}
            onClick={handleClose}
            onMouseDown={(e) => e.stopPropagation()}
            title="Fermer cette map"
          >
            <X className="w-4 h-4" />
          </Button>
        )}
      </div>

      {/* Content Area - Plus proche des axes */}
          <div className="flex-1 overflow-hidden p-0 bg-transparent relative" style={{ containerType: 'inline-size' }}>
        {easyViewMode && (canShow3D || isSingleLineMap) ? (
          /* Mode EasyView avec 3D/2D disponible -> split view: Texte en haut + 3D/2D en bas */
          <div className="flex flex-col h-full">
              {/* Vue Texte - hauteur adaptée au contenu du tableau */}
              <div
                className="h-fit overflow-hidden"
                style={{
                  borderBottom: `1px solid ${getCellBorderColor()}`,
                  containerType: 'inline-size'
                }}
              >
                <div className="w-full h-fit overflow-hidden" ref={tableContainerRef} onClick={() => setContextMenu(null)}>
                  <table ref={tableRef} className="border-collapse" style={{ marginTop: '0px', width: 'max-content', tableLayout: 'auto' }}>
                    <thead>
                      <tr>
                        <th className="sticky left-0 z-20 px-1.5 py-1 text-[11px] font-medium text-center relative"
                          style={{
                            background: getAxisCellBg(),
                            border: `1px solid ${getCellBorderColor()}`,
                            color: getCellTextColor()
                          }}>
                          {(displayMapValues.length > 1 || (displayMapValues[0] && displayMapValues[0].length > 1)) && (
                            <div className="flex flex-col justify-between h-full w-full py-0.5">
                              <div
                                className="text-[8px] leading-tight truncate max-w-full px-0.5 text-right"
                                style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)' }}
                                title={parseAxisUnits().xUnit}
                              >
                                {parseAxisUnits().xUnit}
                              </div>
                              <div
                                className="text-[8px] leading-tight truncate max-w-full px-0.5 text-center"
                                style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)' }}
                                title={parseAxisUnits().yUnit}
                              >
                                {parseAxisUnits().yUnit}
                              </div>
                            </div>
                          )}
                        </th>
                        {displayXAxisLabels.map((label, i) => (
                          <th
                            key={i}
                            className="text-center cursor-pointer transition-colors font-mono"
                            style={{
                              background: selectedXAxisCells.has(i) ? getSelectionBg() : getAxisCellBg(),
                              border: `1px solid ${getCellBorderColor()}`,
                              color: getCellTextColor(),
                              padding: 'var(--zs-cell-pad, 2px 4px)' as any,
                              fontSize: 'var(--zs-cell-font, 11px)',
                              fontVariantNumeric: 'tabular-nums',
                              lineHeight: 1,
                              whiteSpace: 'nowrap',
                              overflow: 'hidden',
                              width: 'var(--zs-cell-w, 56px)',
                              maxWidth: 'var(--zs-cell-w, 56px)',
                              height: 'var(--zs-cell-h, 20px)'
                            }}
                            onMouseDown={(e) => handleXAxisMouseDown(i, e)}
                            onMouseEnter={() => handleXAxisMouseEnter(i)}
                            onDoubleClick={() => handleEditDisplayXAxis(i)}
                            onContextMenu={(e) => handleXAxisContextMenu(e, i)}
                          >
                            {formatAxisDisplay(label, effDisplayXCorrection, displayXPrecisionOverride)}
                          </th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      {(() => {
                        const allValues = displayMapValues.flat();
                        const minValue = Math.min(...allValues);
                        const maxValue = Math.max(...allValues);
                        return displayMapValues.map((row, rowIndex) => (
                          <tr key={rowIndex}>
                            <td
                              className="sticky left-0 z-10 font-medium text-center cursor-pointer transition-colors font-mono"
                              style={{
                                background: selectedYAxisCells.has(rowIndex) ? getSelectionBg() : getAxisCellBg(),
                                border: `1px solid ${getCellBorderColor()}`,
                                color: getCellTextColor(),
                                padding: 'var(--zs-cell-pad, 2px 4px)' as any,
                                fontSize: 'var(--zs-cell-font, 11px)',
                                fontVariantNumeric: 'tabular-nums',
                                lineHeight: 1,
                                whiteSpace: 'nowrap',
                                overflow: 'hidden',
                                minWidth: 'var(--zs-yaxis-w, 44px)',
                                maxWidth: 'var(--zs-yaxis-max, 60px)',
                                height: 'var(--zs-cell-h, 20px)',
                                textAlign: 'center'
                              }}
                              onMouseDown={(e) => handleYAxisMouseDown(rowIndex, e)}
                              onMouseEnter={() => handleYAxisMouseEnter(rowIndex)}
                              onDoubleClick={() => handleEditDisplayYAxis(rowIndex)}
                              onContextMenu={(e) => handleYAxisContextMenu(e, rowIndex)}
                            >
                              {formatAxisDisplay(displayYAxisLabels[rowIndex], effDisplayYCorrection, displayYPrecisionOverride)}
                            </td>
                            {row.map((value, colIndex) => {
                              const mapCoords = toMapCoords(rowIndex, colIndex);
                              const cellKey = getCellKey(mapCoords.row, mapCoords.col);
                              const isSelected = selectedCells.has(cellKey);
                              const bgColor = isSelected ? getSelectionBg() : (disableTableColors ? 'transparent' : getValueColor(value, minValue, maxValue, theme));
                              return (
                                <td
                                  key={colIndex}
                                  onMouseDown={(e) => handleCellMouseDown(rowIndex, colIndex, e)}
                                  onMouseEnter={() => handleCellMouseEnter(rowIndex, colIndex)}
                                  onDoubleClick={() => handlePromptEdit(rowIndex, colIndex, value)}
                                  onContextMenu={(e) => handleContextMenu(e, rowIndex, colIndex, value)}
                                  className="font-mono text-center cursor-pointer transition-colors"
                                  style={{
                                    border: `1px solid ${getCellBorderColor()}`,
                                    backgroundColor: bgColor,
                                    color: theme === 'light' ? 'black' : 'white',
                                    padding: 'var(--zs-cell-pad, 2px 4px)' as any,
                                    fontSize: 'var(--zs-cell-font, 11px)',
                                    fontVariantNumeric: 'tabular-nums',
                                    lineHeight: 1,
                                    whiteSpace: 'nowrap',
                                    overflow: 'hidden',
                                    width: 'var(--zs-cell-w, 56px)',
                                    maxWidth: 'var(--zs-cell-w, 56px)',
                                    height: 'var(--zs-cell-h, 20px)'
                                  }}
                                >
                                  {value.toFixed(cellDecimals)}
                                </td>
                              );
                            })}
                          </tr>
                        ));
                      })()}
                    </tbody>
                  </table>
                </div>
                {/* Context Menu for EasyView mode */}
                {contextMenu && (
                  <div
                    ref={contextMenuRef}
                    style={{
                      position: "fixed",
                      left: adjustedContextMenuPos?.x ?? contextMenu.x,
                      top: adjustedContextMenuPos?.y ?? contextMenu.y,
                      background: 'rgba(22, 25, 34, 0.92)',
                      border: '1px solid rgba(255, 255, 255, 0.1)',
                      backdropFilter: 'blur(18px) saturate(140%)',
                      WebkitBackdropFilter: 'blur(18px) saturate(140%)',
                      color: '#ffffff',
                      visibility: adjustedContextMenuPos ? 'visible' : 'hidden'
                    }}
                    className="z-50 rounded-lg shadow-lg p-1.5 flex flex-col gap-0.5 text-[12px] min-w-[160px]"
                    onClick={(e) => e.stopPropagation()}
                  >
                    {/* Copy */}
                    <button
                      className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                      onClick={() => {
                        if (contextMenu.type === 'cell') {
                          const cellsArray = Array.from(selectedCells).map(key => {
                            const [row, col] = key.split('-').map(Number);
                            return { row, col };
                          });
                          if (cellsArray.length === 0) { setContextMenu(null); return; }
                          const minRow = Math.min(...cellsArray.map(c => c.row));
                          const maxRow = Math.max(...cellsArray.map(c => c.row));
                          const minCol = Math.min(...cellsArray.map(c => c.col));
                          const maxCol = Math.max(...cellsArray.map(c => c.col));
                          const values: string[][] = [];
                          for (let r = minRow; r <= maxRow; r++) {
                            const rowValues: string[] = [];
                            for (let c = minCol; c <= maxCol; c++) {
                              const cellKey = `${r}-${c}`;
                              if (selectedCells.has(cellKey)) {
                                const value = mapValues[r]?.[c];
                                rowValues.push(value !== undefined ? value.toString() : '');
                              } else {
                                rowValues.push('');
                              }
                            }
                            values.push(rowValues);
                          }
                          writeClipboard({ values, type: 'cell', rows: maxRow - minRow + 1, cols: maxCol - minCol + 1 });
                          toast({ title: t.mapViewer.copy, description: `${cellsArray.length} value(s) copied` });
                        } else if (contextMenu.type === 'xAxis') {
                          const indices = Array.from(selectedXAxisCells).sort((a, b) => a - b);
                          const values = indices.map(index => [readSourceAxis(displayXAxisToSource(index)) || '']);
                          writeClipboard({ values, type: 'xAxis', rows: indices.length, cols: 1 });
                          toast({ title: t.mapViewer.copy, description: `${indices.length} value(s) copied` });
                        } else if (contextMenu.type === 'yAxis') {
                          const indices = Array.from(selectedYAxisCells).sort((a, b) => a - b);
                          const values = indices.map(index => [readSourceAxis(displayYAxisToSource(index)) || '']);
                          writeClipboard({ values, type: 'yAxis', rows: indices.length, cols: 1 });
                          toast({ title: t.mapViewer.copy, description: `${indices.length} value(s) copied` });
                        }
                        setContextMenu(null);
                      }}
                    >
                      {t.mapViewer.copy}
                    </button>

                    {/* Paste */}
                    <button
                      className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                      onClick={() => {
                        const clip = readClipboard();
                        if (!clip) {
                          toast({ title: t.common.error, description: "Nothing to paste", variant: "destructive" });
                          setContextMenu(null);
                          return;
                        }
                        if (contextMenu.type === 'cell') {
                          const cellsArray = Array.from(selectedCells).map(key => {
                            const [row, col] = key.split('-').map(Number);
                            return { row, col };
                          });
                          if (cellsArray.length === 0) { setContextMenu(null); return; }
                          const startRow = Math.min(...cellsArray.map(c => c.row));
                          const startCol = Math.min(...cellsArray.map(c => c.col));
                          let pastedCount = 0;
                          clip.values.forEach((rowValues, rowOffset) => {
                            rowValues.forEach((value, colOffset) => {
                              if (value === '') return;
                              const targetRow = startRow + rowOffset;
                              const targetCol = startCol + colOffset;
                              if (targetRow < mapValues.length && targetCol < (mapValues[0]?.length || 0)) {
                                const parsed = Number(value.replace(',', '.'));
                                if (!Number.isNaN(parsed)) {
                                  updateCellValue(targetRow, targetCol, parsed);
                                  pastedCount++;
                                }
                              }
                            });
                          });
                          toast({ title: t.mapViewer.paste, description: `${pastedCount} value(s) pasted` });
                        }
                        setContextMenu(null);
                      }}
                    >
                      {t.mapViewer.paste}
                    </button>

                    {/* Smooth Selection - cells only, not axis values */}
                    {contextMenu.type === 'cell' && (
                      <>
                    <div className="border-t border-gray-600 my-1" />

                    <button
                      className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                      onClick={() => {
                        if (contextMenu.type === 'cell' && selectedCells.size > 2) {
                          let minRow = 0xFFFF, maxRow = 0, minCol = 0xFFFF, maxCol = 0;
                          selectedCells.forEach(key => {
                            const [row, col] = key.split('-').map(Number);
                            if (row > maxRow) maxRow = row;
                            if (row < minRow) minRow = row;
                            if (col > maxCol) maxCol = col;
                            if (col < minCol) minCol = col;
                          });
                          if (maxCol === minCol) {
                            const topValue = mapValues[maxRow]?.[maxCol] ?? 0;
                            const bottomValue = mapValues[minRow]?.[maxCol] ?? 0;
                            const cellCount = selectedCells.size;
                            const diffValue = (topValue - bottomValue) / (cellCount - 1);
                            for (let idx = 1; idx < cellCount - 1; idx++) {
                              const newValue = Math.round(bottomValue + (idx * diffValue));
                              updateCellValue(minRow + idx, maxCol, newValue);
                            }
                          } else if (maxRow === minRow) {
                            const rightValue = mapValues[maxRow]?.[maxCol] ?? 0;
                            const leftValue = mapValues[maxRow]?.[minCol] ?? 0;
                            const cellCount = selectedCells.size;
                            const diffValue = (rightValue - leftValue) / (cellCount - 1);
                            for (let idx = 1; idx < cellCount - 1; idx++) {
                              const newValue = Math.round(leftValue + (idx * diffValue));
                              updateCellValue(minRow, minCol + idx, newValue);
                            }
                          } else {
                            const currentValues = mapValues.map(row => [...row]);
                            for (let tely = 1; tely < maxRow - minRow; tely++) {
                              for (let telx = 1; telx < maxCol - minCol; telx++) {
                                const currentRow = minRow + tely;
                                const currentCol = minCol + telx;
                                const valx1 = currentValues[currentRow]?.[currentCol - 1] ?? currentValues[currentRow]?.[minCol] ?? 0;
                                const valx2 = currentValues[currentRow]?.[currentCol + 1] ?? currentValues[currentRow]?.[currentCol] ?? 0;
                                const valy1 = currentValues[currentRow - 1]?.[currentCol] ?? currentValues[minRow]?.[currentCol] ?? 0;
                                const valy2 = currentValues[currentRow + 1]?.[currentCol] ?? currentValues[currentRow]?.[currentCol] ?? 0;
                                const valueX = (valx1 + valx2) / 2;
                                const valueY = (valy1 + valy2) / 2;
                                const newValue = Math.round((valueX + valueY) / 2);
                                updateCellValue(currentRow, currentCol, newValue);
                              }
                            }
                          }
                          toast({ title: t.errors.smoothed, description: t.errors.selectionSmoothed });
                        }
                        setContextMenu(null);
                      }}
                    >
                      {t.mapViewer.smoothSelection}
                    </button>
                      </>
                    )}

                    <div className="border-t border-gray-600 my-1" />

                    {/* Increase */}
                    <button
                      className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                      onClick={() => {
                        if (contextMenu.type === 'cell') {
                          selectedCells.forEach(cellKey => {
                            const [row, col] = cellKey.split('-').map(Number);
                            const currentValue = mapValues[row]?.[col] ?? 0;
                            updateCellValue(row, col, currentValue + incrementValue);
                          });
                        } else if (contextMenu.type === 'xAxis') {
                          selectedXAxisCells.forEach(index => mutateDisplayXAxis(index, cur => {
                            const v = parseFloat(cur);
                            return isNaN(v) ? cur : (v + incrementValue).toString();
                          }));
                        } else if (contextMenu.type === 'yAxis') {
                          selectedYAxisCells.forEach(index => mutateDisplayYAxis(index, cur => {
                            const v = parseFloat(cur);
                            return isNaN(v) ? cur : (v + incrementValue).toString();
                          }));
                        }
                        setContextMenu(null);
                      }}
                    >
                      {t.mapViewer.increase} (+{incrementValue})
                    </button>

                    {/* Decrease */}
                    <button
                      className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                      onClick={() => {
                        if (contextMenu.type === 'cell') {
                          selectedCells.forEach(cellKey => {
                            const [row, col] = cellKey.split('-').map(Number);
                            const currentValue = mapValues[row]?.[col] ?? 0;
                            updateCellValue(row, col, currentValue - incrementValue);
                          });
                        } else if (contextMenu.type === 'xAxis') {
                          selectedXAxisCells.forEach(index => mutateDisplayXAxis(index, cur => {
                            const v = parseFloat(cur);
                            return isNaN(v) ? cur : (v - incrementValue).toString();
                          }));
                        } else if (contextMenu.type === 'yAxis') {
                          selectedYAxisCells.forEach(index => mutateDisplayYAxis(index, cur => {
                            const v = parseFloat(cur);
                            return isNaN(v) ? cur : (v - incrementValue).toString();
                          }));
                        }
                        setContextMenu(null);
                      }}
                    >
                      {t.mapViewer.decrease} (-{incrementValue})
                    </button>

                    <div className="border-t border-gray-600 my-1" />

                    {/* Change absolute */}
                    <button
                      className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                      onClick={() => {
                        const menuType = contextMenu.type;
                        const cellsToUpdate = [...selectedCells];
                        const xCellsToUpdate = [...selectedXAxisCells];
                        const yCellsToUpdate = [...selectedYAxisCells];
                        const workspaceCenter = { x: window.innerWidth / 2 + 140, y: window.innerHeight * 0.35 };
                        setAbsoluteValueModal({
                          isOpen: true,
                          inputValue: "",
                          position: workspaceCenter,
                          isDragging: false,
                          dragOffset: { x: 0, y: 0 },
                          onConfirm: (parsed: number) => {
                            if (menuType === 'cell') {
                              cellsToUpdate.forEach(cellKey => {
                                const [row, col] = cellKey.split('-').map(Number);
                                updateCellValue(row, col, parsed);
                              });
                            } else if (menuType === 'xAxis') {
                              xCellsToUpdate.forEach(index => mutateDisplayXAxis(index, () => parsed.toString()));
                            } else if (menuType === 'yAxis') {
                              yCellsToUpdate.forEach(index => mutateDisplayYAxis(index, () => parsed.toString()));
                            }
                          }
                        });
                        setContextMenu(null);
                      }}
                    >
                      {t.mapViewer.changeAbsolute}
                    </button>

                    {/* Original value */}
                    <button
                      className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                      onClick={() => {
                        if (contextMenu.type === 'cell') {
                          selectedCells.forEach(cellKey => {
                            const [row, col] = cellKey.split('-').map(Number);
                            const original = originalValuesRef.current?.[row]?.[col];
                            if (original !== undefined) updateCellValue(row, col, original);
                          });
                          toast({ title: t.errors.restored, description: t.errors.originalValuesRestored });
                        }
                        setContextMenu(null);
                      }}
                    >
                      {t.mapViewer.originalValue}
                    </button>

                    <div className="border-t border-gray-600 my-1" />

                    {/* Apply to similar maps */}
                    <button
                      className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                      onClick={() => {
                        if (allMaps && allMaps.length > 0) {
                          setSimilarMapsModal({
                            isOpen: true,
                            copyType: Object.keys(changedCells).length > 0 ? 'modifications' : 'all',
                            selectedMaps: [],
                          });
                        } else {
                          toast({ title: t.common.error, description: "Map list not loaded" });
                        }
                        setContextMenu(null);
                      }}
                    >
                      {t.mapViewer.changeSimilarMaps}
                    </button>

                    {/* Properties */}
                    <button
                      className={`px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors ${onOpenProperties ? 'text-white' : 'text-gray-400'}`}
                      onClick={() => {
                        if (onOpenProperties) {
                          onOpenProperties();
                        } else {
                          toast({ title: t.errors.comingSoon, description: t.errors.comingSoonDescription });
                        }
                        setContextMenu(null);
                      }}
                    >
                      {t.mapViewer.properties}
                    </button>
                  </div>
                )}
              </div>

              {/* Vue 3D/2D - prend l'espace restant */}
              <div className="flex-1 overflow-hidden">
                {canShow3D ? (
                  /* Afficher vue 3D */
                  <div className="w-full h-full relative">
                    <Plot
                      key={`3d-easyview-${mapData.address}`}
                      data={plot3DData}
                      layout={{
                        paper_bgcolor: "transparent",
                        plot_bgcolor: "transparent",
                        scene: {
                          xaxis: {
                            title: parseAxisUnits().xLabel + " (" + parseAxisUnits().xUnit + ")",
                            backgroundcolor: "transparent",
                            gridcolor: "#374151",
                            showbackground: true,
                            color: "#9ca3af",
                            tickmode: "array" as const,
                            tickvals: plot3DTicks.xTickVals,
                            ticktext: plot3DTicks.xTickText,
                          },
                          yaxis: {
                            title: parseAxisUnits().yLabel + " (" + parseAxisUnits().yUnit + ")",
                            backgroundcolor: "transparent",
                            gridcolor: "#374151",
                            showbackground: true,
                            color: "#9ca3af",
                            tickmode: "array" as const,
                            tickvals: plot3DTicks.yTickVals,
                            ticktext: plot3DTicks.yTickText,
                          },
                          zaxis: {
                            title: "Value",
                            backgroundcolor: "transparent",
                            gridcolor: "#374151",
                            showbackground: true,
                            color: "#9ca3af",
                          },
                          // Position de la caméra restaurée depuis savedCameraPositions (persiste entre les montages)
                          camera: cameraPosition,
                          aspectmode: "manual",
                          aspectratio: { x: 1, y: 1, z: 0.7 },
                        },
                        margin: { t: 20, r: 0, b: 0, l: 0 },
                        autosize: true,
                        // La révision de zoom force l'application de la caméra
                        // des boutons +/− (uirevision fige sinon l'état UI)
                        uirevision: `${mapData.address}-z${cameraZoomRev}`,
                      }}
                      config={{
                        displayModeBar: false,
                        displaylogo: false,
                        staticPlot: isDraggingWindow, // Freeze plot interactions during window drag for better performance
                      }}
                      style={{ width: "100%", height: "100%" }}
                      useResizeHandler={true}
                      onRelayout={handlePlotlyRelayout}
                    />
                    {/* Zoom +/− du graphique 3D — même style que les onglets de
                        vue, coin BAS-GAUCHE du panneau 3D (à droite, ils
                        recouvraient la poignée de redimensionnement) */}
                    <div
                      className="absolute bottom-0 left-0 z-30 flex"
                      style={{
                        borderTop: `1px solid ${getCellBorderColor()}`,
                        borderRight: `1px solid ${getCellBorderColor()}`,
                        background: theme === 'light' ? '#f1f3f5' : '#1a1a1a'
                      }}
                    >
                      <button
                        onClick={() => zoomCamera(1.25)}
                        className="px-3 py-1 text-[13px] leading-[14px] font-medium transition-colors"
                        style={{
                          borderRight: `1px solid ${getCellBorderColor()}`,
                          background: getViewButtonBg(),
                          color: getCellTextColor()
                        }}
                        onMouseEnter={(e) => e.currentTarget.style.background = getViewButtonBgHover()}
                        onMouseLeave={(e) => e.currentTarget.style.background = getViewButtonBg()}
                        title="Zoom -"
                      >
                        −
                      </button>
                      <button
                        onClick={() => zoomCamera(0.8)}
                        className="px-3 py-1 text-[13px] leading-[14px] font-medium transition-colors"
                        style={{
                          background: getViewButtonBg(),
                          color: getCellTextColor()
                        }}
                        onMouseEnter={(e) => e.currentTarget.style.background = getViewButtonBgHover()}
                        onMouseLeave={(e) => e.currentTarget.style.background = getViewButtonBg()}
                        title="Zoom +"
                      >
                        +
                      </button>
                    </div>
                  </div>
                ) : (
                  /* Afficher vue 2D si 3D pas disponible */
                  <div className="w-full h-full">
                    <Plot
                      key={`2d-easyview-${mapData.address}`}
                      data={[{
                        x: displayXAxisLabels,
                        y: displayMapValues[0],
                        type: "scatter",
                        mode: "lines+markers",
                        name: mapData.name,
                        line: {
                          width: 3,
                          color: "#dc2626",
                        },
                        marker: {
                          size: 6,
                          color: "#dc2626",
                        },
                        hovertemplate: "X: %{x}<br>Value: %{y:.1f}<extra></extra>",
                      }]}
                      layout={{
                        paper_bgcolor: "transparent",
                        plot_bgcolor: "transparent",
                        xaxis: {
                          title: mapData.map_type?.includes("Injection") ? "RPM" : "Load",
                          color: "#9ca3af",
                          gridcolor: "#374151",
                        },
                        yaxis: {
                          title: "Value",
                          color: "#9ca3af",
                          gridcolor: "#374151",
                        },
                        margin: { t: 20, r: 30, b: 40, l: 60 },
                        autosize: true,
                        showlegend: false,
                      }}
                      config={{
                        displayModeBar: false,
                        displaylogo: false,
                      }}
                      style={{ width: "100%", height: "100%" }}
                      useResizeHandler={!isDraggingWindow}
                    />
                  </div>
                )}
              </div>
            </div>
        ) : (viewMode === "text" || (easyViewMode && !canShow3D && !isSingleLineMap)) ? (
          <>
          <div className="w-full h-full overflow-hidden" ref={tableContainerRef} onClick={() => setContextMenu(null)}>
            <table ref={tableRef} className="border-collapse" style={{ marginTop: '0px', width: 'max-content', tableLayout: 'auto' }}>
              <thead>
                <tr>
                  <th className="sticky left-0 z-20 px-1.5 py-1 text-[11px] font-medium text-center relative"
                          style={{
                            background: getAxisCellBg(),
                            border: `1px solid ${getCellBorderColor()}`,
                            color: getCellTextColor()
                          }}>
                    {/* Only show axis units if map has dimensions > 1x1 */}
                    {(displayMapValues.length > 1 || (displayMapValues[0] && displayMapValues[0].length > 1)) && (
                      <div className="flex flex-col justify-between h-full w-full py-0.5">
                        <div
                          className="text-[8px] leading-tight truncate max-w-full px-0.5 text-right"
                          style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)' }}
                          title={parseAxisUnits().xUnit}
                        >
                          {parseAxisUnits().xUnit}
                        </div>
                        <div
                          className="text-[8px] leading-tight truncate max-w-full px-0.5 text-center"
                          style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)' }}
                          title={parseAxisUnits().yUnit}
                        >
                          {parseAxisUnits().yUnit}
                        </div>
                      </div>
                    )}
                  </th>
                  {displayXAxisLabels.map((label, i) => (
                    <th
                      key={i}
                      className="text-center cursor-pointer transition-colors font-mono"
                      style={{
                        background: selectedXAxisCells.has(i) ? getSelectionBg() : getAxisCellBg(),
                        border: `1px solid ${getCellBorderColor()}`,
                        color: getCellTextColor(),
                        padding: 'var(--zs-cell-pad, 2px 4px)' as any,
                        fontSize: 'var(--zs-cell-font, 11px)',
                        fontVariantNumeric: 'tabular-nums',
                        lineHeight: 1, // la police ne doit jamais forcer la hauteur de ligne
                        whiteSpace: 'nowrap',
                        overflow: 'hidden',
                        width: 'var(--zs-cell-w, 56px)',
                        maxWidth: 'var(--zs-cell-w, 56px)',
                        height: 'var(--zs-cell-h, 20px)'
                      }}
                      onMouseDown={(e) => handleXAxisMouseDown(i, e)}
                      onMouseEnter={() => handleXAxisMouseEnter(i)}
                      onDoubleClick={() => handleEditDisplayXAxis(i)}
                      onContextMenu={(e) => handleXAxisContextMenu(e, i)}
                    >
                      {formatAxisDisplay(label, effDisplayXCorrection, displayXPrecisionOverride)}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {(() => {
                  // Calculer min et max pour le dégradé de couleur
                  const allValues = displayMapValues.flat();
                  const minValue = Math.min(...allValues);
                  const maxValue = Math.max(...allValues);

                  return displayMapValues.map((row, rowIndex) => (
                    <tr key={rowIndex}>
                      <td
                        className="sticky left-0 z-10 font-medium text-center cursor-pointer transition-colors font-mono"
                        style={{
                          background: selectedYAxisCells.has(rowIndex) ? getSelectionBg() : getAxisCellBg(),
                          border: `1px solid ${getCellBorderColor()}`,
                          color: getCellTextColor(),
                          padding: 'var(--zs-cell-pad, 2px 4px)' as any,
                          fontSize: 'var(--zs-cell-font, 11px)',
                          fontVariantNumeric: 'tabular-nums',
                          lineHeight: 1, // la police ne doit jamais forcer la hauteur de ligne
                          whiteSpace: 'nowrap',
                          overflow: 'hidden',
                          minWidth: 'var(--zs-yaxis-w, 44px)',
                          maxWidth: 'var(--zs-yaxis-max, 60px)',
                          height: 'var(--zs-cell-h, 20px)',
                          textAlign: 'center'
                        }}
                        onMouseDown={(e) => handleYAxisMouseDown(rowIndex, e)}
                        onMouseEnter={() => handleYAxisMouseEnter(rowIndex)}
                        onDoubleClick={() => handleEditDisplayYAxis(rowIndex)}
                        onContextMenu={(e) => handleYAxisContextMenu(e, rowIndex)}
                      >
                        {formatAxisDisplay(displayYAxisLabels[rowIndex], effDisplayYCorrection, displayYPrecisionOverride)}
                      </td>
                      {row.map((value, colIndex) => {
                        const mapCoords = toMapCoords(rowIndex, colIndex);
                        const cellKey = getCellKey(mapCoords.row, mapCoords.col);
                        const isSelected = selectedCells.has(cellKey);
                        const bgColor = isSelected ? getSelectionBg() : (disableTableColors ? 'transparent' : getValueColor(value, minValue, maxValue, theme));

                        return (
                          <td
                            key={colIndex}
                            onMouseDown={(e) => handleCellMouseDown(rowIndex, colIndex, e)}
                            onMouseEnter={() => handleCellMouseEnter(rowIndex, colIndex)}
                            onDoubleClick={() => handlePromptEdit(rowIndex, colIndex, value)}
                            onContextMenu={(e) => handleContextMenu(e, rowIndex, colIndex, value)}
                            className="font-mono text-center cursor-pointer transition-colors"
                            style={{
                              border: `1px solid ${getCellBorderColor()}`,
                              backgroundColor: bgColor,
                              color: theme === 'light' ? 'black' : 'white',
                              padding: 'var(--zs-cell-pad, 2px 4px)' as any,
                              fontSize: 'var(--zs-cell-font, 11px)',
                              fontVariantNumeric: 'tabular-nums',
                              lineHeight: 1, // la police ne doit jamais forcer la hauteur de ligne
                              whiteSpace: 'nowrap',
                              overflow: 'hidden',
                              width: 'var(--zs-cell-w, 56px)',
                              maxWidth: 'var(--zs-cell-w, 56px)',
                              height: 'var(--zs-cell-h, 20px)'
                            }}
                          >
                            {value.toFixed(cellDecimals)}
                          </td>
                        );
                      })}
                    </tr>
                  ));
                })()}
              </tbody>
            </table>
          </div>
          {contextMenu && (
            <div
              ref={contextMenuRef}
              style={{
                position: "fixed",
                left: adjustedContextMenuPos?.x ?? contextMenu.x,
                top: adjustedContextMenuPos?.y ?? contextMenu.y,
                background: 'rgba(22, 25, 34, 0.92)',
                border: '1px solid rgba(255, 255, 255, 0.1)',
                backdropFilter: 'blur(18px) saturate(140%)',
                WebkitBackdropFilter: 'blur(18px) saturate(140%)',
                color: '#ffffff',
                visibility: adjustedContextMenuPos ? 'visible' : 'hidden'
              }}
              className="z-50 rounded-lg shadow-lg p-1.5 flex flex-col gap-0.5 text-[12px] min-w-[160px]"
              onClick={(e) => e.stopPropagation()}
            >
              {/* Copy */}
              <button
                className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                onClick={() => {
                  if (contextMenu.type === 'cell') {
                    // Trouver les bornes de la sélection pour préserver la structure 2D
                    const cellsArray = Array.from(selectedCells).map(key => {
                      const [row, col] = key.split('-').map(Number);
                      return { row, col };
                    });

                    if (cellsArray.length === 0) {
                      setContextMenu(null);
                      return;
                    }

                    const minRow = Math.min(...cellsArray.map(c => c.row));
                    const maxRow = Math.max(...cellsArray.map(c => c.row));
                    const minCol = Math.min(...cellsArray.map(c => c.col));
                    const maxCol = Math.max(...cellsArray.map(c => c.col));

                    // Créer un tableau 2D avec les valeurs
                    const values: string[][] = [];
                    for (let r = minRow; r <= maxRow; r++) {
                      const rowValues: string[] = [];
                      for (let c = minCol; c <= maxCol; c++) {
                        const cellKey = `${r}-${c}`;
                        if (selectedCells.has(cellKey)) {
                          const value = mapValues[r]?.[c];
                          rowValues.push(value !== undefined ? value.toString() : '');
                        } else {
                          rowValues.push(''); // Cellule non sélectionnée dans la zone
                        }
                      }
                      values.push(rowValues);
                    }

                    writeClipboard({
                      values,
                      type: 'cell',
                      rows: maxRow - minRow + 1,
                      cols: maxCol - minCol + 1
                    });

                    const totalValues = cellsArray.length;
                    toast({ title: t.mapViewer.copy, description: `${totalValues} value(s) copied` });
                  } else if (contextMenu.type === 'xAxis') {
                    const indices = Array.from(selectedXAxisCells).sort((a, b) => a - b);
                    const values = indices.map(index => [readSourceAxis(displayXAxisToSource(index)) || '']);
                    writeClipboard({
                      values,
                      type: 'xAxis',
                      rows: indices.length,
                      cols: 1
                    });
                    toast({ title: t.mapViewer.copy, description: `${indices.length} value(s) copied` });
                  } else if (contextMenu.type === 'yAxis') {
                    const indices = Array.from(selectedYAxisCells).sort((a, b) => a - b);
                    const values = indices.map(index => [readSourceAxis(displayYAxisToSource(index)) || '']);
                    writeClipboard({
                      values,
                      type: 'yAxis',
                      rows: indices.length,
                      cols: 1
                    });
                    toast({ title: t.mapViewer.copy, description: `${indices.length} value(s) copied` });
                  }
                  setContextMenu(null);
                }}
              >
                {t.mapViewer.copy}
              </button>

              {/* Paste */}
              <button
                className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                onClick={() => {
                  const clip = readClipboard();
                  if (!clip) {
                    toast({ title: t.common.error, description: "Nothing to paste", variant: "destructive" });
                    setContextMenu(null);
                    return;
                  }

                  if (contextMenu.type === 'cell') {
                    // Trouver la cellule de départ (la plus en haut à gauche de la sélection)
                    const cellsArray = Array.from(selectedCells).map(key => {
                      const [row, col] = key.split('-').map(Number);
                      return { row, col };
                    });

                    if (cellsArray.length === 0) {
                      setContextMenu(null);
                      return;
                    }

                    const startRow = Math.min(...cellsArray.map(c => c.row));
                    const startCol = Math.min(...cellsArray.map(c => c.col));

                    // Coller les valeurs en cascade à partir de la cellule de départ
                    let pastedCount = 0;
                    clip.values.forEach((rowValues, rowOffset) => {
                      rowValues.forEach((value, colOffset) => {
                        if (value === '') return; // Ignorer les cellules vides

                        const targetRow = startRow + rowOffset;
                        const targetCol = startCol + colOffset;

                        // Vérifier que la cellule cible existe
                        if (targetRow < mapValues.length && targetCol < (mapValues[0]?.length || 0)) {
                          const parsed = Number(value.replace(',', '.'));
                          if (!Number.isNaN(parsed)) {
                            updateCellValue(targetRow, targetCol, parsed);
                            pastedCount++;
                          }
                        }
                      });
                    });

                    toast({ title: t.mapViewer.paste, description: `${pastedCount} value(s) pasted` });
                  } else if (contextMenu.type === 'xAxis') {
                    const indices = Array.from(selectedXAxisCells).sort((a, b) => a - b);
                    const startIndex = indices[0] ?? 0;
                    const displayLen = displayXAxisLabels.length;
                    let pastedCount = 0;
                    clip.values.forEach((rowValues, offset) => {
                      const value = rowValues[0];
                      const displayIdx = startIndex + offset;
                      if (value && displayIdx < displayLen) {
                        mutateDisplayXAxis(displayIdx, () => value);
                        pastedCount++;
                      }
                    });
                    toast({ title: t.mapViewer.paste, description: `${pastedCount} value(s) pasted` });
                  } else if (contextMenu.type === 'yAxis') {
                    const indices = Array.from(selectedYAxisCells).sort((a, b) => a - b);
                    const startIndex = indices[0] ?? 0;
                    const displayLen = displayYAxisLabels.length;
                    let pastedCount = 0;
                    clip.values.forEach((rowValues, offset) => {
                      const value = rowValues[0];
                      const displayIdx = startIndex + offset;
                      if (value && displayIdx < displayLen) {
                        mutateDisplayYAxis(displayIdx, () => value);
                        pastedCount++;
                      }
                    });
                    toast({ title: t.mapViewer.paste, description: `${pastedCount} value(s) pasted` });
                  }
                  setContextMenu(null);
                }}
              >
                {t.mapViewer.paste}
              </button>

              {/* Smooth Selection - cells only, not axis values */}
              {contextMenu.type === 'cell' && (
                <>
              <div className="border-t border-gray-600 my-1" />

              <button
                className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                onClick={() => {
                  if (contextMenu.type === 'cell' && selectedCells.size > 2) {
                    // Récupérer les limites de la sélection
                    let minRow = 0xFFFF, maxRow = 0, minCol = 0xFFFF, maxCol = 0;

                    selectedCells.forEach(key => {
                      const [row, col] = key.split('-').map(Number);
                      if (row > maxRow) maxRow = row;
                      if (row < minRow) minRow = row;
                      if (col > maxCol) maxCol = col;
                      if (col < minCol) minCol = col;
                    });

                    if (maxCol === minCol) {
                      // Une seule colonne sélectionnée - interpolation linéaire verticale
                      const topValue = mapValues[maxRow]?.[maxCol] ?? 0;
                      const bottomValue = mapValues[minRow]?.[maxCol] ?? 0;
                      const cellCount = selectedCells.size;
                      const diffValue = (topValue - bottomValue) / (cellCount - 1);

                      for (let t = 1; t < cellCount - 1; t++) {
                        const newValue = Math.round(bottomValue + (t * diffValue));
                        updateCellValue(minRow + t, maxCol, newValue);
                      }
                    } else if (maxRow === minRow) {
                      // Une seule ligne sélectionnée - interpolation linéaire horizontale
                      const rightValue = mapValues[maxRow]?.[maxCol] ?? 0;
                      const leftValue = mapValues[maxRow]?.[minCol] ?? 0;
                      const cellCount = selectedCells.size;
                      const diffValue = (rightValue - leftValue) / (cellCount - 1);

                      for (let t = 1; t < cellCount - 1; t++) {
                        const newValue = Math.round(leftValue + (t * diffValue));
                        updateCellValue(minRow, minCol + t, newValue);
                      }
                    } else {
                      // Bloc sélectionné - moyenne des 4 voisins (comme EDCSuite)
                      // On copie les valeurs actuelles pour ne pas interférer pendant le calcul
                      const currentValues = mapValues.map(row => [...row]);

                      for (let tely = 1; tely < maxRow - minRow; tely++) {
                        for (let telx = 1; telx < maxCol - minCol; telx++) {
                          const currentRow = minRow + tely;
                          const currentCol = minCol + telx;

                          // Valeurs des voisins
                          const valx1 = currentValues[currentRow]?.[currentCol - 1] ?? currentValues[currentRow]?.[minCol] ?? 0;
                          const valx2 = currentValues[currentRow]?.[currentCol + 1] ?? currentValues[currentRow]?.[currentCol] ?? 0;
                          const valy1 = currentValues[currentRow - 1]?.[currentCol] ?? currentValues[minRow]?.[currentCol] ?? 0;
                          const valy2 = currentValues[currentRow + 1]?.[currentCol] ?? currentValues[currentRow]?.[currentCol] ?? 0;

                          // Moyenne des voisins horizontaux et verticaux
                          const valueX = (valx1 + valx2) / 2;
                          const valueY = (valy1 + valy2) / 2;
                          const newValue = Math.round((valueX + valueY) / 2);

                          updateCellValue(currentRow, currentCol, newValue);
                        }
                      }
                    }

                    toast({ title: t.errors.smoothed, description: t.errors.selectionSmoothed });
                  }
                  setContextMenu(null);
                }}
              >
                {t.mapViewer.smoothSelection}
              </button>
                </>
              )}

              <div className="border-t border-gray-600 my-1" />

              {/* Value +1 */}
              <button
                className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                onClick={() => {
                  if (contextMenu.type === 'cell') {
                    selectedCells.forEach(cellKey => {
                      const [row, col] = cellKey.split('-').map(Number);
                      const currentValue = mapValues[row]?.[col];
                      if (currentValue !== undefined) {
                        updateCellValue(row, col, currentValue + incrementValue);
                      }
                    });
                  } else if (contextMenu.type === 'xAxis') {
                    selectedXAxisCells.forEach(index => mutateDisplayXAxis(index, cur => {
                      const parsed = Number(cur?.replace(',', '.'));
                      return Number.isNaN(parsed) ? cur : (parsed + 1).toString();
                    }));
                  } else if (contextMenu.type === 'yAxis') {
                    selectedYAxisCells.forEach(index => mutateDisplayYAxis(index, cur => {
                      const parsed = Number(cur?.replace(',', '.'));
                      return Number.isNaN(parsed) ? cur : (parsed + 1).toString();
                    }));
                  }
                  setContextMenu(null);
                }}
              >
                {t.mapViewer.increase} (+{incrementValue})
              </button>

              {/* Value -1 */}
              <button
                className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                onClick={() => {
                  if (contextMenu.type === 'cell') {
                    selectedCells.forEach(cellKey => {
                      const [row, col] = cellKey.split('-').map(Number);
                      const currentValue = mapValues[row]?.[col];
                      if (currentValue !== undefined) {
                        updateCellValue(row, col, currentValue - incrementValue);
                      }
                    });
                  } else if (contextMenu.type === 'xAxis') {
                    selectedXAxisCells.forEach(index => mutateDisplayXAxis(index, cur => {
                      const parsed = Number(cur?.replace(',', '.'));
                      return Number.isNaN(parsed) ? cur : (parsed - 1).toString();
                    }));
                  } else if (contextMenu.type === 'yAxis') {
                    selectedYAxisCells.forEach(index => mutateDisplayYAxis(index, cur => {
                      const parsed = Number(cur?.replace(',', '.'));
                      return Number.isNaN(parsed) ? cur : (parsed - 1).toString();
                    }));
                  }
                  setContextMenu(null);
                }}
              >
                {t.mapViewer.decrease} (-{incrementValue})
              </button>

              <div className="border-t border-gray-600 my-1" />

              {/* Change absolute */}
              <button
                className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                onClick={() => {
                  const menuType = contextMenu.type;
                  const cellsToUpdate = [...selectedCells];
                  const xCellsToUpdate = [...selectedXAxisCells];
                  const yCellsToUpdate = [...selectedYAxisCells];

                  // Calculate initial position centered in workspace
                  const workspaceCenter = {
                    x: window.innerWidth / 2 + 140, // Account for sidebar
                    y: window.innerHeight * 0.35
                  };
                  setAbsoluteValueModal({
                    isOpen: true,
                    inputValue: "",
                    position: workspaceCenter,
                    isDragging: false,
                    dragOffset: { x: 0, y: 0 },
                    onConfirm: (parsed: number) => {
                      if (menuType === 'cell') {
                        cellsToUpdate.forEach(cellKey => {
                          const [row, col] = cellKey.split('-').map(Number);
                          updateCellValue(row, col, parsed);
                        });
                      } else if (menuType === 'xAxis') {
                        xCellsToUpdate.forEach(index => mutateDisplayXAxis(index, () => parsed.toString()));
                      } else if (menuType === 'yAxis') {
                        yCellsToUpdate.forEach(index => mutateDisplayYAxis(index, () => parsed.toString()));
                      }
                    }
                  });
                  setContextMenu(null);
                }}
              >
                {t.mapViewer.changeAbsolute}
              </button>

              {/* Original value */}
              <button
                className="px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors"
                onClick={() => {
                  if (contextMenu.type === 'cell') {
                    selectedCells.forEach(cellKey => {
                      const [row, col] = cellKey.split('-').map(Number);
                      handleRestoreOriginal(row, col);
                    });
                  } else if (contextMenu.type === 'xAxis') {
                    // Le ref "original" est en ordre SOURCE : on traduit l'index
                    // affiché vers (axe source, index source), puis on lit le ref
                    // du bon axe à cet index.
                    selectedXAxisCells.forEach(index => {
                      const tgt = displayXAxisToSource(index);
                      const ref = tgt.axis === 'x' ? originalXAxisLabelsRef.current : originalYAxisLabelsRef.current;
                      const orig = ref[tgt.index];
                      if (orig) mutateSourceAxis(tgt, () => orig);
                    });
                  } else if (contextMenu.type === 'yAxis') {
                    selectedYAxisCells.forEach(index => {
                      const tgt = displayYAxisToSource(index);
                      const ref = tgt.axis === 'x' ? originalXAxisLabelsRef.current : originalYAxisLabelsRef.current;
                      const orig = ref[tgt.index];
                      if (orig) mutateSourceAxis(tgt, () => orig);
                    });
                  }
                  setContextMenu(null);
                }}
              >
                {t.mapViewer.originalValue}
              </button>

              <div className="border-t border-gray-600 my-1" />

              {/* Change similar maps */}
              <button
                className={`px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors ${allMaps ? 'text-white' : 'text-gray-400'}`}
                onClick={() => {
                  if (allMaps) {
                    setSimilarMapsModal({
                      isOpen: true,
                      copyType: Object.keys(changedCells).length > 0 ? 'modifications' : 'all',
                      selectedMaps: [],
                    });
                  } else {
                    toast({ title: t.common.error, description: "Map list not loaded" });
                  }
                  setContextMenu(null);
                }}
              >
                {t.mapViewer.changeSimilarMaps}
              </button>

              {/* Properties */}
              <button
                className={`px-3 py-1.5 text-left rounded hover:bg-white/10 transition-colors ${onOpenProperties ? 'text-white' : 'text-gray-400'}`}
                onClick={() => {
                  if (onOpenProperties) {
                    onOpenProperties();
                  } else {
                    toast({ title: t.errors.comingSoon, description: t.errors.comingSoonDescription });
                  }
                  setContextMenu(null);
                }}
              >
                {t.mapViewer.properties}
              </button>
            </div>
          )}
          </>
        ) : (
          displayMapValues.length === 1 && displayMapValues[0].length > 1 ? (
            <div className="w-full h-full">
              <Plot
                key={`2d-${mapData.address}`}
                data={[{
                  x: displayXAxisLabels,
                  y: displayMapValues[0],
                  type: "scatter",
                  mode: "lines+markers",
                  name: mapData.name,
                  line: {
                    width: 3,
                    color: "#dc2626",
                  },
                  marker: {
                    size: 6,
                    color: "#dc2626",
                  },
                  hovertemplate: "X: %{x}<br>Value: %{y:.1f}<extra></extra>",
                }]}
                layout={{
                  title: {
                    text: mapData.name,
                    font: { color: "#ffffff", size: 14 },
                  },
                  paper_bgcolor: "transparent",
                  plot_bgcolor: "transparent",
                  xaxis: {
                    title: mapData.map_type?.includes("Injection") ? "RPM" : "Load",
                    color: "#9ca3af",
                    gridcolor: "#374151",
                  },
                  yaxis: {
                    title: "Value",
                    color: "#9ca3af",
                    gridcolor: "#374151",
                  },
                  margin: { t: 50, r: 50, b: 50, l: 80 },
                  autosize: true,
                  showlegend: false,
                }}
                config={{
                  displayModeBar: false,
                  displaylogo: false,
                }}
                style={{ width: "100%", height: "100%" }}
                useResizeHandler={true}
              />
            </div>
          ) : (
            <div className="flex items-center justify-center h-full">
              <div className="text-center p-8 bg-yellow-500/10 border border-yellow-500/20 rounded-lg">
                <p className="text-yellow-500 text-lg mb-2">ÔÜá´©Å Vue 2D non disponible</p>
                <p className="text-sm" style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.7)' : 'rgba(255, 255, 255, 0.7)' }}>
                  Cette map ne peut pas ├¬tre affich├®e en vue 2D.
                  <br />
                  La vue 2D n├®cessite une map avec une seule ligne de donn├®es.
                </p>
              </div>
            </div>
          )
        )}

        {/* View Mode Tabs - Masqués en mode EasyView sauf si 3D/2D non disponibles */}
        {(!easyViewMode || (easyViewMode && !canShow3D && !isSingleLineMap)) && (
          <div
            className="absolute bottom-0 left-0 z-20 flex"
            style={{
              borderTop: `1px solid ${getCellBorderColor()}`,
              background: theme === 'light' ? '#f1f3f5' : '#1a1a1a'
            }}
          >
            <button
              onClick={() => handleViewModeChange("text")}
              className="px-3.5 py-1 text-[11px] leading-[14px] font-medium transition-colors"
              style={{
                borderRight: `1px solid ${getCellBorderColor()}`,
                background: viewMode === "text" ? 'linear-gradient(90deg, #dc2626, #ef4444, #f97316)' : getViewButtonBg(),
                color: viewMode === "text" ? '#ffffff' : getCellTextColor()
              }}
              onMouseEnter={(e) => {
                if (viewMode !== "text") e.currentTarget.style.background = getViewButtonBgHover();
              }}
              onMouseLeave={(e) => {
                if (viewMode !== "text") e.currentTarget.style.background = getViewButtonBg();
              }}
            >
              Text
            </button>
            <button
              onClick={() => handleViewModeChange("2d")}
              disabled={!isSingleLineMap}
              className="px-3.5 py-1 text-[11px] leading-[14px] font-medium transition-colors"
              style={{
                borderRight: `1px solid ${getCellBorderColor()}`,
                background: !isSingleLineMap
                  ? (theme === 'light' ? '#f1f3f5' : '#1a1a1a')
                  : viewMode === "2d"
                    ? 'linear-gradient(90deg, #dc2626, #ef4444, #f97316)'
                    : getViewButtonBg(),
                color: !isSingleLineMap
                  ? (theme === 'light' ? 'rgba(0, 0, 0, 0.3)' : 'rgba(255, 255, 255, 0.3)')
                  : viewMode === "2d"
                    ? '#ffffff'
                    : getCellTextColor(),
                cursor: !isSingleLineMap ? 'not-allowed' : 'pointer'
              }}
              onMouseEnter={(e) => {
                if (isSingleLineMap && viewMode !== "2d") e.currentTarget.style.background = getViewButtonBgHover();
              }}
              onMouseLeave={(e) => {
                if (isSingleLineMap && viewMode !== "2d") e.currentTarget.style.background = getViewButtonBg();
              }}
            >
              2D
            </button>
            <button
              onClick={() => handleViewModeChange("3d")}
              disabled={!canShow3D}
              className="px-3.5 py-1 text-[11px] leading-[14px] font-medium transition-colors"
              style={{
                background: !canShow3D
                  ? (theme === 'light' ? '#f1f3f5' : '#1a1a1a')
                  : (viewMode as ViewMode) === "3d"
                    ? 'linear-gradient(90deg, #dc2626, #ef4444, #f97316)'
                    : getViewButtonBg(),
                color: !canShow3D
                  ? (theme === 'light' ? 'rgba(0, 0, 0, 0.3)' : 'rgba(255, 255, 255, 0.3)')
                  : (viewMode as ViewMode) === "3d"
                    ? '#ffffff'
                    : getCellTextColor(),
                cursor: !canShow3D ? 'not-allowed' : 'pointer'
              }}
              onMouseEnter={(e) => {
                if (canShow3D && (viewMode as ViewMode) !== "3d") e.currentTarget.style.background = getViewButtonBgHover();
              }}
              onMouseLeave={(e) => {
                if (canShow3D && (viewMode as ViewMode) !== "3d") e.currentTarget.style.background = getViewButtonBg();
              }}
            >
              3D
            </button>
          </div>
        )}
      </div>

      {/* Modal for similar maps - Centered on viewport (workspace area) */}
      {similarMapsModal?.isOpen && (() => {
        // Center on viewport, accounting for left sidebar (approximately 280px)
        const sidebarWidth = 280;
        const centerX = sidebarWidth + (window.innerWidth - sidebarWidth) / 2;
        const centerY = window.innerHeight / 2;

        // Find similar maps (same base name pattern and same dimensions)
        const currentDims = mapData.dimensions?.TwoDimensional;
        const currentRows = currentDims?.rows || 1;
        const currentCols = currentDims?.cols || 1;

        // Extract base name (remove address prefix like "E22FA ")
        const getBaseName = (name: string) => {
          // Remove hex address prefix (e.g., "E22FA " -> "Start of Injection")
          return name.replace(/^[A-Fa-f0-9]{4,6}\s+/, '').trim();
        };
        const currentBaseName = getBaseName(mapData.name);

        const similarMaps = (allMaps || []).filter(m => {
          if (m.address === mapData.address) return false; // Exclude current map
          const dims = m.dimensions?.TwoDimensional;
          const rows = dims?.rows || 1;
          const cols = dims?.cols || 1;
          const baseName = getBaseName(m.name);
          // Same base name AND same dimensions
          return baseName === currentBaseName && rows === currentRows && cols === currentCols;
        });

        // Les axes édités comptent aussi comme modifications à propager
        const origXLabels = originalXAxisLabelsRef.current;
        const origYLabels = originalYAxisLabelsRef.current;
        const axisEdited =
          (xAxisLabels.length === origXLabels.length && xAxisLabels.some((v, i) => v !== origXLabels[i])) ||
          (yAxisLabels.length === origYLabels.length && yAxisLabels.some((v, i) => v !== origYLabels[i]));
        const hasModifications = Object.keys(changedCells).length > 0 || axisEdited;

        return (
        <div
          className="fixed z-[9999] rounded-lg shadow-2xl select-none"
          style={{
            top: centerY,
            left: centerX,
            transform: 'translate(-50%, -50%)',
            background: 'rgba(22, 25, 34, 0.92)',
            border: '1px solid rgba(255, 255, 255, 0.1)',
            backdropFilter: 'blur(18px) saturate(140%)',
            WebkitBackdropFilter: 'blur(18px) saturate(140%)',
            minWidth: '320px',
            maxWidth: '400px',
            maxHeight: '500px',
          }}
        >
          {/* Header */}
          <div
            className="px-3 py-2 flex items-center justify-between"
            style={{ borderBottom: '1px solid rgba(128, 128, 128, 0.3)' }}
          >
            <span className="text-[13px] text-white font-medium">{t.mapViewer.similarMaps}</span>
            <button
              onClick={() => setSimilarMapsModal(null)}
              className="p-0.5 rounded hover:bg-white/10 transition-colors text-gray-400 hover:text-white"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>

          {/* Content */}
          <div className="p-3 flex flex-col gap-3">
            <p className="text-[12px] text-gray-300">
              {t.mapViewer.similarMapsDescription}
            </p>

            {/* Copy type selection */}
            <div className="flex flex-col gap-1">
              <span className="text-[12px] text-gray-400">{t.mapViewer.copyType}</span>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="radio"
                  name="copyType"
                  checked={similarMapsModal.copyType === 'modifications'}
                  onChange={() => setSimilarMapsModal({ ...similarMapsModal, copyType: 'modifications' })}
                  className="w-4 h-4 accent-red-500"
                  disabled={!hasModifications}
                />
                <span className={`text-[12px] ${!hasModifications ? 'text-gray-500' : 'text-white'}`}>
                  {t.mapViewer.modificationsOnly} {!hasModifications && t.mapViewer.noModifications}
                </span>
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="radio"
                  name="copyType"
                  checked={similarMapsModal.copyType === 'all'}
                  onChange={() => setSimilarMapsModal({ ...similarMapsModal, copyType: 'all' })}
                  className="w-4 h-4 accent-red-500"
                />
                <span className="text-[12px] text-white">{t.mapViewer.entireMap}</span>
              </label>
            </div>

            {/* Separator */}
            <div className="border-t border-gray-600" />

            {/* Maps list */}
            <div className="flex flex-col gap-1">
              {similarMaps.length > 0 ? (
                <>
                  <button
                    className="text-[12px] text-red-400 hover:text-red-300 text-left mb-1"
                    onClick={() => {
                      if (similarMapsModal.selectedMaps.length === similarMaps.length) {
                        setSimilarMapsModal({ ...similarMapsModal, selectedMaps: [] });
                      } else {
                        setSimilarMapsModal({ ...similarMapsModal, selectedMaps: similarMaps.map(m => m.address) });
                      }
                    }}
                  >
                    {similarMapsModal.selectedMaps.length === similarMaps.length ? t.mapViewer.deselectAllMaps : t.mapViewer.selectAllMaps}
                  </button>
                  <div className="max-h-[200px] overflow-y-auto flex flex-col gap-1">
                    {similarMaps.map((m) => {
                      const dims = m.dimensions?.TwoDimensional;
                      const dimStr = dims ? `${dims.rows}x${dims.cols}` : '1x1';
                      const isSelected = similarMapsModal.selectedMaps.includes(m.address);
                      return (
                        <label
                          key={m.address}
                          className={`flex items-center gap-2 px-2 py-1.5 rounded cursor-pointer transition-colors ${
                            isSelected ? 'bg-white/10' : 'hover:bg-white/5'
                          }`}
                        >
                          <input
                            type="checkbox"
                            checked={isSelected}
                            onChange={() => {
                              const newSelected = isSelected
                                ? similarMapsModal.selectedMaps.filter(a => a !== m.address)
                                : [...similarMapsModal.selectedMaps, m.address];
                              setSimilarMapsModal({ ...similarMapsModal, selectedMaps: newSelected });
                            }}
                            className="w-4 h-4 accent-red-500 rounded"
                          />
                          <span className="text-[12px] text-white flex-1 truncate">{m.name}</span>
                          <span className="text-[11px] text-gray-500">{dimStr}</span>
                        </label>
                      );
                    })}
                  </div>
                </>
              ) : (
                <p className="text-[12px] text-gray-500 italic">{t.mapViewer.noSimilarMaps}</p>
              )}
            </div>

            {/* Buttons */}
            <div className="flex gap-2 mt-1">
              <button
                type="button"
                onClick={() => setSimilarMapsModal(null)}
                className="flex-1 px-3 py-1.5 rounded text-[12px] font-medium text-gray-300 hover:bg-white/10 transition-colors"
              >
                {t.common.cancel}
              </button>
              <button
                type="button"
                disabled={similarMapsModal.selectedMaps.length === 0 || (similarMapsModal.copyType === 'modifications' && !hasModifications)}
                onClick={() => {
                  if (onApplyToSimilarMaps) {
                    onApplyToSimilarMaps(similarMapsModal.selectedMaps, similarMapsModal.copyType);
                  }
                  setSimilarMapsModal(null);
                  toast({
                    title: t.mapViewer.changesApplied,
                    description: t.mapViewer.changesAppliedDescription
                      .replace('{type}', similarMapsModal.copyType === 'modifications' ? t.mapViewer.modificationsOnly : t.mapViewer.entireMap)
                      .replace('{count}', similarMapsModal.selectedMaps.length.toString()),
                  });
                }}
                className={`flex-1 px-3 py-1.5 rounded text-[12px] font-medium transition-colors ${
                  similarMapsModal.selectedMaps.length > 0 && (similarMapsModal.copyType !== 'modifications' || hasModifications)
                    ? 'text-white bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400'
                    : 'text-gray-500 bg-gray-700 cursor-not-allowed'
                }`}
              >
                {t.mapViewer.change}
              </button>
            </div>
          </div>
        </div>
        );
      })()}

      {/* Modal for absolute value input - Centered on map container */}
      {absoluteValueModal?.isOpen && (() => {
        const containerRect = tableContainerRef.current?.getBoundingClientRect();
        const centerX = containerRect ? containerRect.left + containerRect.width / 2 : window.innerWidth / 2;
        const centerY = containerRect ? containerRect.top + containerRect.height / 2 : window.innerHeight / 2;
        return (
        <div
          className="fixed z-[9999] rounded-lg shadow-2xl select-none"
          style={{
            top: centerY,
            left: centerX,
            transform: 'translate(-50%, -50%)',
            background: 'rgba(22, 25, 34, 0.92)',
            border: '1px solid rgba(255, 255, 255, 0.1)',
            backdropFilter: 'blur(18px) saturate(140%)',
            WebkitBackdropFilter: 'blur(18px) saturate(140%)',
            minWidth: '220px',
          }}
        >
          {/* Header */}
          <div
            className="px-3 py-2 flex items-center justify-between"
            style={{ borderBottom: '1px solid rgba(128, 128, 128, 0.3)' }}
          >
            <span className="text-[12px] text-white font-medium">{t.mapViewer.changeAbsolute}</span>
            <button
              onClick={() => setAbsoluteValueModal(null)}
              className="p-0.5 rounded hover:bg-white/10 transition-colors text-gray-400 hover:text-white"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
          <div className="p-3 flex flex-col gap-2">
            <input
              type="text"
              autoFocus
              value={absoluteValueModal.inputValue}
              onChange={(e) => setAbsoluteValueModal({
                ...absoluteValueModal,
                inputValue: e.target.value
              })}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  if (!absoluteValueModal.inputValue.trim()) return;
                  const parsed = Number(absoluteValueModal.inputValue.replace(",", "."));
                  if (!Number.isNaN(parsed)) {
                    absoluteValueModal.onConfirm(parsed);
                    setAbsoluteValueModal(null);
                  } else {
                    toast({ title: t.mapViewer.invalidValue, description: t.mapViewer.enterNumber, variant: "destructive" });
                  }
                } else if (e.key === 'Escape') {
                  setAbsoluteValueModal(null);
                }
              }}
              className="w-full px-2 py-1.5 rounded text-[12px] text-white bg-white/10 border border-white/20 focus:outline-none"
              placeholder="Enter value..."
              spellCheck={false}
            />
            <div className="flex gap-2 mt-1">
              <button
                type="button"
                onClick={() => setAbsoluteValueModal(null)}
                className="flex-1 px-3 py-1.5 rounded text-[12px] font-medium text-gray-300 hover:bg-white/10 transition-colors"
              >
                {t.common.cancel}
              </button>
              <button
                type="button"
                disabled={!absoluteValueModal.inputValue.trim()}
                onClick={() => {
                  const parsed = Number(absoluteValueModal.inputValue.replace(",", "."));
                  if (!Number.isNaN(parsed)) {
                    absoluteValueModal.onConfirm(parsed);
                    setAbsoluteValueModal(null);
                  } else {
                    toast({ title: t.mapViewer.invalidValue, description: t.mapViewer.enterNumber, variant: "destructive" });
                  }
                }}
                className={`flex-1 px-3 py-1.5 rounded text-[12px] font-medium transition-colors ${
                  absoluteValueModal.inputValue.trim()
                    ? 'text-white bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400'
                    : 'text-gray-500 bg-gray-700 cursor-not-allowed'
                }`}
              >
                {t.mapViewer.ok}
              </button>
            </div>
          </div>
        </div>
        );
      })()}

      {/* Themed value-edit prompt (cell / axis) — replaces window.prompt() */}
      {valuePrompt && (
        <PromptModal
          title={valuePrompt.title}
          description={t.mapViewer.newValueLabel}
          initialValue={valuePrompt.value}
          confirmLabel={t.common.apply}
          cancelLabel={t.common.cancel}
          onConfirm={(input) => {
            valuePrompt.onSubmit(input);
            setValuePrompt(null);
          }}
          onCancel={() => setValuePrompt(null)}
        />
      )}
    </div>
  );
}

