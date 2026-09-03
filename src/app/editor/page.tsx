"use client";

import { useEffect, useState, useRef, useCallback, useMemo } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/button";
import {
  ChevronRight,
  ChevronDown,
  Folder,
  FolderOpen,
  ArrowDownAZ,
  ArrowDownZA,
  ArrowDown01,
  FileText,
  Download,
  Save,
  Upload,
  Edit,
  Trash2,
  ChevronDown as ChevronDownIcon,
  AlertTriangle,
  ChevronUp,
  Cpu,
  MoreVertical,
  X,
  Eye,
  RefreshCw,
  FileJson,
} from "lucide-react";
import { PiHeadCircuit } from "react-icons/pi";
import { HexdumpViewer, type MapRegion } from "@/components/hexdump-viewer";
import { EditorToolbar } from "@/components/editor-toolbar";
import { MapViewer, clearMapDataCache, buildPlot3DTicks } from "@/components/map-viewer";
import { SettingsMenu } from "@/components/settings-menu";
import { setAppZoom, setAppMinWidth } from "@/lib/webview-zoom";
import { DTCModal } from "@/components/dtc-modal";
import { SolutionsModal } from "@/components/solutions-modal";
import { getSolutionImplementation } from "@/lib/ecu/solutions";
import { CompareModal } from "@/components/compare-modal";
import FloatingLines from "@/components/FloatingLines";
import ZedGradientDefs, { ZedFileIcon } from "@/components/zed-gradient-defs";
import { ChecksumModal } from "@/components/checksum-modal";
import { MappackExportModal } from "@/components/mappack-export-modal";
import { MODAL_GLASS, MODAL_GLASS_LIGHT, TOAST_GLASS, TOAST_GLASS_LIGHT } from "@/lib/modal-glass";
import { StyledSelect } from "@/components/styled-select";
import { formatEcuWithManufacturer } from "@/lib/ecu-manufacturer";
import { isBigEndianEcu } from "@/lib/ecu-endianness";
import { ConfirmModal } from "@/components/confirm-modal";
import { PromptModal } from "@/components/prompt-modal";
import { correctChecksumByEcuType, isChecksumSupported, ChecksumResult } from "@/lib/ecu/bosch/checksums";
import { disableDTC, detectDTCs, type DetectedDTC, type CodeblockInfo } from "@/lib/ecu/bosch/dtc";
import { saveBytesToFile } from "@/lib/local/save-file";
import { identifyEcu, bytesToBase64, detectorVersion } from "@/lib/local/detector";
import { ThemeProvider, useTheme } from "@/contexts/theme-context";
import { useSettings } from "@/contexts/settings-context";
import { getCustomWallpaper, subscribeCustomWallpaper } from "@/lib/custom-wallpaper";
import { useI18n } from "@/contexts/i18n-context";
import axios from "axios";
import { useToast } from "@/hooks/use-toast";
import dynamic from "next/dynamic";

// Import Plotly dynamiquement pour éviter les problèmes SSR
const Plot = dynamic(() => import("react-plotly.js"), {
  ssr: false,
  loading: () => null,
});

interface MapData {
  name: string;
  address: number;
  size: number;
  confidence?: number;
  description?: string;
  map_type?: string;
  codeblock_id?: number;
  codeblock_start_address?: number;
  codeblock_end_address?: number;
  dimensions?: {
    TwoDimensional?: {
      rows: number;
      cols: number;
    };
    OneDimensional?: {
      length: number;
    };
  };
  // Axis addresses and correction factors
  x_axis_address?: number;
  y_axis_address?: number;
  x_axis_correction?: number;
  y_axis_correction?: number;
  correction_factor?: number;
  offset?: number;
  x_axis_offset?: number;
  y_axis_offset?: number;
  // Category and subcategory
  category?: string;
  subcategory?: string;
  // Header display info
  x_label?: string;
  y_label?: string;
  unit?: string;
  y_axis_inverted?: boolean;
}

interface ProjectData {
  project_name: string;
  file_name: string;
  original_name: string;
  file_data: number[]; // May be loaded later from PocketBase if not in sessionStorage
  file_size: number;
  vehicle_brand?: string;
  vehicle_model?: string;
  engine_type?: string;
  ecu_type?: string;
  transmission_type?: string;
  year?: string;
  power?: string;
  customer?: string;
  stage?: string;
  notes?: string;
  hardware_version?: string;
  software_version?: string;
  created?: string;
  detectionResults: {
    maps: MapData[];
    total_maps: number;
    processing_time_ms: number;
    /** Version du moteur ayant produit ces résultats (re-scan si périmée). */
    detector_version?: number;
    /** Rapport de complétude EDC16 : familles de maps attendues vs trouvées. */
    expected_maps?: { label: string; expected: number; found: number }[];
  };
  fileId?: string;
  versions?: VersionDto[];
  currentVersionId?: string | null;
  applied_solutions?: Record<string, string>; // { solutionId: versionName } - persisted to backend
}

interface VersionDto {
  id: string;
  fileId: string;
  name: string;
  isCurrent: boolean;
  baseVersionId?: string | null;
  createdAt: string;
}

// Helper to save project data to sessionStorage WITHOUT file_data to avoid quota exceeded
// The file_data is stored in PocketBase and loaded on demand
const saveProjectToSession = (data: ProjectData) => {
  const { file_data, ...dataWithoutFileData } = data;
  try {
    sessionStorage.setItem("currentProject", JSON.stringify(dataWithoutFileData));
  } catch (e) {
    console.error("Error saving project to sessionStorage:", e);
  }
};

// Interface pour les paramètres d'affichage d'une map (persistés par projet)
interface MapDisplaySettings {
  // Onglet Map
  name: string;
  id: string;
  description: string;
  unit: string;
  skipBytesPerLine: number;
  startAddress: string; // Hex format
  width: number;
  height: number;
  // Inverser l'affichage (transposer lignes/colonnes). undefined = comportement
  // par défaut de la map (shouldSwapAxes) ; true/false = override utilisateur qui
  // BASCULE ce défaut. N'affecte QUE l'affichage éditeur, pas l'export mappack.
  invertDisplay?: boolean;
  wordSize: '8b' | '16b';
  dataOrganization: 'HiLo' | 'LoHi';
  signed: boolean;
  factor: number;
  offset: number;
  divisor: number;
  numberFormat: 'Decimal' | 'Hexadecimal';
  precision: number;
  // Onglet X Axis
  xAxis: {
    id: string;
    description: string;
    unit: string;
    dataSource: 'ROM' | 'values'; // 'values' = [1, 2, 3, ...]
    skipBytesPerLine: number;
    startAddress: string;
    wordSize: '8b' | '16b';
    dataOrganization: 'HiLo' | 'LoHi';
    signed: boolean;
    factor: number;
    offset: number;
    divisor: number;
    numberFormat: 'Decimal' | 'Hexadecimal';
    precision: number;
    mirror: boolean; // Carte miroir - inverse l'ordre des valeurs
  };
  // Onglet Y Axis
  yAxis: {
    id: string;
    description: string;
    unit: string;
    dataSource: 'ROM' | 'values';
    skipBytesPerLine: number;
    startAddress: string;
    wordSize: '8b' | '16b';
    dataOrganization: 'HiLo' | 'LoHi';
    signed: boolean;
    factor: number;
    offset: number;
    divisor: number;
    numberFormat: 'Decimal' | 'Hexadecimal';
    precision: number;
    mirror: boolean; // Carte miroir - inverse l'ordre des valeurs
  };
}

// Valeurs par défaut pour les paramètres d'affichage
// Defaults reflect the REAL detected values of the map (correction factors,
// offsets, addresses, dimensions, data type...) so the Properties window
// always shows what the map actually uses — saving without touching anything
// keeps the display strictly identical.
const getDefaultMapDisplaySettings = (map: MapData): MapDisplaySettings => {
  const dims = map.dimensions?.TwoDimensional;
  const oneDim = map.dimensions?.OneDimensional;
  const width = dims?.cols ?? oneDim?.length ?? 1;
  const height = dims?.rows ?? (oneDim ? 1 : 1);
  const dataType = (map as { data_type?: string }).data_type || '';
  const littleEndian = (map as { is_little_endian?: boolean }).is_little_endian === true;
  const cellFactor = map.correction_factor ?? 1;
  return {
    name: map.name || '',
    id: '',
    description: map.description || '',
    unit: map.unit || '',
    skipBytesPerLine: 0,
    startAddress: map.address.toString(16).toUpperCase(),
    width,
    height,
    wordSize: dataType.includes('8') ? '8b' : '16b',
    dataOrganization: littleEndian ? 'LoHi' : 'HiLo',
    signed: dataType === 'Int16' || dataType === 'Int8',
    factor: cellFactor,
    offset: map.offset ?? 0,
    divisor: 1,
    numberFormat: 'Decimal',
    // Same default the viewer uses for cells (1 decimal)
    precision: 1,
    xAxis: {
      id: '',
      description: '',
      unit: map.x_label || '',
      dataSource: map.x_axis_address ? 'ROM' : 'values',
      skipBytesPerLine: 0,
      startAddress: map.x_axis_address ? map.x_axis_address.toString(16).toUpperCase() : '',
      wordSize: '16b',
      dataOrganization: 'HiLo',
      signed: false,
      factor: map.x_axis_correction ?? 1,
      offset: map.x_axis_offset ?? 0,
      divisor: 1,
      numberFormat: 'Decimal',
      // Same decimals rule the viewer applies to axis labels
      precision: (map.x_axis_correction ?? 1) < 0.1 ? 2 : (map.x_axis_correction ?? 1) < 1.0 ? 1 : 0,
      mirror: false,
    },
    yAxis: {
      id: '',
      description: '',
      unit: map.y_label || '',
      dataSource: map.y_axis_address ? 'ROM' : 'values',
      skipBytesPerLine: 0,
      startAddress: map.y_axis_address ? map.y_axis_address.toString(16).toUpperCase() : '',
      wordSize: '16b',
      dataOrganization: 'HiLo',
      signed: false,
      factor: map.y_axis_correction ?? 1,
      offset: map.y_axis_offset ?? 0,
      divisor: 1,
      numberFormat: 'Decimal',
      precision: 0,
      mirror: false,
    },
  };
};

// Composant MapPropertiesModal pour éditer les paramètres d'affichage d'une map
interface MapPropertiesModalProps {
  mapData: MapData;
  settings: MapDisplaySettings;
  onClose: () => void;
  onSave: (settings: MapDisplaySettings) => void;
  isClosing?: boolean;
  theme?: 'default' | 'light' | 'oled';
  workspaceRef?: React.RefObject<HTMLDivElement>;
}

function MapPropertiesModal({
  mapData,
  settings,
  onClose,
  onSave,
  isClosing = false,
  theme = 'default',
  workspaceRef,
}: MapPropertiesModalProps) {
  const [activeTab, setActiveTab] = useState<'map' | 'xAxis' | 'yAxis'>('map');
  const [localSettings, setLocalSettings] = useState<MapDisplaySettings>(settings);
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [isDragging, setIsDragging] = useState(false);
  const [hasMoved, setHasMoved] = useState(false); // Track if user has moved the modal
  const dragStartRef = useRef({ x: 0, y: 0, posX: 0, posY: 0 });
  const modalRef = useRef<HTMLDivElement>(null);

  const handleSave = () => {
    onSave(localSettings);
    onClose();
  };

  const updateMapSetting = <K extends keyof MapDisplaySettings>(key: K, value: MapDisplaySettings[K]) => {
    setLocalSettings(prev => ({ ...prev, [key]: value }));
  };

  const updateXAxisSetting = <K extends keyof MapDisplaySettings['xAxis']>(key: K, value: MapDisplaySettings['xAxis'][K]) => {
    setLocalSettings(prev => ({
      ...prev,
      xAxis: { ...prev.xAxis, [key]: value }
    }));
  };

  const updateYAxisSetting = <K extends keyof MapDisplaySettings['yAxis']>(key: K, value: MapDisplaySettings['yAxis'][K]) => {
    setLocalSettings(prev => ({
      ...prev,
      yAxis: { ...prev.yAxis, [key]: value }
    }));
  };

  // Drag handlers
  const handleDragStart = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest('button')) return;
    e.preventDefault();
    setIsDragging(true);
    dragStartRef.current = {
      x: e.clientX,
      y: e.clientY,
      posX: position.x,
      posY: position.y
    };
  };

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      const dx = e.clientX - dragStartRef.current.x;
      const dy = e.clientY - dragStartRef.current.y;

      // Calculate new position
      let newX = dragStartRef.current.posX + dx;
      let newY = dragStartRef.current.posY + dy;

      // Get modal dimensions and workspace bounds (like map windows)
      const modalRect = modalRef.current?.getBoundingClientRect();
      const workspaceRect = workspaceRef?.current?.getBoundingClientRect();

      if (modalRect && workspaceRect) {
        const modalWidth = modalRect.width;
        const modalHeight = modalRect.height;

        // Calculate bounds relative to workspace (same logic as clampPosition for maps)
        // The modal is centered by default, so position is offset from center
        const centerX = workspaceRect.width / 2;
        const centerY = workspaceRect.height / 2;

        // Max bounds: modal must stay within workspace
        const minX = -centerX + modalWidth / 2;
        const maxX = centerX - modalWidth / 2;
        const minY = -centerY + modalHeight / 2;
        const maxY = centerY - modalHeight / 2;

        // Clamp position within bounds
        newX = Math.max(minX, Math.min(maxX, newX));
        newY = Math.max(minY, Math.min(maxY, newY));
      }

      setPosition({ x: newX, y: newY });
      setHasMoved(true); // Mark that user has moved the modal
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging, workspaceRef]);

  // Styles adaptés au thème (même style que ProjectInfoEditModal)
  const isLight = theme === 'light';
  const inputClass = isLight
    ? "w-full px-3 py-2 bg-gray-100 border border-gray-300 rounded-lg text-black placeholder:text-gray-400 focus:outline-none focus:ring-0 text-sm"
    : "w-full px-3 py-2 bg-black/15 border border-white/20 rounded-lg text-white placeholder:text-white/50 focus:outline-none focus:ring-0 text-sm";
  const labelClass = isLight
    ? "block text-sm font-medium mb-1.5 text-gray-700"
    : "block text-sm font-medium mb-1.5 text-white";
  const checkboxClass = "w-5 h-5 rounded border-white/30 text-red-600 focus:ring-red-500 cursor-pointer bg-black/15";
  const smallInputClass = isLight
    ? "px-2 py-1.5 bg-gray-100 border border-gray-300 rounded text-black text-center text-sm"
    : "px-2 py-1.5 bg-black/15 border border-white/20 rounded text-white text-center text-sm";
  const textMutedClass = isLight ? "text-gray-500" : "text-white/50";

  return (
    <div
      className="absolute inset-0 z-[70] flex items-center justify-center pointer-events-none"
    >
      <div
        ref={modalRef}
        className={`relative w-full max-w-2xl pointer-events-auto shadow-2xl shadow-black/50`}
        style={{
          transform: `translate(${position.x}px, ${position.y}px)`,
          animation: isDragging || hasMoved ? 'none' : (isClosing ? 'modalCollapse 0.2s ease-out forwards' : 'modalExpand 0.2s ease-out forwards'),
          cursor: isDragging ? 'move' : undefined
        }}
      >
        {/* Close button */}
        <button
          onClick={onClose}
          className={`absolute top-4 right-4 z-10 p-2 rounded-lg transition-colors ${
            isLight
              ? 'hover:bg-black/5 text-black/50 hover:text-black'
              : 'hover:bg-white/5 text-white/60 hover:text-white'
          }`}
        >
          <X className="w-5 h-5" />
        </button>

        {/* Content */}
        <div
          className="border rounded-lg p-5 min-h-[420px]"
          style={{
            backgroundColor: isLight
              ? 'rgba(255, 255, 255, 0.9)'
              : theme === 'oled'
                ? 'rgba(15, 15, 18, 0.95)'
                : 'rgba(22, 25, 34, 0.92)',
            borderColor: isLight ? 'rgba(0, 0, 0, 0.1)' : 'rgba(255, 255, 255, 0.1)',
            backdropFilter: 'blur(18px) saturate(140%)',
            WebkitBackdropFilter: 'blur(18px) saturate(140%)'
          }}
        >
          {/* Draggable header - zone élargie pour faciliter le drag */}
          <div
            className={`-mx-5 -mt-5 px-5 pt-5 pb-3 mb-3 select-none cursor-move`}
            onMouseDown={handleDragStart}
          >
            <h2 className={`text-xl font-bold ${isLight ? 'text-black' : 'text-white'} pointer-events-none`}>
              Properties
              <span className={`ml-2 text-sm font-normal ${textMutedClass}`}>
                {mapData.name || ''} — ${mapData.address.toString(16).toUpperCase()}
              </span>
            </h2>
          </div>

          {/* Tabs */}
          <div className={`flex border-b mb-3 ${isLight ? 'border-gray-200' : 'border-white/20'}`}>
            {(['map', 'xAxis', 'yAxis'] as const).map((tab) => (
              <button
                key={tab}
                className={`px-4 py-2 text-sm font-medium transition-colors relative ${
                  activeTab === tab
                    ? (isLight ? 'text-black' : 'text-white')
                    : (isLight ? 'text-gray-500 hover:text-gray-700' : 'text-white/50 hover:text-white/80')
                }`}
                onClick={() => setActiveTab(tab)}
              >
                {tab === 'map' ? 'Map' : tab === 'xAxis' ? 'X Axis' : 'Y Axis'}
                {activeTab === tab && (
                  <div className={`absolute bottom-0 left-0 right-0 h-0.5 ${isLight ? 'bg-black' : 'bg-white'}`} />
                )}
              </button>
            ))}
          </div>

          {/* Content - Map Tab */}
          {activeTab === 'map' && (
            <div className="space-y-3">
              {/* Row 1: Name */}
              <div>
                <label className={labelClass}>Name</label>
                <input type="text" value={localSettings.name} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} placeholder="Map name" />
              </div>

              {/* Row 3: Start address, Width x Height, Skip bytes */}
              <div className="grid grid-cols-3 gap-3">
                <div>
                  <label className={labelClass}>Start address</label>
                  <input type="text" value={'$' + localSettings.startAddress} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed font-mono`} placeholder="$DDA7C" />
                </div>
                <div>
                  <label className={labelClass}>Width x Height</label>
                  <div className="flex gap-1">
                    <input type="number" value={localSettings.width} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} min={1} />
                    <input type="number" value={localSettings.height} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} min={1} />
                  </div>
                </div>
                <div>
                  <label className={labelClass}>Skip bytes/line</label>
                  <input type="number" value={localSettings.skipBytesPerLine} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} min={0} />
                </div>
              </div>

              {/* Row 4: Word size, Data org, Number format, Precision */}
              <div className="grid grid-cols-4 gap-3">
                <div>
                  <label className={labelClass}>Word size</label>
                  <input type="text" value={localSettings.wordSize} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} />
                </div>
                <div>
                  <label className={labelClass}>Data org</label>
                  <input type="text" value={localSettings.dataOrganization} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} />
                </div>
                <div>
                  <label className={labelClass}>Format</label>
                  <input type="text" value={localSettings.numberFormat} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} />
                </div>
                <div>
                  <label className={labelClass}>Precision</label>
                  <input type="number" value={localSettings.precision} onChange={(e) => updateMapSetting('precision', Math.max(0, Math.min(6, parseInt(e.target.value) || 0)))} className={inputClass} min={0} max={6} />
                </div>
              </div>

              {/* Row 5: Factor + Sign */}
              <div className="flex items-end gap-3">
                <div className="flex-1">
                  <label className={labelClass}>Factor</label>
                  <div className={`flex items-center gap-2 text-sm ${isLight ? 'text-gray-700' : 'text-white'}`}>
                    <span>Value =</span>
                    <input type="number" step="0.001" value={localSettings.factor} onChange={(e) => updateMapSetting('factor', parseFloat(e.target.value) || 1)} className={`w-20 ${smallInputClass}`} />
                    <span>× Eprom +</span>
                    <input type="number" step="0.1" value={localSettings.offset} onChange={(e) => updateMapSetting('offset', parseFloat(e.target.value) || 0)} className={`w-16 ${smallInputClass}`} />
                    <span>÷</span>
                    <input type="number" step="1" value={localSettings.divisor} onChange={(e) => updateMapSetting('divisor', parseFloat(e.target.value) || 1)} className={`w-14 ${smallInputClass}`} />
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Content - X Axis Tab */}
          {activeTab === 'xAxis' && (
            <div className="space-y-3">
              {/* Row 1: Unit, Data source, Start address */}
              <div className="grid grid-cols-3 gap-3">
                <div>
                  <label className={labelClass}>Unit</label>
                  <input type="text" value={localSettings.xAxis.unit} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} placeholder="RPM" />
                </div>
                <div>
                  <label className={labelClass}>Data source</label>
                  <input type="text" value={localSettings.xAxis.dataSource === 'values' ? '[1, 2, 3, ...]' : 'ROM'} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} />
                </div>
                <div>
                  <label className={labelClass}>Start address</label>
                  <input type="text" value={localSettings.xAxis.startAddress ? '$' + localSettings.xAxis.startAddress : ''} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed font-mono`} placeholder="$DDA78" />
                </div>
              </div>

              {/* Row 2: Skip bytes, Word size, Data org, Format */}
              <div className="grid grid-cols-4 gap-3">
                <div>
                  <label className={labelClass}>Skip bytes/line</label>
                  <input type="number" value={localSettings.xAxis.skipBytesPerLine} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} min={0} />
                </div>
                <div>
                  <label className={labelClass}>Word size</label>
                  <input type="text" value={localSettings.xAxis.wordSize} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} />
                </div>
                <div>
                  <label className={labelClass}>Data org</label>
                  <input type="text" value={localSettings.xAxis.dataOrganization} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} />
                </div>
                <div>
                  <label className={labelClass}>Format</label>
                  <input type="text" value={localSettings.xAxis.numberFormat} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} />
                </div>
              </div>

              {/* Row 4: Factor + Sign + Mirror */}
              <div className="flex items-end gap-3">
                <div className="flex-1">
                  <label className={labelClass}>Factor</label>
                  <div className={`flex items-center gap-2 text-sm ${isLight ? 'text-gray-700' : 'text-white'}`}>
                    <span>Value =</span>
                    <input type="number" step="0.001" value={localSettings.xAxis.factor} onChange={(e) => updateXAxisSetting('factor', parseFloat(e.target.value) || 1)} className={`w-20 ${smallInputClass}`} />
                    <span>× Eprom +</span>
                    <input type="number" step="0.1" value={localSettings.xAxis.offset} onChange={(e) => updateXAxisSetting('offset', parseFloat(e.target.value) || 0)} className={`w-16 ${smallInputClass}`} />
                    <span>÷</span>
                    <input type="number" step="1" value={localSettings.xAxis.divisor} onChange={(e) => updateXAxisSetting('divisor', parseFloat(e.target.value) || 1)} className={`w-14 ${smallInputClass}`} />
                  </div>
                </div>
                <div className="w-24">
                  <label className={labelClass}>Precision</label>
                  <input type="number" value={localSettings.xAxis.precision} onChange={(e) => updateXAxisSetting('precision', Math.max(0, Math.min(6, parseInt(e.target.value) || 0)))} className={inputClass} min={0} max={6} />
                </div>
              </div>

              {/* Mirror option */}
              <div className={`flex items-center gap-3 p-2.5 rounded-lg border ${isLight ? 'bg-blue-50 border-blue-200' : 'bg-blue-500/10 border-blue-500/30'}`}>
                <input type="checkbox" checked={localSettings.xAxis.mirror} onChange={(e) => updateXAxisSetting('mirror', e.target.checked)} className={checkboxClass} id="xAxisMirror" />
                <label htmlFor="xAxisMirror" className={`text-sm font-medium cursor-pointer ${isLight ? 'text-blue-800' : 'text-blue-300'}`}>Mirror map (reverse axis order)</label>
              </div>
            </div>
          )}

          {/* Content - Y Axis Tab */}
          {activeTab === 'yAxis' && (
            <div className="space-y-3">
              {/* Row 1: Unit, Data source, Start address */}
              <div className="grid grid-cols-3 gap-3">
                <div>
                  <label className={labelClass}>Unit</label>
                  <input type="text" value={localSettings.yAxis.unit} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} placeholder="mg/str" />
                </div>
                <div>
                  <label className={labelClass}>Data source</label>
                  <input type="text" value={localSettings.yAxis.dataSource === 'values' ? '[1, 2, 3, ...]' : 'ROM'} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} />
                </div>
                <div>
                  <label className={labelClass}>Start address</label>
                  <input type="text" value={localSettings.yAxis.startAddress ? '$' + localSettings.yAxis.startAddress : ''} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed font-mono`} placeholder="$Hex" />
                </div>
              </div>

              {/* Row 2: Skip bytes, Word size, Data org, Format */}
              <div className="grid grid-cols-4 gap-3">
                <div>
                  <label className={labelClass}>Skip bytes/line</label>
                  <input type="number" value={localSettings.yAxis.skipBytesPerLine} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} min={0} />
                </div>
                <div>
                  <label className={labelClass}>Word size</label>
                  <input type="text" value={localSettings.yAxis.wordSize} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} />
                </div>
                <div>
                  <label className={labelClass}>Data org</label>
                  <input type="text" value={localSettings.yAxis.dataOrganization} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} />
                </div>
                <div>
                  <label className={labelClass}>Format</label>
                  <input type="text" value={localSettings.yAxis.numberFormat} readOnly disabled className={`${inputClass} opacity-60 cursor-not-allowed`} />
                </div>
              </div>

              {/* Row 4: Factor + Sign + Mirror */}
              <div className="flex items-end gap-3">
                <div className="flex-1">
                  <label className={labelClass}>Factor</label>
                  <div className={`flex items-center gap-2 text-sm ${isLight ? 'text-gray-700' : 'text-white'}`}>
                    <span>Value =</span>
                    <input type="number" step="0.001" value={localSettings.yAxis.factor} onChange={(e) => updateYAxisSetting('factor', parseFloat(e.target.value) || 1)} className={`w-20 ${smallInputClass}`} />
                    <span>× Eprom +</span>
                    <input type="number" step="0.1" value={localSettings.yAxis.offset} onChange={(e) => updateYAxisSetting('offset', parseFloat(e.target.value) || 0)} className={`w-16 ${smallInputClass}`} />
                    <span>÷</span>
                    <input type="number" step="1" value={localSettings.yAxis.divisor} onChange={(e) => updateYAxisSetting('divisor', parseFloat(e.target.value) || 1)} className={`w-14 ${smallInputClass}`} />
                  </div>
                </div>
                <div className="w-24">
                  <label className={labelClass}>Precision</label>
                  <input type="number" value={localSettings.yAxis.precision} onChange={(e) => updateYAxisSetting('precision', Math.max(0, Math.min(6, parseInt(e.target.value) || 0)))} className={inputClass} min={0} max={6} />
                </div>
              </div>

              {/* Mirror option */}
              <div className={`flex items-center gap-3 p-2.5 rounded-lg border ${isLight ? 'bg-blue-50 border-blue-200' : 'bg-blue-500/10 border-blue-500/30'}`}>
                <input type="checkbox" checked={localSettings.yAxis.mirror} onChange={(e) => updateYAxisSetting('mirror', e.target.checked)} className={checkboxClass} id="yAxisMirror" />
                <label htmlFor="yAxisMirror" className={`text-sm font-medium cursor-pointer ${isLight ? 'text-blue-800' : 'text-blue-300'}`}>Mirror map (reverse axis order)</label>
              </div>
            </div>
          )}

          {/* Footer */}
          <div className={`flex justify-between items-center gap-3 pt-4 mt-4 border-t ${isLight ? 'border-gray-200' : 'border-white/20'}`}>
            <button
              onClick={() => setLocalSettings(getDefaultMapDisplaySettings(mapData))}
              className={`px-4 py-1.5 rounded-lg transition-colors font-medium border ${isLight ? 'text-gray-600 border-gray-300 hover:bg-gray-100' : 'text-white/70 border-white/20 hover:bg-white/10'}`}
              title="Restore the detected values"
            >
              Reset
            </button>
            <div className="flex items-center gap-3">
              <button onClick={onClose} className={`px-4 py-1.5 rounded-lg transition-colors font-medium ${isLight ? 'text-gray-600 hover:bg-gray-100' : 'text-white/70 hover:bg-white/10'}`}>Cancel</button>
              <button onClick={handleSave} className="px-5 py-1.5 bg-red-600 text-white rounded-lg hover:bg-red-700 transition-colors font-medium">Save</button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

// Function to get icon styles based on stage
function getStageIconStyles(stage?: string, isLight?: boolean) {
  switch (stage) {
    case 'Stage 1':
      return {
        background: 'bg-green-500/20',
        border: 'border-green-500/30',
        numberColor: 'text-green-500',
        stageNumber: '1'
      };
    case 'Stage 2':
      return {
        background: 'bg-yellow-500/20',
        border: 'border-yellow-500/30',
        numberColor: 'text-yellow-500',
        stageNumber: '2'
      };
    case 'Stage 3':
      return {
        background: 'bg-red-500/20',
        border: 'border-red-500/30',
        numberColor: 'text-red-500',
        stageNumber: '3'
      };
    default:
      return {
        background: isLight ? 'bg-black/[0.04]' : 'bg-slate-500/20',
        border: isLight ? 'border-black/10' : 'border-slate-500/30',
        numberColor: null,
        stageNumber: null
      };
  }
}

// Project Info Edit Modal Component (same as dashboard)
function ProjectInfoEditModal({
  projectData,
  onClose,
  onSave,
  isClosing = false,
}: {
  projectData: ProjectData;
  onClose: () => void;
  onSave: (updatedInfo: Partial<ProjectData>) => void;
  isClosing?: boolean;
}) {
  const { t } = useI18n();
  // La modale suit le thème de son écran (dashboard ou éditeur)
  const { theme } = useTheme();
  const L = theme === 'light';
  const labelCls = `block text-sm font-medium mb-2 ${L ? 'text-slate-900' : 'text-white'}`;
  const inputCls = `w-full px-3 py-2 rounded-lg focus:outline-none focus:ring-0 ${L ? 'bg-black/[0.05] border border-black/20 text-slate-900 placeholder:text-black/40' : 'bg-black/15 border border-white/20 text-white placeholder:text-white/50'}`;
  const inputSmCls = `w-full px-2 py-2 rounded-lg text-sm focus:outline-none focus:ring-0 ${L ? 'bg-black/[0.05] border border-black/20 text-slate-900 placeholder:text-black/40' : 'bg-black/15 border border-white/20 text-white placeholder:text-white/50'}`;
  const [projectName, setProjectName] = useState(projectData.project_name || projectData.original_name || "");
  const [vehicleBrand, setVehicleBrand] = useState(projectData.vehicle_brand || "");
  const [vehicleModel, setVehicleModel] = useState(projectData.vehicle_model || "");
  const [engineType, setEngineType] = useState(projectData.engine_type || "");
  const [transmissionType, setTransmissionType] = useState(projectData.transmission_type || "");
  const [year, setYear] = useState(projectData.year || "");
  const [power, setPower] = useState(projectData.power || "");
  const [customer, setCustomer] = useState(projectData.customer || "");
  const [stage, setStage] = useState(projectData.stage || "");
  const [notes, setNotes] = useState(projectData.notes || "");
  const date = projectData.created ? new Date(projectData.created).toISOString().split('T')[0] : new Date().toISOString().split('T')[0];

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSave({
      project_name: projectName,
      vehicle_brand: vehicleBrand,
      vehicle_model: vehicleModel,
      engine_type: engineType,
      transmission_type: transmissionType,
      year: year,
      power: power,
      customer: customer,
      stage: stage,
      notes: notes,
    });
  };

  const brands = ["Audi", "Seat", "Skoda", "Volkswagen"];
  const stages = ["Stage 1", "Stage 2", "Stage 3"];
  const transmissions = ["Automatic", "Manual"];
  const years = Array.from({ length: new Date().getFullYear() - 1996 }, (_, i) => new Date().getFullYear() - i);

  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center backdrop-blur-sm"
      style={{
        backgroundColor: '#000000a2',
        animation: isClosing ? 'backdropFadeOut 0.2s ease-out forwards' : 'backdropFadeIn 0.2s ease-out forwards'
      }}
      // Ne se ferme JAMAIS au clic sur le fond — uniquement via les boutons
      onClick={(e) => e.stopPropagation()}
    >
      <div
        className="relative w-full max-w-4xl max-h-[95vh] overflow-y-auto upload-scroll"
        style={{
          animation: isClosing ? 'modalCollapse 0.2s ease-out forwards' : 'modalExpand 0.2s ease-out forwards'
        }}
      >
        {/* Close button */}
        <button
          onClick={onClose}
          className={`absolute top-4 right-4 z-10 p-2 rounded-lg transition-colors ${L ? 'hover:bg-black/5' : 'hover:bg-white/5'}`}
          style={{ color: L ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.6)' }}
        >
          <X className="w-5 h-5" />
        </button>

        {/* Project Info Form */}
        <div className="max-w-4xl mx-auto">
          <div className="border rounded-lg p-8" style={L ? MODAL_GLASS_LIGHT : MODAL_GLASS}>
            <h2 className={`text-2xl font-bold mb-6 ${L ? "text-slate-900" : "text-white"}`}>{t.projectInfo.title}</h2>

            <form onSubmit={handleSubmit} className="space-y-6">
              {/* File Info Section */}
              <div>
                <label className={labelCls}>{t.projectInfo.ecuFile}</label>
                <div className="p-4 rounded-lg border" style={{ backgroundColor: L ? 'rgba(0, 0, 0, 0.05)' : 'rgba(142, 142, 142, 0.13)' }}>
                  <div className="flex items-center gap-4">
                    {(() => {
                      const iconStyles = getStageIconStyles(stage, L);
                      return (
                        <div className={`w-12 h-12 rounded-xl ${iconStyles.background} flex items-center justify-center border ${iconStyles.border}`}>
                          {iconStyles.stageNumber ? (
                            <span className="text-xl font-bold" style={{ fontStyle: 'italic' }}>
                              <span className={L ? "text-slate-900" : "text-white"}>ST</span>
                              <span className={iconStyles.numberColor}>{iconStyles.stageNumber}</span>
                            </span>
                          ) : (
                            <ZedFileIcon className="w-6 h-6" barColor={L ? "#334155" : "#ffffff"} />
                          )}
                        </div>
                      );
                    })()}
                    <div className="flex-1 min-w-0">
                      <h4 className={`font-semibold truncate ${L ? "text-slate-900" : "text-white"}`} title={projectData.original_name}>{projectData.original_name}</h4>
                      <div className="flex items-center gap-4">
                        <p className="text-sm text-slate-400">
                          {(projectData.file_size / 1024).toFixed(2)} KB
                        </p>
                        <div className="flex items-center gap-4 flex-1 justify-center">
                          {projectData.hardware_version && (
                            <div className="flex items-center gap-2 text-sm text-slate-400">
                              <span>HW:</span>
                              <span>{projectData.hardware_version}</span>
                            </div>
                          )}
                          {projectData.software_version && (
                            <div className="flex items-center gap-2 text-sm text-slate-400">
                              <span>SW:</span>
                              <span>{projectData.software_version}</span>
                            </div>
                          )}
                        </div>
                      </div>
                    </div>
                    {projectData.ecu_type && (
                      <span className={`text-xs px-2 py-0.5 rounded-full text-white ${L ? "bg-gradient-to-r from-red-600 via-red-500 to-orange-500 border border-red-600/50" : "bg-gradient-to-r from-red-600/50 via-red-500/50 to-orange-500/50 border border-red-500/40"}`}>
                        {projectData.ecu_type}
                      </span>
                    )}
                  </div>
                </div>
              </div>

              {/* Project Name */}
              <div>
                <label className={labelCls}>{t.projectInfo.projectName}</label>
                <input
                  type="text"
                  value={projectName}
                  onChange={(e) => setProjectName(e.target.value)}
                  className={inputCls}
                  placeholder={t.projectInfo.projectNamePlaceholder}
                  spellCheck={false}
                />
              </div>

              {/* Vehicle Information */}
              <div className="border-t pt-6">
                <h3 className={`text-lg font-semibold mb-4 ${L ? "text-slate-900" : "text-white"}`}>{t.projectInfo.vehicleInfo}</h3>
                <div className="grid md:grid-cols-3 gap-3">
                  {/* Première ligne: Brand - Model - Year */}
                  <div>
                    <label className={labelCls}>{t.projectInfo.brand}</label>
                    <StyledSelect
                      appearance="auto"
                      value={vehicleBrand}
                      onChange={setVehicleBrand}
                      className="w-full"
                      options={[
                        { value: "", label: t.projectInfo.select },
                        ...brands.map((brand) => ({ value: brand, label: brand })),
                      ]}
                    />
                  </div>

                  <div>
                    <label className={labelCls}>{t.projectInfo.model}</label>
                    <input
                      type="text"
                      value={vehicleModel}
                      onChange={(e) => setVehicleModel(e.target.value)}
                      className={inputSmCls}
                      placeholder={t.projectInfo.modelPlaceholder}
                      spellCheck={false}
                    />
                  </div>

                  <div>
                    <label className={labelCls}>{t.projectInfo.year}</label>
                    <StyledSelect
                      appearance="auto"
                      value={String(year)}
                      onChange={setYear}
                      className="w-full"
                      options={[
                        { value: "", label: t.projectInfo.select },
                        ...years.map((y) => ({ value: String(y), label: String(y) })),
                      ]}
                    />
                  </div>

                  {/* Deuxième ligne: Engine Type - Power (HP) - Transmission */}
                  <div>
                    <label className={labelCls}>{t.projectInfo.engineType}</label>
                    <input
                      type="text"
                      value={engineType}
                      onChange={(e) => setEngineType(e.target.value)}
                      className={inputSmCls}
                      placeholder={t.projectInfo.engineTypePlaceholder}
                      spellCheck={false}
                    />
                  </div>

                  <div>
                    <label className={labelCls}>{t.projectInfo.power}</label>
                    <input
                      type="text"
                      value={power}
                      onChange={(e) => setPower(e.target.value)}
                      className={inputSmCls}
                      placeholder={t.projectInfo.powerPlaceholder}
                      spellCheck={false}
                    />
                  </div>

                  <div>
                    <label className={labelCls}>{t.projectInfo.transmission}</label>
                    <StyledSelect
                      appearance="auto"
                      value={transmissionType}
                      onChange={setTransmissionType}
                      className="w-full"
                      options={[
                        { value: "", label: t.projectInfo.select },
                        { value: "Automatic", label: t.projectInfo.automatic },
                        { value: "Manual", label: t.projectInfo.manual },
                      ]}
                    />
                  </div>

                  {/* Troisième ligne: Customer - Stage - Date */}
                  <div>
                    <label className={labelCls}>{t.projectInfo.customer}</label>
                    <input
                      type="text"
                      value={customer}
                      onChange={(e) => setCustomer(e.target.value)}
                      className={inputSmCls}
                      placeholder={t.projectInfo.customerPlaceholder}
                      spellCheck={false}
                    />
                  </div>

                  <div>
                    <label className={labelCls}>{t.projectInfo.stage}</label>
                    <StyledSelect
                      appearance="auto"
                      value={stage}
                      onChange={setStage}
                      className="w-full"
                      options={[
                        { value: "", label: t.projectInfo.select },
                        ...stages.map((s) => ({ value: s, label: s })),
                      ]}
                    />
                  </div>

                  <div>
                    <label className={labelCls}>{t.projectInfo.date}</label>
                    <input
                      type="date"
                      value={date}
                      readOnly
                      className={`${inputSmCls} cursor-not-allowed opacity-75`}
                    />
                  </div>
                </div>
              </div>

              {/* Notes */}
              <div>
                <label className={labelCls}>{t.projectInfo.notes}</label>
                <textarea
                  value={notes}
                  onChange={(e) => setNotes(e.target.value)}
                  rows={3}
                  className={`${inputCls} resize-none`}
                  placeholder={t.projectInfo.notesPlaceholder}
                  spellCheck={false}
                />
              </div>

              {/* Action Buttons */}
              <div className="flex gap-3 pt-4">
                <Button
                  type="submit"
                  className="flex-1 bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400 text-white"
                >
                  {t.common.save}
                </Button>
                <Button
                  type="button"
                  onClick={onClose}
                  variant="outline"
                  className="flex-1"
                >
                  {t.common.close}
                </Button>
              </div>
            </form>
          </div>
        </div>
      </div>
    </div>
  );
}

// Composant PreviewWindow pour afficher un graphique 3D de la map active
interface PreviewWindowProps {
  activeMapAddress: number | null;
  openMaps: MapData[];
  plot3DDataMap: Map<number, {
    plot3DData: any[];
    xAxisLabels: string[];
    yAxisLabels: string[];
    canShow3D: boolean;
  }>;
  dataVersion: number; // Déclenche un re-render quand les données changent
  mapEasyViewStatus: Map<number, boolean>;
  zIndex: number;
  layout: { x: number; y: number; width: number; height: number };
  onLayoutChange: (layout: { x: number; y: number; width: number; height: number }) => void;
  onClose: () => void;
  onFocus: () => void;
  theme: 'default' | 'light' | 'oled';
  getWindowHeaderBg: () => string;
  getWindowHeaderTextColor: () => string;
  getBorderColor: () => string;
  getButtonHoverClass: () => string;
  getWindowBg: () => string;
  workspaceRef: React.RefObject<HTMLDivElement | null>;
  t: any; // i18n translations
  onDragActiveChange?: (active: boolean) => void; // notifie le parent pour l'overlay anti-interaction
}

// Cache global pour sauvegarder la position de caméra de chaque map dans la Preview
// Persiste entre les re-renders et les montages/démontages du composant
const previewCameraPositions: Record<number, { eye: {x: number, y: number, z: number}, center: {x: number, y: number, z: number}, up: {x: number, y: number, z: number} }> = {};

function PreviewWindow({
  activeMapAddress,
  openMaps,
  plot3DDataMap,
  dataVersion,
  mapEasyViewStatus,
  zIndex,
  layout,
  onLayoutChange,
  onClose,
  onFocus,
  theme,
  getWindowHeaderBg,
  getWindowHeaderTextColor,
  getBorderColor,
  getButtonHoverClass,
  getWindowBg,
  workspaceRef,
  t,
  onDragActiveChange,
}: PreviewWindowProps) {
  // dataVersion est utilisé pour déclencher un re-render quand les données changent
  // sans recréer le composant (la clé reste stable)
  const windowRef = useRef<HTMLDivElement>(null);
  const dragState = useRef<{ startX: number; startY: number; originX: number; originY: number } | null>(null);
  const resizeState = useRef<{ startX: number; startY: number; originW: number; originH: number } | null>(null);

  // Fonction pour contraindre la position dans les limites du workspace (comme les fenêtres de maps)
  const clampPosition = (x: number, y: number, width: number, height: number) => {
    const workspaceRect = workspaceRef.current?.getBoundingClientRect();
    if (!workspaceRect) return { x, y };
    const maxX = Math.max(0, workspaceRect.width - width);
    const maxY = Math.max(0, workspaceRect.height - height);
    return {
      x: Math.min(Math.max(0, x), maxX),
      y: Math.min(Math.max(0, y), maxY),
    };
  };

  // Trouver la map active et ses données 3D
  const activeMap = activeMapAddress ? openMaps.find(m => m.address === activeMapAddress) : null;
  const activeMapData = activeMapAddress ? plot3DDataMap.get(activeMapAddress) : null;
  const isActiveMapEasyView = activeMapAddress ? mapEasyViewStatus.get(activeMapAddress) || false : false;
  // Étiquettes d'axes du preview : la surface est tracée en indices
  // (espacement uniforme), les vraies valeurs viennent en ticktext
  const previewTicks = activeMapData && activeMapData.canShow3D
    ? buildPlot3DTicks(activeMapData.xAxisLabels, activeMapData.yAxisLabels)
    : null;

  // Fonction pour obtenir la position de caméra (sauvegardée ou par défaut)
  // Calculée à chaque render pour toujours avoir la dernière position
  const getCameraPosition = () => {
    if (!activeMapAddress) return null;

    // Si position déjà sauvegardée, la retourner
    if (previewCameraPositions[activeMapAddress]) {
      return previewCameraPositions[activeMapAddress];
    }

    // Sinon, position par défaut: coin bas-gauche (caméra rapprochée,
    // même réglage que le map-viewer — la surface remplit le panneau)
    const defaultPosition = {
      eye: { x: -1.05, y: -1.05, z: 0.6 },
      // Même décalage que le map-viewer : la scène remonte dans le cadre
      center: { x: 0, y: 0, z: -0.15 },
      up: { x: 0, y: 0, z: 1 },
    };
    // Sauvegarder la position par défaut
    previewCameraPositions[activeMapAddress] = defaultPosition;
    return defaultPosition;
  };

  // Handler pour sauvegarder la position de caméra quand l'utilisateur interagit avec le graphique
  const handleRelayout = useCallback((event: any) => {
    if (!activeMapAddress) return;

    // Sauvegarder la position de caméra si elle est présente dans l'événement
    if (event['scene.camera']) {
      const camera = event['scene.camera'];
      previewCameraPositions[activeMapAddress] = {
        eye: camera.eye || { x: -1.8, y: -1.8, z: 1.0 },
        center: camera.center || { x: 0, y: 0, z: 0 },
        up: camera.up || { x: 0, y: 0, z: 1 },
      };
    }
  }, [activeMapAddress]);

  // Obtenir la position de caméra actuelle
  const cameraPosition = getCameraPosition();

  // Zoom programmatique (boutons +/−) : rapproche ou éloigne l'œil du centre
  // à partir de la DERNIÈRE position utilisateur ; le bump de révision force
  // Plotly à appliquer la nouvelle caméra malgré uirevision.
  const [zoomRev, setZoomRev] = useState(0);
  const zoomCamera = (factor: number) => {
    if (!activeMapAddress) return;
    const cam = previewCameraPositions[activeMapAddress] || getCameraPosition();
    if (!cam) return;
    const c = cam.center || { x: 0, y: 0, z: 0 };
    const eye = {
      x: c.x + (cam.eye.x - c.x) * factor,
      y: c.y + (cam.eye.y - c.y) * factor,
      z: c.z + (cam.eye.z - c.z) * factor,
    };
    // Bornes : ne pas traverser la surface ni partir à l'infini
    const dist = Math.hypot(eye.x - c.x, eye.y - c.y, eye.z - c.z);
    if (dist < 0.35 || dist > 8) return;
    previewCameraPositions[activeMapAddress] = { ...cam, eye };
    setZoomRev((r) => r + 1);
  };

  // Drag handlers - utilise clampPosition comme les fenêtres de maps
  const handleDragStart = (e: React.MouseEvent) => {
    if (e.button !== 0) return; // drag uniquement au clic gauche
    e.preventDefault();
    onFocus();
    dragState.current = {
      startX: e.clientX,
      startY: e.clientY,
      originX: layout.x,
      originY: layout.y,
    };
    // NOTE: l'overlay n'est activé qu'au premier mouvement réel (sinon il
    // intercepte le mouseup et casse le click du bouton fermer du header)

    const handleMouseMove = (e: MouseEvent) => {
      if (!dragState.current) return;
      onDragActiveChange?.(true);
      const dx = e.clientX - dragState.current.startX;
      const dy = e.clientY - dragState.current.startY;
      // Utiliser clampPosition pour contraindre dans les limites du workspace
      const { x, y } = clampPosition(
        dragState.current.originX + dx,
        dragState.current.originY + dy,
        layout.width,
        layout.height
      );
      onLayoutChange({
        ...layout,
        x,
        y,
      });
    };

    const handleMouseUp = () => {
      dragState.current = null;
      onDragActiveChange?.(false);
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
  };

  // Resize handlers - contraindre aussi le resize dans les limites du workspace
  const handleResizeStart = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    onFocus();
    resizeState.current = {
      startX: e.clientX,
      startY: e.clientY,
      originW: layout.width,
      originH: layout.height,
    };

    const handleMouseMove = (e: MouseEvent) => {
      if (!resizeState.current) return;
      const dx = e.clientX - resizeState.current.startX;
      const dy = e.clientY - resizeState.current.startY;

      // Calculer les nouvelles dimensions
      let newWidth = Math.max(300, resizeState.current.originW + dx);
      let newHeight = Math.max(250, resizeState.current.originH + dy);

      // Contraindre pour ne pas dépasser le workspace
      const workspaceRect = workspaceRef.current?.getBoundingClientRect();
      if (workspaceRect) {
        const maxWidth = workspaceRect.width - layout.x;
        const maxHeight = workspaceRect.height - layout.y;
        newWidth = Math.min(newWidth, maxWidth);
        newHeight = Math.min(newHeight, maxHeight);
      }

      onLayoutChange({
        ...layout,
        width: newWidth,
        height: newHeight,
      });
    };

    const handleMouseUp = () => {
      resizeState.current = null;
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
  };

  return (
    <div
      ref={windowRef}
      className="absolute border shadow-xl shadow-black/40 rounded-md overflow-hidden"
      style={{
        left: layout.x,
        top: layout.y,
        width: layout.width,
        height: layout.height,
        zIndex,
        background: getWindowBg(),
        borderColor: getBorderColor(),
      }}
      onMouseDown={onFocus}
    >
      {/* Header */}
      <div
        className="flex items-center justify-between px-3 py-1.5 cursor-move select-none"
        style={{
          background: getWindowHeaderBg(),
          borderBottom: `1px solid ${getBorderColor()}`,
        }}
        onMouseDown={handleDragStart}
      >
        <div className="flex items-center gap-2" style={{ color: getWindowHeaderTextColor() }}>
          <Eye className="w-4 h-4" />
          <span className="text-sm font-medium">
            {t.preview.title} {activeMap ? `- ${activeMap.name}` : ''}
          </span>
        </div>
        <Button
          variant="ghost"
          size="sm"
          className={`h-6 w-6 p-0 ${getButtonHoverClass()} hover:bg-red-500/20`}
          style={{ color: getWindowHeaderTextColor() }}
          onClick={onClose}
          onMouseDown={(e) => e.stopPropagation()}
          title={t.common.close}
        >
          <X className="w-4 h-4" />
        </Button>
      </div>

      {/* Content - même style que les fenêtres de map */}
      <div
        className="flex-1 overflow-hidden bg-transparent relative"
        style={{ height: layout.height - 40 }}
      >
        {activeMap && isActiveMapEasyView ? (
          // Map en mode EasyView - Preview non disponible
          <div
            className="w-full h-full flex items-center justify-center"
            style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)' }}
          >
            <div className="text-center">
              <Eye className="w-12 h-12 mx-auto mb-3 opacity-30" />
              <p className="text-sm">{t.preview.notAvailableEasyView}</p>
            </div>
          </div>
        ) : activeMap && activeMapData && activeMapData.canShow3D ? (
          <div className="absolute inset-0 w-full h-full">
            {/* @ts-ignore */}
            <Plot
              data={activeMapData.plot3DData}
              layout={{
                paper_bgcolor: "transparent",
                plot_bgcolor: "transparent",
                scene: {
                  xaxis: {
                    title: "X",
                    backgroundcolor: "transparent",
                    gridcolor: "#374151",
                    showbackground: true,
                    color: "#9ca3af",
                    ...(previewTicks ? { tickmode: "array" as const, tickvals: previewTicks.xTickVals, ticktext: previewTicks.xTickText } : {}),
                  },
                  yaxis: {
                    title: "Y",
                    backgroundcolor: "transparent",
                    gridcolor: "#374151",
                    showbackground: true,
                    color: "#9ca3af",
                    ...(previewTicks ? { tickmode: "array" as const, tickvals: previewTicks.yTickVals, ticktext: previewTicks.yTickText } : {}),
                  },
                  zaxis: {
                    title: "Value",
                    backgroundcolor: "transparent",
                    gridcolor: "#374151",
                    showbackground: true,
                    color: "#9ca3af",
                  },
                  camera: cameraPosition || { eye: { x: -1.05, y: -1.05, z: 0.6 }, center: { x: 0, y: 0, z: -0.15 }, up: { x: 0, y: 0, z: 1 } },
                  aspectmode: "manual",
                  aspectratio: { x: 1, y: 1, z: 0.7 },
                },
                margin: { t: 30, r: 10, b: 10, l: 10 },
                autosize: true,
                // La révision de zoom force l'application de la caméra des
                // boutons +/− (uirevision fige sinon l'état UI)
                uirevision: `${activeMapAddress}-z${zoomRev}`,
              }}
              config={{
                displayModeBar: false,
                displaylogo: false,
              }}
              style={{ width: "100%", height: "100%" }}
              useResizeHandler={true}
              onRelayout={handleRelayout}
            />
            {/* Zoom +/− du graphique 3D — coin bas-gauche, comme sur l'EasyView */}
            <div
              className="absolute bottom-0 left-0 z-30 flex rounded-tr overflow-hidden"
              style={{
                border: `1px solid ${getBorderColor()}`,
                background: theme === 'light' ? '#f1f3f5' : '#1a1a1a'
              }}
            >
              <button
                onClick={() => zoomCamera(1.25)}
                className="px-3 py-1 text-[13px] leading-[14px] font-medium transition-colors"
                style={{
                  borderRight: `1px solid ${getBorderColor()}`,
                  color: theme === 'light' ? 'rgba(0, 0, 0, 0.65)' : 'rgba(255, 255, 255, 0.75)'
                }}
                onMouseEnter={(e) => e.currentTarget.style.background = theme === 'light' ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.08)'}
                onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}
                title="Zoom -"
              >
                −
              </button>
              <button
                onClick={() => zoomCamera(0.8)}
                className="px-3 py-1 text-[13px] leading-[14px] font-medium transition-colors"
                style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.65)' : 'rgba(255, 255, 255, 0.75)' }}
                onMouseEnter={(e) => e.currentTarget.style.background = theme === 'light' ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.08)'}
                onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}
                title="Zoom +"
              >
                +
              </button>
            </div>
          </div>
        ) : activeMap && activeMapData && !activeMapData.canShow3D ? (
          <div
            className="w-full h-full flex items-center justify-center"
            style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)' }}
          >
            <div className="text-center">
              <Eye className="w-12 h-12 mx-auto mb-3 opacity-30" />
              <p className="text-sm">{t.preview.map1DNotSupported}</p>
            </div>
          </div>
        ) : (
          <div
            className="w-full h-full flex items-center justify-center"
            style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)' }}
          >
            <div className="text-center">
              <Eye className="w-12 h-12 mx-auto mb-3 opacity-30" />
              <p className="text-sm">{t.preview.clickToSee}</p>
            </div>
          </div>
        )}
      </div>

      {/* Resize handle */}
      <div
        className="absolute bottom-0 right-0 w-4 h-4 cursor-se-resize"
        style={{
          background: `linear-gradient(135deg, transparent 50%, ${getBorderColor()} 50%)`,
        }}
        onMouseDown={handleResizeStart}
      />
    </div>
  );
}

// Les patterns EDC15 du détecteur nomment les maps "Start of injection (SOI)…" ;
// le sigle est retiré partout à l'affichage (projets existants inclus) en
// normalisant les résultats de détection à leur entrée dans projectData.
function stripSoiTag<T extends { maps?: { name?: string }[] } | null | undefined>(detectionResults: T): T {
  if (!detectionResults || !Array.isArray(detectionResults.maps)) return detectionResults;
  const maps = detectionResults.maps.map((m) =>
    typeof m?.name === "string" && m.name.includes(" (SOI)")
      ? { ...m, name: m.name.replace(" (SOI)", "") }
      : m
  );
  return { ...detectionResults, maps };
}

function EditorPageContent() {
  const router = useRouter();
  // Project name comes from the query string (/editor?project=...) — static
  // export cannot use dynamic route segments. Read once per mount.
  const [projectName] = useState(() => {
    if (typeof window === "undefined") return "";
    return new URLSearchParams(window.location.search).get("project") || "";
  });
  const { toast } = useToast();
  const { theme } = useTheme();
  const { settings, platform } = useSettings();
  const { t } = useI18n();

  // Theme-based color helpers (glassmorphism tokens — OLED keeps true black)
  // Fond d'écran de l'éditeur — mêmes options et règles que le dashboard :
  // blanc réservé au thème clair, automatique = look natif du thème
  // (défaut → halos de l'éditeur, clair → blanc, OLED → noir)
  // Image personnalisée (lib/custom-wallpaper) — suivie en direct, utilisable
  // avec TOUS les thèmes ; sans image enregistrée, repli automatique.
  const [customEditorWallpaper, setCustomEditorWallpaper] = useState<string | null>(null);
  useEffect(() => {
    setCustomEditorWallpaper(getCustomWallpaper("editor"));
    return subscribeCustomWallpaper("editor", setCustomEditorWallpaper);
  }, []);

  const storedEditorWallpaper = settings.editorWallpaper;
  let editorWallpaper: string =
    storedEditorWallpaper === "custom" && customEditorWallpaper
      ? "custom"
      : theme === "light"
        ? (storedEditorWallpaper === "lines-light" ? "lines-light" : "white")
        : storedEditorWallpaper && storedEditorWallpaper !== "auto"
          ? storedEditorWallpaper
          : theme === "oled"
            ? "black"
            : "editor";
  if (editorWallpaper !== "custom") {
    if (theme !== "light" && (editorWallpaper === "white" || editorWallpaper === "lines-light")) {
      editorWallpaper = theme === "oled" ? "black" : "editor";
    }
    if (!["lines", "lines-light", "editor", "white", "black"].includes(editorWallpaper)) {
      editorWallpaper = theme === "oled" ? "black" : "editor";
    }
  }

  const getBackgroundColor = () => {
    if (editorWallpaper === "black") return "#000000";
    if (editorWallpaper === "white" || editorWallpaper === "lines-light") return "#eef0f4";
    switch (theme) {
      case 'light':
        return '#eef0f4';
      case 'oled':
        return '#000000';
      default:
        return '#0a0b0f';
    }
  };

  // Un fond d'écran explicitement choisi en OLED doit rester visible :
  // les surfaces passent en translucide (sinon noir opaque = fond invisible)
  const oledShowsWallpaper =
    theme === 'oled' &&
    (editorWallpaper === 'lines' || editorWallpaper === 'editor' || editorWallpaper === 'custom');

  // Translucent sidebar surface; the ambient halos glow through it.
  // OLED stays near-opaque dark to preserve the true-black look.
  const getSidebarBg = () => {
    switch (theme) {
      case 'light':
        return 'linear-gradient(180deg, rgba(255,255,255,0.72), rgba(244,246,250,0.70))';
      case 'oled':
        return oledShowsWallpaper
          ? 'linear-gradient(180deg, rgba(10,10,12,0.72), rgba(0,0,0,0.72))'
          : 'linear-gradient(180deg, rgba(14,14,16,0.94), rgba(0,0,0,0.96))';
      default:
        return 'linear-gradient(180deg, rgba(16,18,26,0.72), rgba(10,11,15,0.72))';
    }
  };

  // backdrop-filter strength per theme (light blur on OLED: nothing colored behind)
  const getGlassBlur = () => {
    return theme === 'oled' ? 'blur(6px)' : 'blur(18px) saturate(140%)';
  };

  // Shared glass surface for buttons/selects inside the sidebar
  const getGlassSurface = () => {
    switch (theme) {
      case 'light':
        return 'rgba(255,255,255,0.6)';
      case 'oled':
        return 'rgba(20,20,23,0.85)';
      default:
        return 'rgba(22,25,34,0.55)';
    }
  };

  const getToolbarBg = () => {
    switch (theme) {
      case 'light':
        return '#ffffff';
      case 'oled':
        return '#000000';
      default:
        return '#000000';
    }
  };

  // Translucent workspace so the ambient halos give it depth (OLED opaque black)
  const getWorkspaceBg = () => {
    switch (theme) {
      case 'light':
        return 'rgba(233,236,241,0.55)';
      case 'oled':
        return oledShowsWallpaper ? 'rgba(0,0,0,0.45)' : '#000000';
      default:
        return 'rgba(18,21,29,0.45)';
    }
  };

  const getTextColor = () => {
    return theme === 'light' ? '#000000' : '#ffffff';
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

  // Couleurs pour les fenêtres (maps et hexdump) — surfaces verre SANS
  // backdrop-filter : les menus contextuels/panneaux fixed des maps seraient
  // décalés (containing block) et le scroll virtualisé du hexdump paierait
  // le coût du blur. La teinte translucide suffit au-dessus des halos.
  const getWindowBg = () => {
    switch (theme) {
      case 'light':
        return 'rgba(255, 255, 255, 0.72)';
      case 'oled':
        return 'rgba(10, 10, 12, 0.88)';
      default:
        return 'rgba(22, 25, 34, 0.72)';
    }
  };

  const getWindowHeaderBg = () => {
    switch (theme) {
      case 'light':
        return 'rgba(255, 255, 255, 0.85)';
      case 'oled':
        return '#0f0f11';
      default:
        return 'rgba(28, 32, 44, 0.85)';
    }
  };

  const getWindowHeaderTextColor = () => {
    return theme === 'light' ? '#000000' : '#ffffff';
  };

  const getButtonHoverClass = () => {
    return theme === 'light' ? 'hover:bg-black/10' : 'hover:bg-white/10';
  };

  const [projectData, setProjectData] = useState<ProjectData | null>(null);
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set());
  // Tri des maps À L'INTÉRIEUR des dossiers : par adresse (défaut), par nom
  // A→Z ou par nom Z→A — le bouton cycle entre les trois. L'ordre des
  // dossiers ne change pas. Persisté PAR PROJET : un projet sans choix
  // enregistré s'ouvre toujours par adresse ; le choix fait sur un projet
  // est retrouvé en y revenant. Transmis à l'export mappack pour que le
  // fichier reprenne le même ordre.
  const [mapSortMode, setMapSortMode] = useState<"address" | "name" | "name-desc">("address");
  useEffect(() => {
    if (!projectName) return;
    const stored = localStorage.getItem(`mapSortMode:${projectName}`);
    setMapSortMode(stored === "name" || stored === "name-desc" ? stored : "address");
  }, [projectName]);
  const toggleMapSortMode = () => {
    setMapSortMode((prev) => {
      const next = prev === "address" ? "name" : prev === "name" ? "name-desc" : "address";
      if (projectName) localStorage.setItem(`mapSortMode:${projectName}`, next);
      return next;
    });
  };
  const [hasAnimatedMappack, setHasAnimatedMappack] = useState(false);
  const [versions, setVersions] = useState<VersionDto[]>([]);
  const [currentVersionId, setCurrentVersionId] = useState<string | null>(null);
  const [showVersionDropdown, setShowVersionDropdown] = useState(false);
  const [tooltipVisible, setTooltipVisible] = useState<string | null>(null);
  const tooltipTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const showTooltip = (id: string) => {
    tooltipTimerRef.current = setTimeout(() => setTooltipVisible(id), 500);
  };
  const hideTooltip = () => {
    if (tooltipTimerRef.current) clearTimeout(tooltipTimerRef.current);
    setTooltipVisible(null);
  };
  const [loadingVersions, setLoadingVersions] = useState(false);
  const [loadingAction, setLoadingAction] = useState<"save" | "import" | "export" | "rename" | "delete" | null>(null);
  
  // Fenêtres ouvertes (maps) et états associés
  const [openMaps, setOpenMaps] = useState<MapData[]>([]);
  const [mapViewModes, setMapViewModes] = useState<Map<number, "text" | "2d" | "3d">>(new Map());
  const [mapEasyViewStatus, setMapEasyViewStatus] = useState<Map<number, boolean>>(new Map());

  // Store global pour les modifications de toutes les maps (persist même après fermeture des fenêtres)
  // Clé: mapAddress, Valeur: Record<cellKey, newValue>
  const [allMapModifications, setAllMapModifications] = useState<Map<number, Record<string, number>>>(new Map());

  // Persisted axis-label edits per map, so closing/reopening a map keeps the
  // user's edited labels instead of falling back to the values read from the
  // file. Stored as state (not ref) so that re-renders propagate the
  // persisted labels back into MapViewer via `initialXAxisLabels` /
  // `initialYAxisLabels`.
  const [mapAxisLabels, setMapAxisLabels] = useState<Map<number, { x?: string[]; y?: string[] }>>(new Map());
  const handleAxisLabelsChange = useCallback((mapAddress: number, axes: { x?: string[]; y?: string[] }) => {
    setMapAxisLabels(prev => {
      const existing = prev.get(mapAddress) || {};
      const merged = {
        x: axes.x !== undefined ? axes.x : existing.x,
        y: axes.y !== undefined ? axes.y : existing.y,
      };
      if (!merged.x && !merged.y) {
        if (!prev.has(mapAddress)) return prev;
        const next = new Map(prev);
        next.delete(mapAddress);
        return next;
      }
      const prevEntry = prev.get(mapAddress);
      const xSame = JSON.stringify(prevEntry?.x) === JSON.stringify(merged.x);
      const ySame = JSON.stringify(prevEntry?.y) === JSON.stringify(merged.y);
      if (xSame && ySame) return prev;
      const next = new Map(prev);
      next.set(mapAddress, merged);
      // Axis label edits need to be persisted just like cell edits.
      // Mark as dirty so the next Save call picks them up.
      if (!isLoadingVersionRef.current) {
        setTimeout(() => setHasUnsavedChanges(true), 0);
      }
      return next;
    });
  }, []);

  // Store pour les modifications binaires directes (DTCs, etc.)
  // Clé: address, Valeur: { oldValue, newValue }
  const [binaryModifications, setBinaryModifications] = useState<Map<number, { oldValue: number; newValue: number }>>(new Map());

  // Tracks the display-vs-file row/col flip state for each open map. MapViewer
  // reorders rows/cols for human-friendly display (e.g. RPM descending), so
  // when we re-encode display-coord edits into file bytes we need to know
  // whether to mirror the row/col index. Kept in a ref so updates don't re-render.
  const mapAxesFlipRef = useRef<Map<number, { rowsReversed: boolean; colsReversed: boolean }>>(new Map());
  const handleAxesFlipChange = useCallback((mapAddress: number, flip: { rowsReversed: boolean; colsReversed: boolean }) => {
    const existing = mapAxesFlipRef.current.get(mapAddress);
    if (existing && existing.rowsReversed === flip.rowsReversed && existing.colsReversed === flip.colsReversed) return;
    mapAxesFlipRef.current.set(mapAddress, flip);
  }, []);

  // Store original file data (never modified) for version switching
  const originalFileDataRef = useRef<number[] | null>(null);

  // Store original detected maps (from database, never modified in memory)
  // Used to check if solutions like Launch Control were already active in the original file
  const originalDetectedMapsRef = useRef<Array<{ name?: string; address?: number }> | null>(null);

  // Store imported file data for each version (versionId -> file data)
  // This allows switching between versions with different binary data
  const versionFileDataRef = useRef<Map<string, number[]>>(new Map());

  const [hexdumpCollapsed, setHexdumpCollapsed] = useState(false);
  const [hexdumpLayout, setHexdumpLayout] = useState<{ x: number; y: number; width: number; height: number }>({
    x: 0,
    y: 0,
    // Largeur mode 8b (voir HEXDUMP_WINDOW_WIDTH) : contenu + minimap 32px.
    // Resynchronisée par effet à chaque bascule 8b/16b.
    width: 452,
    height: 700,
  });
  const [hexdumpMovedFromOrigin, setHexdumpMovedFromOrigin] = useState(false);
  const [hexdumpZIndex, setHexdumpZIndex] = useState(10);
  const [mapLayouts, setMapLayouts] = useState<Map<number, { x: number; y: number; width: number; height: number }>>(new Map());
  
  // Hexdump format state
  const [hexdumpSize, setHexdumpSize] = useState<"8b" | "16b">("8b");
  // Ordre des octets de l'hexdump 16 bits : par défaut celui de l'ECU (HiLo
  // = big-endian EDC16/MJD, LoHi = EDC15), basculable dans la toolbar.
  const [hexdumpByteOrder, setHexdumpByteOrder] = useState<"hilo" | "lohi">("lohi");
  useEffect(() => {
    if (projectData?.ecu_type) {
      setHexdumpByteOrder(isBigEndianEcu(projectData.ecu_type) ? "hilo" : "lohi");
    }
  }, [projectData?.ecu_type]);
  const [hexdumpFormat, setHexdumpFormat] = useState<"hex" | "dec">("hex");

  // Largeur de fenêtre hexdump calée sur son contenu réel (adresse + valeurs +
  // ASCII + minimap 64px). Les cellules 16 bits (2.4rem) sont ~70px plus
  // larges que les 8 bits (1.85rem) : la fenêtre doit suivre le mode, et sa
  // largeur MINIMALE = le contenu complet — l'ASCII ne peut jamais être rogné.
  // 8b : 8 octets/ligne (ASCII 8 caractères) ; 16b : 8 mots/ligne (ASCII 16).
  // Minimap réduite à 24px (-25 %) : largeurs fenêtre ajustées de -8px
  const HEXDUMP_WINDOW_WIDTH: Record<"8b" | "16b", number> = { "8b": 444, "16b": 568 };

  // Recale la largeur à chaque bascule 8b/16b (la hauteur choisie est gardée)
  useEffect(() => {
    setHexdumpLayout(prev =>
      prev.width === HEXDUMP_WINDOW_WIDTH[hexdumpSize]
        ? prev
        : { ...prev, width: HEXDUMP_WINDOW_WIDTH[hexdumpSize] }
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hexdumpSize]);
  const [easyViewMode, setEasyViewMode] = useState(settings.easyViewDefault);

  // Modify value for toolbar (shared with MapViewer for +/- keyboard shortcuts)
  const [modifyValue, setModifyValue] = useState<string>('1');

  // Command to send to active MapViewer when Zap button is clicked
  const [modifyCommand, setModifyCommand] = useState<{ operation: 'add' | 'fill'; value: number; timestamp: number } | null>(null);

  // Sync easyViewMode with settings when settings change (for newly loaded settings)
  useEffect(() => {
    setEasyViewMode(settings.easyViewDefault);
  }, [settings.easyViewDefault]);

  // Project Info Modal state
  const [showProjectInfoModal, setShowProjectInfoModal] = useState(false);
  const [isClosingProjectInfoModal, setIsClosingProjectInfoModal] = useState(false);

  // Zoom de l'éditeur (webview native, persisté) — boutons +/− de la toolbar
  const [editorZoom, setEditorZoom] = useState<number>(() => {
    if (typeof window === "undefined") return 100;
    const saved = parseInt(localStorage.getItem("zedsuite-editor-zoom") || "100", 10);
    return Number.isFinite(saved) && saved >= 60 && saved <= 150 ? saved : 100;
  });
  useEffect(() => {
    setAppZoom(editorZoom / 100);
    localStorage.setItem("zedsuite-editor-zoom", String(editorZoom));
    return () => {
      setAppZoom(1);
    };
  }, [editorZoom]);
  const changeEditorZoom = (delta: number) =>
    setEditorZoom((z) => Math.min(150, Math.max(60, z + delta)));


  // Map Properties Modal state
  const [showMapPropertiesModal, setShowMapPropertiesModal] = useState(false);
  const [isClosingMapPropertiesModal, setIsClosingMapPropertiesModal] = useState(false);
  const [mapPropertiesTarget, setMapPropertiesTarget] = useState<MapData | null>(null);
  // Store pour les settings d'affichage de chaque map (clé: mapAddress)
  const [mapDisplaySettingsStore, setMapDisplaySettingsStore] = useState<Map<number, MapDisplaySettings>>(new Map());

  // Preview window state
  const [showPreviewWindow, setShowPreviewWindow] = useState(false);
  const [previewWindowKey, setPreviewWindowKey] = useState(0); // Key pour forcer le reset du composant
  const [previewLayout, setPreviewLayout] = useState({ x: 100, y: 100, width: 500, height: 400 });
  const [previewZIndex, setPreviewZIndex] = useState(100);

  // Store pour les données 3D de chaque map (pour Preview window)
  const mapPlot3DDataRef = useRef<Map<number, {
    plot3DData: any[];
    xAxisLabels: string[];
    yAxisLabels: string[];
    canShow3D: boolean;
  }>>(new Map());

  // State pour forcer le re-render du Preview quand les données changent
  const [previewDataVersion, setPreviewDataVersion] = useState(0);

  // Curseur global - map/hexdump active et infos de sélection
  const [activeMapAddress, setActiveMapAddress] = useState<number | null>(null);
  const [isHexdumpActive, setIsHexdumpActive] = useState(true); // Par défaut hexdump est visible
  const [hexdumpScrollToAddress, setHexdumpScrollToAddress] = useState<number | null>(null); // Pour scroll manuel vers une map
  const [hexdumpScrollKey, setHexdumpScrollKey] = useState(0); // Key to force scroll even if same address
  const [globalCursorInfo, setGlobalCursorInfo] = useState<{
    mapName: string;
    mapAddress: number;
    dimensions: string;
    selectedCount: number;
    selectedCells: Array<{ row: number; col: number; address: number; value: number }>;
  } | null>(null);
  // Overlay plein écran pendant le drag/resize d'une fenêtre (map/preview/hexdump) :
  // garde le curseur cohérent et empêche la souris d'interagir avec les
  // graphiques (rotation 3D) si elle passe au-dessus pendant le déplacement
  const [isWindowDragActive, setIsWindowDragActive] = useState(false);
  const [overlayCursor, setOverlayCursor] = useState<'move' | 'se-resize'>('move');

  // Settings menu state
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isClosingSettings, setIsClosingSettings] = useState(false);

  // DTC modal state
  const [isDTCOpen, setIsDTCOpen] = useState(false);
  const [isDTCClosing, setIsDTCClosing] = useState(false);
  const [dtcRefreshKey, setDtcRefreshKey] = useState(0);
  // DTC notification state
  const [dtcNotification, setDtcNotification] = useState<{ count: number; visible: boolean; fading: boolean }>({ count: 0, visible: false, fading: false });

  // Solutions modal state
  const [isSolutionsOpen, setIsSolutionsOpen] = useState(false);
  const [isSolutionsClosing, setIsSolutionsClosing] = useState(false);
  // Solutions appliquées dans ce projet : { id: nom de version }
  const [usedSolutions, setUsedSolutions] = useState<Record<string, string>>({});
  const [solutionNotification, setSolutionNotification] = useState<{ count: number; visible: boolean; fading: boolean }>({ count: 0, visible: false, fading: false });

  // Mappack lock state
  const [mappackUnlocked, setMappackUnlocked] = useState(false);
  // True when this project's mappack has already been exported (loaded from
  // the server, kept in sync after an export) — the export confirmation
  // modal must not open again in that case.
  const [mappackExported, setMappackExported] = useState(false);
  // Export button hint: when the mappack becomes unlocked the button shows
  // its "Export" label for 2 seconds, then collapses to the icon only
  // (the label re-expands on hover).
  const [mappackExportHint, setMappackExportHint] = useState(false);
  useEffect(() => {
    if (!mappackUnlocked) return;
    setMappackExportHint(true);
    const timer = setTimeout(() => setMappackExportHint(false), 2000);
    return () => clearTimeout(timer);
  }, [mappackUnlocked]);
  const [mappackUnlocking, setMappackUnlocking] = useState(false);
  const [mappackIsPro, setMappackIsPro] = useState(true);
  // Per-ECU mappack settings from the server (admin-configurable):
  // whether export is enabled for this ECU and its price in blue coins.
  const [mappackExportEnabled, setMappackExportEnabled] = useState(true);
  const [mappackPrice, setMappackPrice] = useState(40);
  const [mappackJustUnlocked, setMappackJustUnlocked] = useState(false);
  const [showUnlockConfirm, setShowUnlockConfirm] = useState(false);
  const [maxVersionsPerProject, setMaxVersionsPerProject] = useState(10);

  // Save/Version notification state
  const [saveNotification, setSaveNotification] = useState<{ type: 'save' | 'version' | 'deleted'; message?: string; visible: boolean; fading: boolean }>({ type: 'save', visible: false, fading: false });

  const showInlineNotification = (message: string) => {
    setSaveNotification({ type: 'save', message, visible: true, fading: false });
    setTimeout(() => {
      setSaveNotification(prev => ({ ...prev, fading: true }));
    }, 2500);
    setTimeout(() => {
      setSaveNotification(prev => ({ ...prev, visible: false, fading: false, message: undefined }));
    }, 3000);
  };

  const [versionLimitNotification, setVersionLimitNotification] = useState<{ visible: boolean; fading: boolean }>({ visible: false, fading: false });

  // Compare modal state
  const [isCompareOpen, setIsCompareOpen] = useState(false);

  // Checksum modal state
  const [isChecksumModalOpen, setIsChecksumModalOpen] = useState(false);
  const [isChecksumModalClosing, setIsChecksumModalClosing] = useState(false);
  const [isChecksumCalculating, setIsChecksumCalculating] = useState(false);
  const [isChecksumComplete, setIsChecksumComplete] = useState(false);
  const [checksumCorrectedData, setChecksumCorrectedData] = useState<number[] | null>(null);
  // État du checksum des données à exporter, vérifié à l'ouverture de la
  // fenêtre d'export : ok → export direct, bad → correction proposée
  const [exportChecksumStatus, setExportChecksumStatus] = useState<'checking' | 'ok' | 'bad'>('checking');

  // ── État du mappack : rapport de complétude EDC16 (expected_maps) ──
  // Le détecteur liste les familles de maps qui existent TOUJOURS sur cette
  // famille d'ECU ; le badge à côté de « Mappack » affiche le % de règles
  // satisfaites, et la fenêtre détaille ce qui manque.
  const expectedMaps = projectData?.detectionResults?.expected_maps ?? null;
  const missingExpected = useMemo(
    () => (expectedMaps || []).filter((e) => e.found < e.expected),
    [expectedMaps]
  );
  const mappackConfidence = useMemo(() => {
    if (!expectedMaps || expectedMaps.length === 0) return null;
    const ok = expectedMaps.length - missingExpected.length;
    return Math.round((ok / expectedMaps.length) * 100);
  }, [expectedMaps, missingExpected]);
  const [showMappackHealth, setShowMappackHealth] = useState(false);
  // Ouverture AUTOMATIQUE au chargement du projet quand il manque des maps
  // (une seule fois par ouverture du projet)
  const mappackHealthAutoShownRef = useRef(false);
  useEffect(() => {
    if (mappackHealthAutoShownRef.current || !expectedMaps) return;
    mappackHealthAutoShownRef.current = true;
    if (missingExpected.length > 0) setShowMappackHealth(true);
  }, [expectedMaps, missingExpected]);

  // Search modal state
  const [showSearchModal, setShowSearchModal] = useState(false);
  const [searchConfig, setSearchConfig] = useState<{
    valueType: 'dec' | 'hex';
    dataSize: '8b' | '16b';
    value: string;
    fromAddress: string;
    toAddress: string;
    searchInOriginal: boolean;
  }>({
    valueType: 'dec',
    dataSize: '8b',
    value: '',
    fromAddress: '',
    toAddress: '',
    searchInOriginal: false,
  });
  const [searchResults, setSearchResults] = useState<number[]>([]); // Array of addresses
  const [currentSearchIndex, setCurrentSearchIndex] = useState<number>(-1);
  const [hasSearched, setHasSearched] = useState(false); // Track if a search has been performed
  const [searchModalPosition, setSearchModalPosition] = useState<{ x: number; y: number } | null>(null);
  const searchModalRef = useRef<HTMLDivElement>(null);
  const searchModalDragState = useRef<{
    startX: number;
    startY: number;
    originX: number;
    originY: number;
  } | null>(null);

  // Map tree context menu state (right-click on map in tree view)
  const [mapTreeContextMenu, setMapTreeContextMenu] = useState<{
    x: number;
    y: number;
    map: MapData;
  } | null>(null);
  const mapTreeContextMenuRef = useRef<HTMLDivElement>(null);

  // Confirmation dialog for unsaved changes
  const [showUnsavedChangesDialog, setShowUnsavedChangesDialog] = useState(false);
  const [pendingVersionSwitch, setPendingVersionSwitch] = useState<string | null>(null);
  const [pendingNavigation, setPendingNavigation] = useState<string | null>(null);
  // Confirmation dialog for deleting the current version. Holds the version's
  // display name (null = closed).
  const [deleteVersionName, setDeleteVersionName] = useState<string | null>(null);
  // Rename dialog for the current version. Holds the current name as the
  // input's initial value (null = closed).
  const [renameVersionValue, setRenameVersionValue] = useState<string | null>(null);
  // Generic themed text prompt (promise-based) used for "new version name"
  // across the several create-version flows. null = closed.
  const [textPrompt, setTextPrompt] = useState<{
    title: string;
    description?: string;
    initialValue: string;
    resolve: (value: string | null) => void;
  } | null>(null);
  // Returns the entered text (trimmed by the caller) or null if cancelled.
  const promptForText = (title: string, initialValue: string, description?: string) =>
    new Promise<string | null>((resolve) => {
      setTextPrompt({ title, description, initialValue, resolve });
    });

  // Nom par défaut des nouvelles versions : v1, v2, v3… (le plus grand
  // vN existant + 1, pour ne jamais proposer un nom déjà pris)
  const nextVersionName = () => {
    const maxN = versions.reduce((max, v) => {
      const m = /^v(\d+)$/i.exec((v.name || "").trim());
      return m ? Math.max(max, parseInt(m[1], 10)) : max;
    }, 0);
    return `v${maxN + 1}`;
  };

  // Track if user has made changes since loading the version (dirty flag)
  const [hasUnsavedChanges, setHasUnsavedChanges] = useState(false);

  // État du checksum de la version courante — surveillé en continu sur les
  // octets courants (édits inclus), affiché sous le sélecteur de version
  const [checksumStatus, setChecksumStatus] = useState<'checking' | 'ok' | 'bad' | 'unsupported'>('checking');
  const [showChecksumSaveConfirm, setShowChecksumSaveConfirm] = useState(false);

  // Note: Les maps modifiées sont trackées via allMapModifications qui est déjà version-specific

  // Sidebar resize state
  const [sidebarWidth, setSidebarWidth] = useState(335);
  const isResizing = useRef(false);
  const sidebarRef = useRef<HTMLDivElement>(null);

  // Largeur minimale de la fenêtre = barre d'outils + barre latérale, le
  // tout ramené à l'échelle du zoom. Élargir la barre latérale agrandit donc
  // la fenêtre au besoin, au lieu de pousser la barre d'outils hors du cadre.
  // Ne dépend que de valeurs pilotées par l'utilisateur (zoom, largeur de la
  // barre) : le redimensionnement de la fenêtre ne les modifie pas, donc pas
  // de boucle.
  useEffect(() => {
    setAppMinWidth(TOOLBAR_MIN_CSS_WIDTH + sidebarWidth, editorZoom / 100);
  }, [sidebarWidth, editorZoom]);

  // Sync sidebar width to CSS variable for global Toaster positioning
  useEffect(() => {
    document.documentElement.style.setProperty('--sidebar-width', `${sidebarWidth}px`);
    return () => {
      document.documentElement.style.removeProperty('--sidebar-width');
    };
  }, [sidebarWidth]);
  const workspaceRef = useRef<HTMLDivElement>(null);
  const mapRefs = useRef<Record<number, HTMLDivElement | null>>({});
  const dragState = useRef<{
    address: number;
    startX: number;
    startY: number;
    originX: number;
    originY: number;
  } | null>(null);

  // Paramètres de dimension pour la vue texte (tailles fixes pour cohérence)
  const TEXT_VIEW_SIZING = {
    stickyWidth: 49,        // Largeur de la colonne Y axis (44px + padding)
    colWidth: 58,           // Largeur des cellules (56px + border)
    headerHeight: 22,       // Hauteur du header (20px + border)
    rowHeight: 22,          // Hauteur des lignes (20px + border)
    chromeHeight: 51,       // Titre + onglets avec marge réduite
    paddingWidth: 10,       // Petite marge à droite
    paddingHeight: 6,       // Espace minimal en bas pour les boutons
  } as const;
  const getTextPaddingWidth = (colCount: number) => (colCount >= 15 ? 0 : TEXT_VIEW_SIZING.paddingWidth);
  const hexdumpRef = useRef<HTMLDivElement | null>(null);
  const handleSaveRef = useRef<(() => Promise<void>) | null>(null);
  const isLoadingVersionRef = useRef(false); // Flag to prevent marking changes as unsaved during version load
  const hexdumpDragState = useRef<{
    startX: number;
    startY: number;
    originX: number;
    originY: number;
  } | null>(null);
  
  // Handle sidebar resize - use refs to avoid dependency issues
  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!isResizing.current) return;
    const newWidth = e.clientX;
    if (newWidth >= 335 && newWidth <= 500) {
      setSidebarWidth(newWidth);
    }
  }, []);

  const handleMouseUp = useCallback(() => {
    isResizing.current = false;
  }, []);

  // Cleanup effect for event listeners
  useEffect(() => {
    const cleanup = () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };

    // Cleanup on unmount or when handlers change
    return cleanup;
  }, [handleMouseMove, handleMouseUp]);

  // Reset search results when hexdump display format changes
  // because the previous results were for a different format
  useEffect(() => {
    setSearchResults([]);
    setCurrentSearchIndex(-1);
    setHasSearched(false);
  }, [hexdumpSize, hexdumpFormat]);

  // Close map tree context menu when clicking outside
  useEffect(() => {
    if (!mapTreeContextMenu) return;

    const handleClickOutside = (e: MouseEvent) => {
      if (mapTreeContextMenuRef.current && !mapTreeContextMenuRef.current.contains(e.target as Node)) {
        setMapTreeContextMenu(null);
      }
    };

    const timeoutId = setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
    }, 0);

    return () => {
      clearTimeout(timeoutId);
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [mapTreeContextMenu]);

  const handleMouseDown = (e: React.MouseEvent) => {
    isResizing.current = true;
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
    e.preventDefault();
  };

  const handleImport = async () => {
    if (!mappackUnlocked) {
      toast({ title: t.mappack?.unlockRequired || "Mappack Locked", description: t.mappack?.unlockRequiredImport || "You must unlock the mappack before importing a file.", variant: "destructive" });
      return;
    }

    // Check daily upload/import limit (shared limit)
    try {
      const res = await fetch("/api/versioning/track-action", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ action: "import" }),
      });
      const data = await res.json();
      if (!data.allowed) {
        toast({ title: t.mappack?.dailyUploadLimit || "Daily Upload Limit Reached", description: t.mappack?.dailyUploadLimitDescription || "You have reached the daily upload/import limit. Try again in 24h.", variant: "destructive" });
        return;
      }
    } catch {
      // If limit check fails, allow the import
    }

    // Créer un input file temporaire
    const input = document.createElement('input');
    input.type = 'file';
    // Pas d'attribut accept : la boîte de dialogue s'ouvre sur "Tous les fichiers",
    // comme l'import depuis le dashboard (les binaires ECU n'ont souvent pas d'extension)

    input.onchange = async (e: Event) => {
      const target = e.target as HTMLInputElement;
      const file = target.files?.[0];
      if (!file) return;

      // Même plafond de taille que l'upload du dashboard
      if (file.size > platform.maxFileSizeMB * 1024 * 1024) {
        toast({
          title: t.errors.fileTooLarge,
          description: t.errors.fileTooLargeDescription,
          variant: "destructive",
        });
        return;
      }

      setLoadingAction("import");

      try {
        // Lire le fichier
        const arrayBuffer = await file.arrayBuffer();
        const uint8Array = new Uint8Array(arrayBuffer);
        const fileData = Array.from(uint8Array);

        // Garde anti-fichier étranger : un binaire qui ne provient pas du même
        // calculateur que l'origine du projet casserait l'affichage des maps
        // (adresses décalées) — on le refuse avant de créer la version.
        const original = originalFileDataRef.current;
        if (original && original.length > 0) {
          const rejectImport = (description: string) => {
            toast({ title: t.errors.importWrongFileTitle, description, variant: "destructive" });
            setLoadingAction(null);
          };

          // Une lecture du même ECU a toujours exactement la même taille
          if (fileData.length !== original.length) {
            rejectImport(`${t.errors.importSizeMismatch} (${fileData.length.toLocaleString()} ≠ ${original.length.toLocaleString()} bytes)`);
            return;
          }

          // Identité : type ECU + numéros HW/SW du fichier importé vs projet
          let identityVerified = false;
          try {
            const ident = await identifyEcu(bytesToBase64(uint8Array), file.name);
            if (
              projectData?.ecu_type && projectData.ecu_type !== "Unknown" &&
              ident.ecu_type && ident.ecu_type !== "Unknown" &&
              ident.ecu_type !== projectData.ecu_type
            ) {
              rejectImport(`${t.errors.importEcuMismatch} (${ident.ecu_type} ≠ ${projectData.ecu_type})`);
              return;
            }
            if (projectData?.hardware_version && ident.hardware_version &&
                ident.hardware_version !== projectData.hardware_version) {
              rejectImport(`${t.errors.importIdentityMismatch} (HW ${ident.hardware_version} ≠ ${projectData.hardware_version})`);
              return;
            }
            if (projectData?.software_version && ident.software_version &&
                ident.software_version !== projectData.software_version) {
              rejectImport(`${t.errors.importIdentityMismatch} (SW ${ident.software_version} ≠ ${projectData.software_version})`);
              return;
            }
            identityVerified = !!(
              (projectData?.hardware_version && ident.hardware_version) ||
              (projectData?.software_version && ident.software_version)
            );
          } catch {
            // Identification impossible → on retombe sur la similarité brute
          }

          // Sans preuve par HW/SW : similarité brute avec l'origine (échantillon
          // 1 octet sur 16). Un fichier tuné du même ECU reste >99 % identique ;
          // un fichier étranger passe largement sous les 90 %.
          if (!identityVerified) {
            let same = 0;
            let total = 0;
            for (let i = 0; i < fileData.length; i += 16) {
              total++;
              if (fileData[i] === original[i]) same++;
            }
            if (total > 0 && same / total < 0.9) {
              rejectImport(t.errors.importTooDifferent);
              return;
            }
          }
        }

        // Check version limit
        if (versions.length >= maxVersionsPerProject) {
          setVersionLimitNotification({ visible: true, fading: false });
          setTimeout(() => {
            setVersionLimitNotification(prev => ({ ...prev, fading: true }));
            setTimeout(() => {
              setVersionLimitNotification({ visible: false, fading: false });
            }, 500);
          }, 3000);
          setLoadingAction(null);
          return;
        }

        // Demander le nom de la nouvelle version
        const versionName = await promptForText(t.versionDialogs.newVersionTitle, file.name.replace('.bin', ''), t.versionDialogs.newVersionName);
        if (!versionName?.trim()) {
          setLoadingAction(null);
          return;
        }

        if (!projectData?.fileId) {
          setLoadingAction(null);
          return;
        }

        // Créer une nouvelle version pour ce fichier importé
        // Note: Dans une implémentation complète, on pourrait sauvegarder le fichier binaire
        // et l'associer à la version. Pour l'instant, on crée juste une version vide.
        const response = await axios.post("/api/versioning/versions", {
          fileId: projectData.fileId,
          name: versionName.trim(),
          baseVersionId: currentVersionId,
          setCurrent: true,
        });

        const newVersionId = response.data.version?.id;

        // Store the imported file data for this version
        // This allows switching back to this version and seeing the correct data
        if (newVersionId) {
          versionFileDataRef.current.set(newVersionId, [...fileData]);
        }

        // CRITICAL: Update the file data with the imported file's data
        // This allows seeing the differences between original and imported version
        setProjectData((prevData) => {
          if (!prevData) return prevData;
          return {
            ...prevData,
            file_data: fileData,
          };
        });

        // IMPORTANT: Do NOT update originalFileDataRef here!
        // originalFileDataRef should always contain the ORIGINAL file data (first loaded)
        // This way, when switching to "Ori" version, we still have the original data
        // The imported file data is stored in projectData.file_data for this version

        // Clear the map data cache to force re-reading from new file data
        // Import the clearMapDataCache function from map-viewer
        const { clearMapDataCache } = await import("@/components/map-viewer");
        clearMapDataCache();

        // Detect modifications between imported file and original file
        // This will mark maps as modified (red) in the sidebar
        const originalData = originalFileDataRef.current;
        const detectedModifications = new Map<number, Record<string, number>>();

        if (originalData && projectData?.detectionResults?.maps) {

          for (const map of projectData.detectionResults.maps) {
            const mapAddress = map.address;
            const mapSize = map.size;

            if (mapAddress >= 0 && mapAddress + mapSize <= fileData.length && mapAddress + mapSize <= originalData.length) {
              // Compare map data between original and imported file
              let hasChanges = false;
              const modifications: Record<string, number> = {};

              for (let i = 0; i < mapSize; i++) {
                const originalByte = originalData[mapAddress + i];
                const importedByte = fileData[mapAddress + i];

                if (originalByte !== importedByte) {
                  hasChanges = true;
                  // Store the modification (offset within map -> new value)
                  modifications[i.toString()] = importedByte;
                }
              }

              if (hasChanges) {
                detectedModifications.set(mapAddress, modifications);
              }
            }
          }

        }

        setAllMapModifications(detectedModifications);
        setBinaryModifications(new Map());


        await refreshVersions(projectData.fileId);
      } catch (error: any) {
        console.error("❌ [FRONTEND] import error", error?.message || error);
        toast({
          title: t.errors.importError,
          description: t.errors.importFailed,
          variant: "destructive",
        });
      } finally {
        setLoadingAction(null);
      }
    };

    input.click();
  };

  // Encode des édits de maps (cellules en valeurs AFFICHÉES clé "row-col" +
  // labels d'axes) dans un buffer de fichier, avec exactement les règles de
  // décodage de MapViewer : corrections effectives (fenêtre Propriétés),
  // endianness ECU, dé-flip des coordonnées d'affichage (mapAxesFlipRef).
  // Partagé par buildEditedFileData (état courant : hexdump/export) et
  // buildVersionFileData (reconstruction d'une version pour la comparaison).
  const applyEditsToFileData = useCallback((
    data: Uint8Array,
    mods: Map<number, Record<string, number>>,
    axisEdits: Map<number, { x?: string[]; y?: string[] }>,
  ): void => {
    if (!projectData) return;

    const maps = projectData.detectionResults?.maps || [];
    const ecuBigEndian = isBigEndianEcu(projectData.ecu_type);

    // Effective display corrections: per-project overrides saved in the map
    // Properties window win over the detected values. MUST mirror MapViewer's
    // decode logic, otherwise the display<->raw conversion would use two
    // different factors and write wrong bytes for customized maps.
    const effFactor = (factor?: number, divisor?: number): number | undefined => {
      if (typeof factor !== 'number' || !isFinite(factor)) return undefined;
      const div = typeof divisor === 'number' && isFinite(divisor) && divisor !== 0 ? divisor : 1;
      return factor / div;
    };
    const getEffectiveCorrections = (mapInfo: MapData) => {
      const ds = mapDisplaySettingsStore.get(mapInfo.address);
      return {
        cellCorrection: (ds ? effFactor(ds.factor, ds.divisor) : undefined) ?? (mapInfo.correction_factor ?? 1.0),
        cellOffset: ds && typeof ds.offset === 'number' && isFinite(ds.offset) ? ds.offset : (mapInfo.offset ?? 0.0),
        xCorrection: (ds ? effFactor(ds.xAxis.factor, ds.xAxis.divisor) : undefined) ?? mapInfo.x_axis_correction,
        xOffset: ds && typeof ds.xAxis.offset === 'number' && isFinite(ds.xAxis.offset) ? ds.xAxis.offset : mapInfo.x_axis_offset,
        yCorrection: (ds ? effFactor(ds.yAxis.factor, ds.yAxis.divisor) : undefined) ?? mapInfo.y_axis_correction,
        yOffset: ds && typeof ds.yAxis.offset === 'number' && isFinite(ds.yAxis.offset) ? ds.yAxis.offset : mapInfo.y_axis_offset,
      };
    };

    // Write modified axis labels back to the file. mapAxisLabels stores
    // labels in FILE order (MapViewer converts before notifying), so we can
    // index sequentially without any flip handling.
    const writeAxis = (
      addr: number | undefined,
      labels: string[],
      correction: number | undefined,
      offset: number | undefined,
    ) => {
      if (addr === undefined || addr <= 0) return;
      const corr = correction ?? 1.0;
      const off = offset ?? 0.0;
      for (let i = 0; i < labels.length; i++) {
        const value = Number(String(labels[i]).trim().replace(',', '.'));
        if (!Number.isFinite(value)) continue;
        let raw = Math.round((value - off) / (corr || 1));
        if (raw < 0) raw = raw + 65536;
        raw = raw & 0xFFFF;
        const byteAddress = addr + i * 2;
        if (byteAddress < 0 || byteAddress + 1 >= data.length) continue;
        // Axis values are big-endian on EDC16/MJD6, little-endian elsewhere
        // (matches MapViewer's reader).
        if (ecuBigEndian) {
          data[byteAddress] = (raw >> 8) & 0xFF;
          data[byteAddress + 1] = raw & 0xFF;
        } else {
          data[byteAddress] = raw & 0xFF;
          data[byteAddress + 1] = (raw >> 8) & 0xFF;
        }
      }
    };

    axisEdits.forEach((axes, mapAddress) => {
      const mapInfo = maps.find((m: MapData) => m.address === mapAddress);
      if (!mapInfo) return;
      const eff = getEffectiveCorrections(mapInfo);
      // Même garde que MapViewer côté lecture : sur les Boost target EDC15
      // issus d'une détection antérieure au fix du détecteur, x_axis_address
      // et y_axis_address sont croisés (X → axe RPM, Y → axe IQ). Structure
      // fichier [ID u16][len u16 LE][valeurs] → longueur réelle à adresse-2 ;
      // si X pointe l'axe de `rows` valeurs et Y celui de `cols`, on échange
      // avant d'écrire, sinon les labels édités partiraient au mauvais axe.
      let xAddr = mapInfo.x_axis_address;
      let yAddr = mapInfo.y_axis_address;
      const dims2 = mapInfo.dimensions?.TwoDimensional;
      if (
        !ecuBigEndian &&
        (mapInfo.name || '').toLowerCase().includes('boost target map') &&
        dims2 && dims2.rows !== dims2.cols &&
        typeof xAddr === 'number' && xAddr > 2 &&
        typeof yAddr === 'number' && yAddr > 2
      ) {
        const lenAt = (addr: number): number | null =>
          addr - 2 >= 0 && addr < data.length ? (data[addr - 2] | (data[addr - 1] << 8)) : null;
        if (lenAt(xAddr) === dims2.rows && lenAt(yAddr) === dims2.cols) {
          const tmp = xAddr;
          xAddr = yAddr;
          yAddr = tmp;
        }
      }
      if (axes.x && axes.x.length > 0) {
        writeAxis(xAddr, axes.x, eff.xCorrection, eff.xOffset);
      }
      if (axes.y && axes.y.length > 0) {
        writeAxis(yAddr, axes.y, eff.yCorrection, eff.yOffset);
      }
    });

    mods.forEach((changedCells, mapAddress) => {
      const mapInfo = maps.find((m: MapData) => m.address === mapAddress);
      if (!mapInfo) return;

      const dims = mapInfo.dimensions?.TwoDimensional;
      const rows = dims?.rows || 1;
      const cols = dims?.cols || (mapInfo.dimensions?.OneDimensional?.length ?? 1);
      const totalCells = rows * cols;
      if (totalCells === 0) return;
      const cellSize = Math.max(1, Math.floor(mapInfo.size / totalCells));

      const { cellCorrection: correction, cellOffset: offsetValue } = getEffectiveCorrections(mapInfo);
      const isLittleEndian = (mapInfo as { is_little_endian?: boolean }).is_little_endian === true;
      const useBigEndian = !isLittleEndian && ecuBigEndian;

      const flip = mapAxesFlipRef.current.get(mapAddress);
      const rowsReversed = flip?.rowsReversed ?? false;
      const colsReversed = flip?.colsReversed ?? false;

      Object.entries(changedCells).forEach(([key, displayValue]) => {
        const [rowStr, colStr] = key.includes(',') ? key.split(',') : key.split('-');
        const displayRow = parseInt(rowStr, 10);
        const displayCol = parseInt(colStr, 10);
        if (Number.isNaN(displayRow) || Number.isNaN(displayCol)) return;

        // Convert display coords -> file coords using the flip state captured
        // by MapViewer. Without this the topmost displayed row would write to
        // the bottommost file row on maps where Y axis was reversed for display.
        const row = rowsReversed ? (rows - 1 - displayRow) : displayRow;
        const col = colsReversed ? (cols - 1 - displayCol) : displayCol;

        const linearOffset = (row * cols + col) * cellSize;
        const byteAddress = mapAddress + linearOffset;
        if (byteAddress < 0 || byteAddress + cellSize > data.length) return;

        // Inverse of the decode in MapViewer: displayValue = raw * correction + offset
        let raw = Math.round((displayValue - offsetValue) / correction);

        if (cellSize >= 2) {
          // Clamp to 16-bit range (signed Int16 stored as unsigned two's complement)
          if (raw < 0) raw = raw + 65536;
          raw = raw & 0xFFFF;
          if (useBigEndian) {
            data[byteAddress] = (raw >> 8) & 0xFF;
            data[byteAddress + 1] = raw & 0xFF;
          } else {
            data[byteAddress] = raw & 0xFF;
            data[byteAddress + 1] = (raw >> 8) & 0xFF;
          }
        } else {
          if (raw < 0) raw = raw + 256;
          data[byteAddress] = raw & 0xFF;
        }
      });
    });

  }, [projectData, mapDisplaySettingsStore]);

  // Build a file_data byte array that includes the current in-memory map edits.
  const buildEditedFileData = useCallback((): number[] => {
    if (!projectData?.file_data) return [];
    const data = new Uint8Array(projectData.file_data);
    if (allMapModifications.size === 0 && mapAxisLabels.size === 0) return Array.from(data);
    applyEditsToFileData(data, allMapModifications, mapAxisLabels);
    return Array.from(data);
  }, [projectData, allMapModifications, mapAxisLabels, applyEditsToFileData]);

  // Reconstruit les octets COMPLETS d'une version pour la comparaison :
  // base (fichier importé de la version, sinon original) + modifications
  // binaires (DTC) + édits de cellules/axes ré-encodés via
  // applyEditsToFileData. L'ancien loadVersionData du CompareModal écrivait
  // les valeurs AFFICHÉES telles quelles (sans correction_factor/offset ni
  // dé-flip) → différences fausses ou nulles.
  const buildVersionFileData = useCallback(async (versionId: string): Promise<number[]> => {
    const original = originalFileDataRef.current ?? projectData?.file_data ?? [];
    const version = versions.find(v => v.id === versionId);
    if (!version || version.name === "Ori") return [...original];

    const base = versionFileDataRef.current.get(versionId) ?? original;
    const data = new Uint8Array(base);
    try {
      const res = await axios.get(`/api/versioning/map-edits?versionId=${versionId}`);
      const edits = res.data.edits || [];
      const mods = new Map<number, Record<string, number>>();
      const axisEdits = new Map<number, { x?: string[]; y?: string[] }>();
      edits.forEach((edit: { map_address: number; payload?: Record<string, unknown> }) => {
        const mapAddress = edit.map_address;
        const payload = (edit.payload || {}) as {
          type?: string;
          changes?: { address: number; newValue: number }[];
          changedCells?: { row: number; col: number; value: number }[];
          axisLabels?: { x?: string[]; y?: string[] };
        };
        if (mapAddress === -1 && payload.type === "binary") {
          (payload.changes || []).forEach((c) => {
            if (c.address >= 0 && c.address < data.length) data[c.address] = c.newValue & 0xFF;
          });
          return;
        }
        if (Array.isArray(payload.changedCells) && payload.changedCells.length > 0) {
          const cells: Record<string, number> = {};
          payload.changedCells.forEach((cell) => {
            cells[`${cell.row}-${cell.col}`] = cell.value;
          });
          mods.set(mapAddress, cells);
        }
        const axisLabels = payload.axisLabels;
        if (axisLabels && (Array.isArray(axisLabels.x) || Array.isArray(axisLabels.y))) {
          axisEdits.set(mapAddress, {
            ...(Array.isArray(axisLabels.x) ? { x: axisLabels.x } : {}),
            ...(Array.isArray(axisLabels.y) ? { y: axisLabels.y } : {}),
          });
        }
      });
      applyEditsToFileData(data, mods, axisEdits);
    } catch (error) {
      console.error("buildVersionFileData error", error);
    }
    return Array.from(data);
  }, [versions, projectData, applyEditsToFileData]);

  // Données affichées par l'hexdump : fichier courant + édits de maps EN
  // MÉMOIRE (pas encore sauvegardés). Sans ça, les cellules modifiées dans une
  // map n'apparaissent ni en valeur ni en rouge/bleu dans l'hexdump tant
  // qu'aucune version n'est enregistrée (file_data n'est écrit qu'à la
  // sauvegarde/export).
  const hexdumpDisplayData = useMemo(() => buildEditedFileData(), [buildEditedFileData]);

  // Surveillance du checksum : re-vérification (débouncée) à chaque changement
  // des octets courants — édits de maps, DTC, import, changement de version.
  // Un checksum déjà invalide n'est PAS re-vérifié à chaque modification (il
  // le reste forcément) : seuls un changement de version ou le recalcul
  // manuel relancent la vérification.
  const checksumStatusRef = useRef(checksumStatus);
  checksumStatusRef.current = checksumStatus;
  const checksumCheckedVersionRef = useRef<string | null>(null);
  useEffect(() => {
    if (!projectData?.file_data?.length || hexdumpDisplayData.length === 0) return;
    if (!isChecksumSupported(projectData.ecu_type)) {
      setChecksumStatus('unsupported');
      return;
    }
    const versionChanged = checksumCheckedVersionRef.current !== currentVersionId;
    checksumCheckedVersionRef.current = currentVersionId;
    if (!versionChanged && checksumStatusRef.current === 'bad') return;
    setChecksumStatus('checking');
    const timer = setTimeout(() => {
      // correctChecksumByEcuType ne modifie pas son entrée (copie interne) :
      // fixed === 0 signifie que tous les checksums du fichier sont déjà bons
      const res = correctChecksumByEcuType(projectData.ecu_type, hexdumpDisplayData);
      setChecksumStatus(res ? (res.info.fixed === 0 ? 'ok' : 'bad') : 'unsupported');
    }, 400);
    return () => clearTimeout(timer);
  }, [hexdumpDisplayData, currentVersionId, projectData?.ecu_type, projectData?.file_data?.length]);

  // Actual export function (called after checksum decision)
  const performExport = (withChecksum: boolean = false, correctedData?: number[] | null) => {
    if (!projectData?.file_data) {
      toast({
        title: t.errors.exportError,
        description: t.errors.exportNoData,
        variant: "destructive",
      });
      return;
    }

    setLoadingAction("export");

    try {
      // Use corrected data if provided (from checksum calculation), otherwise use the
      // file_data with current in-memory map edits applied.
      const dataToExport = correctedData ?? buildEditedFileData();

      // Créer le fichier binaire à partir des données
      const uint8Array = new Uint8Array(dataToExport);

      // Nom du fichier avec la version courante. withChecksum = les octets
      // exportés ont un checksum valide — corrigé à l'instant OU vérifié
      // déjà bon — dans les deux cas le nom porte _ChecksumOK.
      const currentVersion = versions.find(v => v.id === currentVersionId);
      const checksumSuffix = withChecksum ? '_ChecksumOK' : '';
      const fileName = `${projectData.project_name}_${currentVersion?.name || 'export'}${checksumSuffix}.bin`;

      // Boîte de dialogue native "Enregistrer sous" (le webview n'a pas de
      // téléchargements navigateur)
      void saveBytesToFile(uint8Array, fileName);

    } catch {
      toast({
        title: t.errors.exportError,
        description: t.errors.exportFailed,
        variant: "destructive",
      });
    } finally {
      setLoadingAction(null);
    }
  };

  // Handle export button click - check limits then show checksum modal
  const handleExport = async () => {
    if (!mappackUnlocked) {
      toast({ title: t.mappack?.unlockRequired || "Mappack Locked", description: t.mappack?.unlockRequiredExport || "You must unlock the mappack before exporting the file.", variant: "destructive" });
      return;
    }
    if (!projectData?.file_data) {
      toast({
        title: t.errors.exportError,
        description: t.errors.exportNoData,
        variant: "destructive",
      });
      return;
    }

    // Check daily export limit
    try {
      const res = await fetch("/api/versioning/track-action", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ action: "export" }),
      });
      const data = await res.json();
      if (!data.allowed) {
        toast({ title: t.mappack?.dailyExportLimit || "Daily Export Limit Reached", description: t.mappack?.dailyExportLimitDescription || "You have reached the daily export limit. Try again in 24h.", variant: "destructive" });
        return;
      }
    } catch {
      // If limit check fails, allow the export
    }

    // No checksum module for this ECU (e.g. Marelli MJD6) → skip the correction
    // prompt entirely and export directly.
    if (!isChecksumSupported(projectData.ecu_type)) {
      performExport(false);
      return;
    }

    // Fenêtre d'export : vérifier le checksum des données réellement
    // exportées (édits inclus) pendant que la fenêtre s'ouvre — elle
    // affiche « OK, exporter » ou « incorrect, corriger avant l'export ».
    setExportChecksumStatus('checking');
    setIsChecksumModalOpen(true);
    setTimeout(() => {
      // correctChecksumByEcuType ne modifie pas son entrée : fixed === 0
      // signifie que tous les checksums du fichier sont déjà bons
      const editedData = buildEditedFileData();
      const res = correctChecksumByEcuType(projectData.ecu_type, editedData);
      setExportChecksumStatus(res ? (res.info.fixed === 0 ? 'ok' : 'bad') : 'ok');
    }, 60);
  };

  // Close checksum modal with animation
  const closeChecksumModal = () => {
    setIsChecksumModalClosing(true);
    setTimeout(() => {
      setIsChecksumModalOpen(false);
      setIsChecksumModalClosing(false);
      setIsChecksumCalculating(false);
      setIsChecksumComplete(false);
      setChecksumCorrectedData(null);
    }, 200);
  };

  // Export without checksum
  const handleExportWithoutChecksum = () => {
    closeChecksumModal();
    performExport(false);
  };

  // Checksum déjà bon : export tel quel, mais le nom porte _ChecksumOK
  // (même garantie qu'après une correction manuelle)
  const handleExportChecksumAlreadyOk = () => {
    closeChecksumModal();
    performExport(true);
  };

  // Export with checksum correction
  const handleExportWithChecksum = () => {
    if (!projectData?.file_data) return;

    // Start calculating animation
    setIsChecksumCalculating(true);

    // Use setTimeout to allow the UI to update before calculation
    setTimeout(() => {
      // Apply in-memory map edits before computing the checksum so the
      // corrected bytes reflect the user's current values.
      const editedData = buildEditedFileData();
      const result = correctChecksumByEcuType(projectData.ecu_type, editedData);

      if (result) {
        const { correctedData, info } = result;

        if (info.result === ChecksumResult.ChecksumOK || info.result === ChecksumResult.ChecksumFail) {
          setChecksumCorrectedData(correctedData);
          setIsChecksumCalculating(false);
          setIsChecksumComplete(true);

          // Auto-close modal and export after showing complete state
          setTimeout(() => {
            closeChecksumModal();
            performExport(true, correctedData);
          }, 800);
        } else {
          // ChecksumTypeError - could not determine correct algorithm
          console.warn("⚠️ Could not determine correct checksum algorithm");
          toast({
            title: t.errors.checksumAlgorithmError,
            description: t.errors.checksumAlgorithmFailed,
            variant: "destructive",
          });
          closeChecksumModal();
          performExport(false);
        }
      } else {
        // ECU type not supported for checksum
        closeChecksumModal();
        performExport(false);
      }
    }, 100);
  };

  const handleDelete = async () => {
    if (!currentVersionId) return;

    // Trouver la version courante
    const currentVersion = versions.find(v => v.id === currentVersionId);
    if (!currentVersion) return;

    // Vérifier si c'est la version "Ori"
    if (currentVersion.name === "Ori") {
      toast({
        title: t.errors.cannotDeleteOri,
        description: t.errors.cannotDeleteOriDescription,
        variant: "destructive",
      });
      return;
    }

    // Vérifier s'il n'y a qu'une seule version
    if (versions.length <= 1) {
      toast({
        title: t.errors.cannotDeleteLast,
        description: t.errors.cannotDeleteLastDescription,
        variant: "destructive",
      });
      return;
    }

    // Open the themed confirmation modal (same theme as the settings window)
    setDeleteVersionName(currentVersion.name);
  };

  // Actually delete the current version (called after modal confirmation)
  const performDeleteVersion = async () => {
    setDeleteVersionName(null);
    if (!currentVersionId) return;

    setLoadingAction("delete");
    try {
      await axios.delete(`/api/versioning/versions/${currentVersionId}`);

      if (projectData?.fileId) {
        await refreshVersions(projectData.fileId);
      }

      setSaveNotification({ type: 'deleted', visible: true, fading: false });
      setTimeout(() => {
        setSaveNotification(prev => ({ ...prev, fading: true }));
      }, 2500);
      setTimeout(() => {
        setSaveNotification(prev => ({ ...prev, visible: false, fading: false }));
      }, 3000);
    } catch (error: any) {
      console.error("❌ [FRONTEND] deleteVersion error", error?.message || error);
      toast({
        title: t.errors.deleteFailed,
        description: t.errors.deleteFailedDescription,
        variant: "destructive",
      });
    } finally {
      setLoadingAction(null);
    }
  };

  // Gestion des layouts (drag/resize)
  const clampPosition = (x: number, y: number, width: number, height: number) => {
    const workspaceRect = workspaceRef.current?.getBoundingClientRect();
    if (!workspaceRect) return { x, y };
    const maxX = Math.max(0, workspaceRect.width - width);
    const maxY = Math.max(0, workspaceRect.height - height);
    return {
      x: Math.min(Math.max(0, x), maxX),
      y: Math.min(Math.max(0, y), maxY),
    };
  };

  const rectanglesOverlap = (a: { x: number; y: number; width: number; height: number }, b: { x: number; y: number; width: number; height: number }) => {
    return !(
      a.x + a.width <= b.x ||
      b.x + b.width <= a.x ||
      a.y + a.height <= b.y ||
      b.y + b.height <= a.y
    );
  };

  // (Ajustement auto désactivé sur ouverture)

  const ensureLayoutForMap = (
    map: MapData,
    existingLayouts: Map<number, { x: number; y: number; width: number; height: number }>
  ) => {
    const mapValuesAny = (map as any)?.map_values;
    const dim = (map as any)?.dimensions?.TwoDimensional;
    const mapName = (map.name || "").toLowerCase();

    // Check if this map needs dimension swap (same logic as map-viewer.tsx)
    const apiRows = dim?.rows || 1;
    const apiCols = dim?.cols || 1;
    const isTorqueLimiter = mapName.includes("torque limiter");
    const isIQByMap = mapName.includes("iq by map");
    const isIQByMAF = mapName.includes("iq by maf");
    const isInjectorDuration = mapName.includes("injector duration") && !mapName.includes("selector");
    const isInjectorDurationNon00 = isInjectorDuration && !mapName.includes("duration 00");
    const isEgrMap = mapName.includes("egr") && !mapName.includes("temperature") && !mapName.includes("temp");

    // Determine if dimensions need to be swapped
    let needsSwap = false;
    if (isTorqueLimiter && apiRows > apiCols && apiRows >= 15 && apiCols <= 5) {
      needsSwap = true;
    } else if ((isIQByMap || isIQByMAF) && apiRows < apiCols) {
      needsSwap = true;
    } else if (isInjectorDurationNon00) {
      needsSwap = true;
    }

    // Apply swap if needed - EGR maps always swap (rows = apiCols, cols = apiRows)
    const effectiveRows = isEgrMap ? apiCols : (needsSwap ? apiCols : apiRows);
    const effectiveCols = isEgrMap ? apiRows : (needsSwap ? apiRows : apiCols);

    // For EGR maps, always use effectiveCols/effectiveRows since data is transposed in display
    // For other maps, use mapValuesAny dimensions if available
    const colCount = isEgrMap
      ? effectiveCols
      : ((Array.isArray(mapValuesAny?.[0]) && mapValuesAny[0].length) ? mapValuesAny[0].length : effectiveCols);
    const rowCount = isEgrMap
      ? effectiveRows
      : (Array.isArray(mapValuesAny) ? mapValuesAny.length : effectiveRows);

    // Même inversion : largeur suit les lignes, hauteur suit les colonnes
    const {
      stickyWidth,
      colWidth,
      headerHeight,
      rowHeight,
      chromeHeight,
      paddingHeight,
    } = TEXT_VIEW_SIZING;
    const paddingWidth = getTextPaddingWidth(colCount);

    // Largeur basée sur le nombre de colonnes (X), hauteur sur le nombre de lignes (Y)
    const tableWidth = stickyWidth + colCount * colWidth;
    const tableHeight = headerHeight + rowCount * rowHeight;

    // On ajoute une marge qui frôle l'apparition des scrollbars (réduite pour les très longues tables)
    const baseWidth = tableWidth + paddingWidth;
    const baseHeight = tableHeight + chromeHeight + paddingHeight;
    const dynamicMinHeight = Math.max(140, baseHeight); // réduit pour les maps très basses

    const width = Math.max(240, Math.min(3200, baseWidth));
    const height = Math.min(1800, Math.max(dynamicMinHeight, baseHeight));


    const gap = 4;
    const newLayouts = new Map(existingLayouts);
    const idx = newLayouts.size;
    const offset = Math.min(idx, 10) * 12;

    const existing = newLayouts.get(map.address);
    // Si l'hexdump a été déplacé de sa position d'origine, ouvrir les maps en cascade depuis le coin
    const shouldUseCascadeFromCorner = hexdumpCollapsed || hexdumpMovedFromOrigin;
    let posX =
      (existing ? existing.x : shouldUseCascadeFromCorner ? 0 : hexdumpLayout.x + hexdumpLayout.width + gap) +
      (existing ? 0 : offset);
    let posY = (existing ? existing.y : 0) + (existing ? 0 : offset);

    const workspaceRect = workspaceRef.current?.getBoundingClientRect();
    if (workspaceRect) {
      const clamped = clampPosition(posX, posY, width, height);
      posX = clamped.x;
      posY = clamped.y;
    }

    newLayouts.set(map.address, { x: posX, y: posY, width, height });
    return newLayouts;
  };

  const handleMapDragStart = (mapAddress: number, e: React.MouseEvent) => {
    if (e.button !== 0) return; // drag uniquement au clic gauche
    e.preventDefault();
    e.stopPropagation();
    const layout = mapLayouts.get(mapAddress);
    if (!layout) return;
    dragState.current = {
      address: mapAddress,
      startX: e.clientX,
      startY: e.clientY,
      originX: layout.x,
      originY: layout.y,
    };
    document.addEventListener("mousemove", handleMapDragMove);
    document.addEventListener("mouseup", handleMapDragEnd);
    document.body.style.userSelect = "none";
    setOverlayCursor('move');
    // NOTE: l'overlay anti-interaction n'est activé qu'au premier mouvement réel
    // (dans handleMapDragMove) — sinon il intercepte le mouseup et casse le
    // click des boutons situés dans la barre de titre (fermer, réduire...)
  };

  const handleMapDragMove = (e: MouseEvent) => {
    if (!dragState.current) return;
    setIsWindowDragActive(true);
    const { address, startX, startY, originX, originY } = dragState.current;
    const dx = e.clientX - startX;
    const dy = e.clientY - startY;
    const currentLayout = mapLayouts.get(address);
    if (!currentLayout) return;
    const { x, y } = clampPosition(originX + dx, originY + dy, currentLayout.width, currentLayout.height);
    setMapLayouts((prev) => {
      const next = new Map(prev);
      const layout = next.get(address);
      if (!layout) return prev;
      next.set(address, { ...layout, x, y });
      return next;
    });
  };

  const handleMapDragEnd = () => {
    dragState.current = null;
    document.removeEventListener("mousemove", handleMapDragMove);
    document.removeEventListener("mouseup", handleMapDragEnd);
    document.body.style.userSelect = "";
    setIsWindowDragActive(false);
  };

  const handleMapAutoSize = (mapAddress: number, width: number, height: number) => {
    setMapLayouts((prev) => {
      const next = new Map(prev);
      const existing = next.get(mapAddress);

      // Calculer la hauteur maximale disponible dans le workspace
      const workspaceRect = workspaceRef.current?.getBoundingClientRect();
      const maxWorkspaceHeight = workspaceRect ? workspaceRect.height - 40 : 1800; // 40px de marge en bas

      // Pas de minimum artificiel : la taille demandée est déjà calée sur le
      // tableau réel (un plancher trop haut créerait un espace à droite/en bas)
      const newWidth = Math.max(120, Math.min(3200, width));
      const newHeight = Math.max(90, Math.min(maxWorkspaceHeight, height));


      if (existing) {
        next.set(mapAddress, { ...existing, width: newWidth, height: newHeight });
      } else {
        next.set(mapAddress, { x: 40, y: 30, width: newWidth, height: newHeight });
      }
      return next;
    });
  };

  // NOTE: le redimensionnement des fenêtres de maps est désormais géré par la
  // poignée custom dans MapViewer (handleResizeHandleMouseDown), qui committe
  // la taille finale via onAutoSize -> handleMapAutoSize.

  const handleHexdumpDragStart = (e: React.MouseEvent) => {
    if (e.button !== 0) return; // drag uniquement au clic gauche
    e.preventDefault();
    e.stopPropagation();
    hexdumpDragState.current = {
      startX: e.clientX,
      startY: e.clientY,
      originX: hexdumpLayout.x,
      originY: hexdumpLayout.y,
    };
    document.addEventListener("mousemove", handleHexdumpDragMove);
    document.addEventListener("mouseup", handleHexdumpDragEnd);
    document.body.style.userSelect = "none";
    setOverlayCursor('move');
    // Overlay activé au premier mouvement (voir handleHexdumpDragMove)
  };

  const handleHexdumpDragMove = (e: MouseEvent) => {
    if (!hexdumpDragState.current) return;
    setIsWindowDragActive(true);
    const { startX, startY, originX, originY } = hexdumpDragState.current;
    const dx = e.clientX - startX;
    const dy = e.clientY - startY;
    const { x, y } = clampPosition(originX + dx, originY + dy, hexdumpLayout.width, hexdumpLayout.height);
    setHexdumpLayout((prev) => ({ ...prev, x, y }));
  };

  const handleHexdumpDragEnd = () => {
    // Vérifier si l'hexdump a été déplacé de sa position d'origine (0, 0)
    if (hexdumpLayout.x !== 0 || hexdumpLayout.y !== 0) {
      setHexdumpMovedFromOrigin(true);
    } else {
      setHexdumpMovedFromOrigin(false);
    }

    hexdumpDragState.current = null;
    document.removeEventListener("mousemove", handleHexdumpDragMove);
    document.removeEventListener("mouseup", handleHexdumpDragEnd);
    document.body.style.userSelect = "";
    setIsWindowDragActive(false);
  };

  const handleHexdumpResizeStop = () => {
    const el = hexdumpRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const workspaceRect = workspaceRef.current?.getBoundingClientRect();
    const width = Math.max(360, rect.width);
    const height = Math.max(260, rect.height);
    const x = rect.left - (workspaceRect?.left || 0);
    const y = rect.top - (workspaceRect?.top || 0);
    const clamped = clampPosition(x, y, width, height);
    setHexdumpLayout({ ...clamped, width, height });
  };
  
  // Ref to track if initial load has been done (prevents re-loading on re-renders)
  const initialLoadDoneRef = useRef(false);
  // Track the current projectName to detect navigation between projects
  const currentProjectNameRef = useRef<string | null>(null);

  useEffect(() => {
    // Reset initialLoadDoneRef when projectName changes (navigation to different project)
    if (currentProjectNameRef.current !== null && currentProjectNameRef.current !== projectName) {
      initialLoadDoneRef.current = false;
      // Reset other refs that should be cleared on project change
      originalFileDataRef.current = null;
      originalDetectedMapsRef.current = null;
      hasRefreshedVersions.current = false;
    }
    currentProjectNameRef.current = projectName;

    // Prevent re-loading if already done
    if (initialLoadDoneRef.current) {
      return;
    }

    // Précharger Plotly IMMÉDIATEMENT au chargement de la page pour éliminer toute latence
    // Cela garantit que Plotly est chargé avant même qu'une map ne soit ouverte
    import("react-plotly.js").then(() => {
      // Plotly est maintenant chargé et prêt
    });

    // Set flag to prevent marking changes as unsaved during initial load
    isLoadingVersionRef.current = true;

    // Load project data from sessionStorage
    const storedData = sessionStorage.getItem("currentProject");
    if (storedData) {
      // Mark initial load as done
      initialLoadDoneRef.current = true;

      const data = JSON.parse(storedData);

      // If file_data is not in sessionStorage (quota exceeded prevention), load from PocketBase
      if (!data.file_data && data.fileId) {

        // Set projectData immediately with metadata (without file_data) to show UI
        // This prevents the loading spinner from showing while we fetch binary data
        setProjectData({ ...data, file_data: [] });

        axios.get(`/api/versioning/file-data/${data.fileId}`)
          .then((response) => {
            const fileData = response.data.file_data;

            // Parse detection_data from PocketBase if available (may be string JSON)
            let detectionResults = data.detectionResults;
            if (response.data.detection_data) {
              try {
                const parsed = typeof response.data.detection_data === 'string'
                  ? JSON.parse(response.data.detection_data)
                  : response.data.detection_data;
                if (parsed && parsed.maps) {
                  detectionResults = parsed;
                  // detection_data loaded from PocketBase
                }
              } catch (e) {
                console.warn("Failed to parse detection_data from PocketBase:", e);
              }
            }
            detectionResults = stripSoiTag(detectionResults);

            const mergedData = { ...data, file_data: fileData, detectionResults };
            setProjectData(mergedData);

            // Set mappack unlock status from API response
            if (response.data.mappack_unlocked !== undefined) {
              setMappackUnlocked(response.data.mappack_unlocked);
            }
            if (response.data.mappack_exported !== undefined) {
              setMappackExported(response.data.mappack_exported === true);
            }

            // Restore per-project map display customizations from the backend
            // (files.map_display_settings) — wins over the sessionStorage copy
            // so the saved layout follows the project across sessions/devices.
            const backendDisplaySettings = response.data.map_display_settings;
            if (backendDisplaySettings && typeof backendDisplaySettings === "object") {
              const restored = new Map<number, MapDisplaySettings>();
              Object.entries(backendDisplaySettings).forEach(([key, value]) => {
                const addr = parseInt(key, 10);
                if (!Number.isNaN(addr) && value) {
                  restored.set(addr, value as MapDisplaySettings);
                }
              });
              if (restored.size > 0) {
                setMapDisplaySettingsStore(restored);
              }
            }

            // Also update sessionStorage with new detection results
            saveProjectToSession(mergedData);

            // Store original file data for version switching (never modify this)
            if (fileData && !originalFileDataRef.current) {
              originalFileDataRef.current = [...fileData];
            }

            // Store original detected maps (from database, never modify this)
            if (detectionResults?.maps && !originalDetectedMapsRef.current) {
              originalDetectedMapsRef.current = detectionResults.maps.map((m: { name?: string; address?: number }) => ({
                name: m.name,
                address: m.address
              }));
            }

          })
          .catch((error) => {
            console.error("❌ Failed to load file data from PocketBase:", error);
            toast({
              title: t.errors.loadFileError,
              description: t.errors.loadFileFailed,
              variant: "destructive",
            });
          });
      } else if (!data.file_data && !data.fileId) {
        // No file data available - this shouldn't happen normally
        console.error("❌ No file_data and no fileId - cannot load project");
        toast({
          title: t.errors.loadFileUnavailable,
          description: t.errors.loadFileUnavailableDescription,
          variant: "destructive",
        });
        router.push("/dashboard");
        return;
      } else {
        data.detectionResults = stripSoiTag(data.detectionResults);
        setProjectData(data);
        // Store original file data for version switching (never modify this)
        if (data.file_data && !originalFileDataRef.current) {
          originalFileDataRef.current = [...data.file_data];
        }
        // Store original detected maps (from database, never modify this)
        if (data.detectionResults?.maps && !originalDetectedMapsRef.current) {
          originalDetectedMapsRef.current = data.detectionResults.maps.map((m: { name?: string; address?: number }) => ({
            name: m.name,
            address: m.address
          }));
        }
      }

      setVersions(data.versions || []);
      setCurrentVersionId(data.currentVersionId || null);
      setOpenMaps([]);
      setHexdumpCollapsed(false);

      // Restaurer les settings d'affichage des maps si présents
      if (data.mapDisplaySettings) {
        const restoredSettings = new Map<number, MapDisplaySettings>();
        Object.entries(data.mapDisplaySettings).forEach(([key, value]) => {
          restoredSettings.set(parseInt(key), value as MapDisplaySettings);
        });
        setMapDisplaySettingsStore(restoredSettings);
      }

      // Check mappack unlock status from server
      if (data.fileId) {
        fetch(`/api/mappack/status?fileId=${data.fileId}`)
          .then(res => res.json())
          .then(statusData => {
            setMappackUnlocked(statusData.unlocked === true);
            setMappackIsPro(statusData.isPro === true);
            setMappackExported(statusData.exported === true);
            setMappackExportEnabled(statusData.exportEnabled !== false);
            if (typeof statusData.mappackPrice === "number") {
              setMappackPrice(statusData.mappackPrice);
            }
            if (statusData.unlocked === true) {
              setMappackJustUnlocked(true);
              setTimeout(() => setMappackJustUnlocked(false), 1500);
            }
          })
          .catch(() => {
            setMappackUnlocked(false);
          });
      }

      // Fetch user limits (max versions per project, etc.)
      fetch('/api/limits/status')
        .then(res => res.json())
        .then(limitsData => {
          if (limitsData.limits?.max_versions_per_project) {
            setMaxVersionsPerProject(limitsData.limits.max_versions_per_project);
          }
        })
        .catch(() => {});

      // Reset the loading flag and dirty flag AFTER MapViewer has synced
      setTimeout(() => {
        setHasUnsavedChanges(false);
        isLoadingVersionRef.current = false;
      }, 2000);
    } else {
      // No project data, redirect to upload
      router.push("/upload");
    }
  }, [projectName, router, toast]);

  // Re-détection automatique des maps quand le moteur de détection a évolué
  // depuis la création du projet. Les résultats sont figés dans le projet à
  // sa création : sans ce rattrapage, un projet existant resterait
  // indéfiniment sur des adresses, facteurs ou libellés d'axes périmés.
  // PIÈGE corrigé : l'ancienne version annulait l'appel en cours (drapeau
  // `cancelled` dans le cleanup) dès qu'une dépendance changeait pendant
  // l'attente — hasUnsavedChanges bascule brièvement au chargement d'une
  // version — et le ref « déjà vérifié » était déjà posé : la re-détection
  // n'avait jamais lieu. On laisse l'appel aller au bout et on n'applique le
  // résultat que si le projet ouvert est toujours le même.
  const redetectCheckedRef = useRef<string | null>(null);
  useEffect(() => {
    const fileId = projectData?.fileId;
    if (!fileId || !projectData.detectionResults) return;
    if (hasUnsavedChanges) return; // ne jamais écraser un travail en cours
    if (redetectCheckedRef.current === fileId) return;

    redetectCheckedRef.current = fileId;
    const storedVersion = Number(projectData.detectionResults?.detector_version ?? 0);

    (async () => {
      try {
        const current = await detectorVersion();
        if (current === 0 || storedVersion >= current) return;

        const response = await axios.post(`/api/files/${fileId}/redetect`);
        if (!response.data?.success || !response.data.detectionResults) return;

        const refreshed = stripSoiTag(response.data.detectionResults);
        setProjectData((prev) => {
          if (!prev || prev.fileId !== fileId) return prev;
          const updated = { ...prev, detectionResults: refreshed };
          saveProjectToSession(updated);
          return updated;
        });
        clearMapDataCache();
        toast({
          title: t.notifications.mapsUpdatedTitle,
          description: `${refreshed.total_maps ?? 0} ${t.notifications.mapsUpdatedDescription}`,
        });
      } catch {
        // Rattrapage silencieux : un échec laisse simplement le projet en état.
      }
    })();
  }, [projectData?.fileId, projectData?.detectionResults, hasUnsavedChanges, toast, t]);

  // Prevent leaving page with unsaved modifications
  useEffect(() => {
    const handleBeforeUnload = (e: BeforeUnloadEvent) => {
      if (hasUnsavedChanges) {
        e.preventDefault();
        e.returnValue = "You have unsaved changes. Are you sure you want to leave?";
        return e.returnValue;
      }
    };

    window.addEventListener("beforeunload", handleBeforeUnload);
    return () => window.removeEventListener("beforeunload", handleBeforeUnload);
  }, [hasUnsavedChanges]);

  const persistVersions = useCallback((nextVersions: VersionDto[], nextCurrent: string | null) => {
    setVersions(nextVersions);
    setCurrentVersionId(nextCurrent);
    setProjectData((prev) => {
      if (!prev) return prev;
      const updated = { ...prev, versions: nextVersions, currentVersionId: nextCurrent };
      saveProjectToSession(updated);
      return updated;
    });
  }, []);

  const refreshVersions = useCallback(
    async (fileId: string) => {
      try {
        setLoadingVersions(true);
        const res = await axios.get(`/api/versioning/versions?fileId=${fileId}`);
        const current = res.data.currentVersionId || null;
        const nextVersions = res.data.versions || [];

        setVersions(nextVersions);
        setCurrentVersionId(current);
        setProjectData((prev) => {
          if (!prev) return prev;
          const updated = { ...prev, versions: nextVersions, currentVersionId: current };
          saveProjectToSession(updated);
          return updated;
        });
      } catch (error: any) {
        console.error("❌ [FRONTEND] refreshVersions error", error?.message || error);
        toast({
          title: t.errors.versionsFetchError,
          description: t.errors.versionsFetchFailed,
          variant: "destructive",
        });
      } finally {
        setLoadingVersions(false);
      }
    },
    [toast],
  );

  // Load map modifications from a version
  const loadVersionModifications = useCallback(
    async (versionId: string) => {
      try {
        const res = await axios.get(`/api/versioning/map-edits?versionId=${versionId}`);
        const edits = res.data.edits || [];

        // Reconstruct the allMapModifications Map from the saved edits
        const modificationsMap = new Map<number, Record<string, number>>();
        const loadedBinaryModifications = new Map<number, { oldValue: number; newValue: number }>();
        const loadedAxisLabels = new Map<number, { x?: string[]; y?: string[] }>();

        edits.forEach((edit: any) => {
          const mapAddress = edit.map_address;

          // Check if this is a binary modification (DTC)
          if (mapAddress === -1 && edit.payload?.type === "binary") {
            if (edit.payload.changes && Array.isArray(edit.payload.changes)) {
              edit.payload.changes.forEach((change: { address: number; oldValue: number; newValue: number }) => {
                loadedBinaryModifications.set(change.address, {
                  oldValue: change.oldValue,
                  newValue: change.newValue,
                });
              });
            }
            return;
          }

          const changedCells: Record<string, number> = {};

          // Convert the saved payload back to the format used by MapViewer
          if (edit.payload?.changedCells && Array.isArray(edit.payload.changedCells)) {
            edit.payload.changedCells.forEach((cell: { row: number; col: number; value: number }) => {
              const key = `${cell.row}-${cell.col}`;
              changedCells[key] = cell.value;
            });
          }

          if (Object.keys(changedCells).length > 0) {
            modificationsMap.set(mapAddress, changedCells);
          }

          // Axis label edits persisted alongside cell edits.
          const axisLabels = edit.payload?.axisLabels;
          if (axisLabels && (Array.isArray(axisLabels.x) || Array.isArray(axisLabels.y))) {
            loadedAxisLabels.set(mapAddress, {
              ...(Array.isArray(axisLabels.x) ? { x: axisLabels.x as string[] } : {}),
              ...(Array.isArray(axisLabels.y) ? { y: axisLabels.y as string[] } : {}),
            });
          }
        });

        // Check if this version has imported file data stored
        const importedFileData = versionFileDataRef.current.get(versionId);

        // If this is an imported version, detect differences from original file
        // This ensures maps modified in the imported file are shown in red
        if (importedFileData && originalFileDataRef.current) {
          const originalData = originalFileDataRef.current;

          // Get the current project data to access maps
          // We need to use a different approach since we're in a callback
          const maps = projectData?.detectionResults?.maps || [];

          for (const map of maps) {
            const mapAddress = map.address;
            const mapSize = map.size;

            if (mapAddress >= 0 && mapAddress + mapSize <= importedFileData.length && mapAddress + mapSize <= originalData.length) {
              // Compare map data between original and imported file
              let hasChanges = false;
              const modifications: Record<string, number> = {};

              for (let i = 0; i < mapSize; i++) {
                const originalByte = originalData[mapAddress + i];
                const importedByte = importedFileData[mapAddress + i];

                if (originalByte !== importedByte) {
                  hasChanges = true;
                  // Store the modification (offset within map -> new value)
                  modifications[i.toString()] = importedByte;
                }
              }

              if (hasChanges) {
                modificationsMap.set(mapAddress, modifications);
              }
            }
          }

        }

        setAllMapModifications(modificationsMap);
        setMapAxisLabels(loadedAxisLabels);

        // Reset to original file data OR imported file data, then apply binary modifications
        setProjectData((prevData) => {
          if (!prevData) return prevData;

          let baseData: Uint8Array;

          if (importedFileData) {
            // This version was created by importing a file - use that file's data
            baseData = new Uint8Array(importedFileData);
          } else {
            // This version is based on modifications to the original file
            baseData = originalFileDataRef.current
              ? new Uint8Array(originalFileDataRef.current)
              : new Uint8Array(prevData.file_data);
          }

          // Apply binary modifications for this version
          if (loadedBinaryModifications.size > 0) {
            loadedBinaryModifications.forEach(({ newValue }, address) => {
              baseData[address] = newValue;
            });
          }

          return {
            ...prevData,
            file_data: Array.from(baseData),
          };
        });

        // Les octets binaires (checksum/DTC) viennent d'être réappliqués :
        // forcer une RE-VÉRIFICATION du checksum. Sans ça, la règle « un
        // checksum invalide n'est pas re-vérifié à chaque modification »
        // gobait la correction quand le premier calcul était parti sur les
        // octets bruts pendant le chargement — au retour sur un projet
        // corrigé, le statut restait NOK à tort (course dépendante du cache,
        // d'où l'asymétrie retour immédiat OK / via autre projet NOK).
        checksumCheckedVersionRef.current = null;

        // Clear the map data cache when switching versions
        const { clearMapDataCache } = await import("@/components/map-viewer");
        clearMapDataCache();

        // Clear any pending binary modifications since we just loaded saved ones
        setBinaryModifications(new Map());
      } catch (error: any) {
        console.error("❌ [FRONTEND] loadVersionModifications error", error?.message || error);
      }
    },
    [projectData?.detectionResults?.maps],
  );

  // Only run once when component mounts with fileId
  const hasRefreshedVersions = useRef(false);
  useEffect(() => {
    if (projectData?.fileId && !hasRefreshedVersions.current) {
      hasRefreshedVersions.current = true;
      refreshVersions(projectData.fileId);
    }
  }, [projectData?.fileId, refreshVersions]);

  // Track previous version ID to avoid reloading when versions array changes
  const previousVersionIdRef = useRef<string | null>(null);
  // Flag to skip the next version change (used after save/create to avoid reloading)
  const skipNextVersionChangeRef = useRef(false);
  // Promesse du dernier rechargement de version (lancé en fire-and-forget
  // dans l'effet) — permet aux flux chaînés (recalcul de checksum après
  // enregistrement) d'attendre la FIN réelle du rechargement.
  const versionLoadPromiseRef = useRef<Promise<void> | null>(null);

  // Load modifications when currentVersionId changes (not when versions array changes)
  useEffect(() => {
    // Les octets du binaire arrivent après le montage (requête séparée),
    // alors que currentVersionId est posé immédiatement. Sans cette
    // attente, les modifications binaires de la version — dont les octets
    // de checksum — seraient appliquées sur un tampon vide puis écrasées
    // par le chargement du fichier : au rouvrir, le checksum repassait NOK
    // alors que les édits de cartes, stockés à part, survivaient.
    if ((projectData?.file_data?.length ?? 0) === 0) {
      return;
    }

    // Skip if we just saved/created and should ignore this version change
    if (skipNextVersionChangeRef.current) {
      skipNextVersionChangeRef.current = false;
      previousVersionIdRef.current = currentVersionId;
      return;
    }

    // Skip if version hasn't actually changed
    if (currentVersionId === previousVersionIdRef.current) {
      return;
    }
    previousVersionIdRef.current = currentVersionId;

    if (!currentVersionId) {
      setAllMapModifications(new Map());
      setMapAxisLabels(new Map());
      setHasUnsavedChanges(false);
      return;
    }

    // Set flag to prevent handleMapModifications from marking as unsaved
    isLoadingVersionRef.current = true;

    // Find the current version to check if it's "Ori"
    const currentVersion = versions.find(v => v.id === currentVersionId);

    if (currentVersion?.name === "Ori") {
      // For "Ori" version, start with empty modifications (no persistence)
      setAllMapModifications(new Map());
      setMapAxisLabels(new Map());
      setBinaryModifications(new Map());
      // Reset file_data to original using the stored original data
      if (originalFileDataRef.current) {
        setProjectData((prevData) => {
          if (!prevData) return prevData;
          return {
            ...prevData,
            file_data: [...originalFileDataRef.current!],
          };
        });
        // Clear map data cache to force MapViewer to re-read from original file data
        clearMapDataCache();
      }
    } else {
      // For other versions, load from PocketBase
      versionLoadPromiseRef.current = loadVersionModifications(currentVersionId);
    }

    // Reset the loading flag and dirty flag AFTER MapViewer has synced
    // This ensures any automatic sync from MapViewer doesn't mark as unsaved
    setTimeout(() => {
      setHasUnsavedChanges(false);
      isLoadingVersionRef.current = false;
    }, 1500);
  }, [currentVersionId, versions, loadVersionModifications, projectData?.file_data?.length]);

  // Callback stable pour gérer les modifications de map
  const handleMapModifications = useCallback((mapAddress: number, changedCells: Record<string, number>) => {
    setAllMapModifications(prev => {
      const next = new Map(prev);
      const currentCells = prev.get(mapAddress);
      const newCellsStr = JSON.stringify(changedCells);
      const currentCellsStr = JSON.stringify(currentCells || {});

      // Only update if actually different
      if (newCellsStr === currentCellsStr) {
        return prev; // No change, return same reference
      }

      if (Object.keys(changedCells).length === 0) {
        next.delete(mapAddress);
      } else {
        next.set(mapAddress, changedCells);
      }

      // Mark as dirty only if NOT loading a version (real user edit)
      // We check inside the setter to ensure we only mark dirty for actual changes
      if (!isLoadingVersionRef.current) {
        // Use setTimeout to avoid setState during render
        setTimeout(() => setHasUnsavedChanges(true), 0);
      }

      return next;
    });
  }, []);

  // Handle applying changes to similar maps
  const handleApplyToSimilarMaps = useCallback((sourceMapAddress: number, targetMaps: number[], copyType: 'modifications' | 'all') => {
    if (!projectData) return;

    const sourceMap = projectData.detectionResults.maps.find((m: MapData) => m.address === sourceMapAddress);
    if (!sourceMap) return;

    const sourceDims = sourceMap.dimensions?.TwoDimensional;
    const sourceRows = sourceDims?.rows || 1;
    const sourceCols = sourceDims?.cols || (sourceMap.dimensions?.OneDimensional?.length ?? 1);

    // ── Lecture fidèle à applyEditsToFileData ─────────────────────────
    // Les valeurs d'allMapModifications sont des valeurs AFFICHÉES (raw ×
    // correction + offset) en coordonnées d'AFFICHAGE. Les comparer ou les
    // produire depuis les octets exige les mêmes règles que MapViewer :
    // endianness ECU (EDC15 = little-endian !), corrections effectives,
    // dé-flip des coordonnées. L'ancienne version lisait en big-endian et
    // comparait affiché vs brut — sur EDC15 la copie « map entière » aurait
    // stocké des valeurs byte-swappées sans correction.
    const ecuBigEndian = isBigEndianEcu(projectData.ecu_type);
    const effFactorLocal = (factor?: number, divisor?: number): number | undefined => {
      if (typeof factor !== 'number' || !isFinite(factor)) return undefined;
      const div = typeof divisor === 'number' && isFinite(divisor) && divisor !== 0 ? divisor : 1;
      return factor / div;
    };
    const getCellCorrections = (mapInfo: MapData) => {
      const ds = mapDisplaySettingsStore.get(mapInfo.address);
      return {
        correction: (ds ? effFactorLocal(ds.factor, ds.divisor) : undefined) ?? (mapInfo.correction_factor ?? 1.0),
        offset: ds && typeof ds.offset === 'number' && isFinite(ds.offset) ? ds.offset : (mapInfo.offset ?? 0.0),
      };
    };
    const cellSizeOf = (mapInfo: MapData) =>
      Math.max(1, Math.floor(mapInfo.size / Math.max(1, sourceRows * sourceCols)));
    const readDisplayValue = (mapInfo: MapData, byteAddress: number, cellSize: number): number => {
      const b0 = projectData.file_data[byteAddress] || 0;
      const b1 = projectData.file_data[byteAddress + 1] || 0;
      const isLE = (mapInfo as { is_little_endian?: boolean }).is_little_endian === true;
      const raw = cellSize >= 2
        ? ((!isLE && ecuBigEndian) ? ((b0 << 8) | b1) : ((b1 << 8) | b0))
        : b0;
      const { correction, offset } = getCellCorrections(mapInfo);
      return raw * correction + offset;
    };
    // Coordonnées d'affichage → offset fichier, avec le flip de la map
    // SOURCE (les similar maps partagent le même type donc la même
    // orientation d'affichage).
    const srcFlip = mapAxesFlipRef.current.get(sourceMapAddress);
    const fileOffsetOf = (row: number, col: number): number => {
      const fileRow = srcFlip?.rowsReversed ? (sourceRows - 1 - row) : row;
      const fileCol = srcFlip?.colsReversed ? (sourceCols - 1 - col) : col;
      return fileRow * sourceCols + fileCol;
    };
    // Propager le flip de la source vers une cible jamais ouverte : sans ça,
    // applyEditsToFileData (export/sauvegarde) dé-flipperait les clés copiées
    // avec un état vide et écrirait les lignes en miroir.
    const propagateFlip = (targetAddress: number) => {
      if (srcFlip && !mapAxesFlipRef.current.get(targetAddress)) {
        mapAxesFlipRef.current.set(targetAddress, { ...srcFlip });
      }
    };
    // Sécurité : on ne copie qu'entre maps de dimensions identiques
    const dimsMatch = (targetMap: MapData) => {
      const d = targetMap.dimensions?.TwoDimensional;
      const tRows = d?.rows || 1;
      const tCols = d?.cols || (targetMap.dimensions?.OneDimensional?.length ?? 1);
      return tRows === sourceRows && tCols === sourceCols;
    };

    // Axes édités de la source (stockés en ordre FICHIER dans mapAxisLabels)
    // — propagés vers les cibles quelle que soit l'option choisie.
    const sourceAxes = mapAxisLabels.get(sourceMapAddress);

    if (copyType === 'modifications') {
      // Copy only modifications from source map to target maps (les axes
      // édités, eux, sont propagés plus bas même sans cellule modifiée)
      const sourceModifications = allMapModifications.get(sourceMapAddress);
      const hasCellMods = !!sourceModifications && Object.keys(sourceModifications).length > 0;
      if (!hasCellMods && !sourceAxes) return;

      if (hasCellMods) setAllMapModifications(prev => {
        const next = new Map(prev);
        for (const targetAddress of targetMaps) {
          const targetMap = projectData.detectionResults.maps.find((m: MapData) => m.address === targetAddress);
          if (!targetMap || !dimsMatch(targetMap)) continue;
          propagateFlip(targetAddress);
          const cellSize = cellSizeOf(targetMap);

          // Copy the modifications to target map, only if different from target's original
          const targetModifications: Record<string, number> = { ...(prev.get(targetAddress) || {}) };

          for (const [key, value] of Object.entries(sourceModifications ?? {})) {
            // Key format is "row-col" or "row,col" (display coords)
            const [rowStr, colStr] = key.includes(',') ? key.split(',') : key.split('-');
            const row = parseInt(rowStr, 10);
            const col = parseInt(colStr, 10);
            if (Number.isNaN(row) || Number.isNaN(col)) continue;
            const targetByteAddress = targetAddress + fileOffsetOf(row, col) * cellSize;
            const targetOriginalDisplay = readDisplayValue(targetMap, targetByteAddress, cellSize);

            // Comparaison en valeurs AFFICHÉES des deux côtés
            if (Math.abs(value - targetOriginalDisplay) > 1e-9) {
              targetModifications[key] = value;
            } else {
              // Remove existing modification if values are now equal
              delete targetModifications[key];
            }
          }

          // Only set if there are actual modifications
          if (Object.keys(targetModifications).length > 0) {
            next.set(targetAddress, targetModifications);
          } else {
            next.delete(targetAddress);
          }
        }
        return next;
      });
    } else {
      // Copy entire map values from source to target maps
      const sourceStartAddress = sourceMap.address;
      const sourceCellSize = cellSizeOf(sourceMap);
      const sourceModifications = allMapModifications.get(sourceMapAddress);

      setAllMapModifications(prev => {
        const next = new Map(prev);
        for (const targetAddress of targetMaps) {
          const targetMap = projectData.detectionResults.maps.find((m: MapData) => m.address === targetAddress);
          if (!targetMap || !dimsMatch(targetMap)) continue;
          propagateFlip(targetAddress);
          const targetCellSize = cellSizeOf(targetMap);

          // Create modifications only for cells that differ from target's original values
          const targetModifications: Record<string, number> = { ...(prev.get(targetAddress) || {}) };

          for (let row = 0; row < sourceRows; row++) {
            for (let col = 0; col < sourceCols; col++) {
              const fileOffset = fileOffsetOf(row, col);
              const cellKey = `${row}-${col}`;
              // Support both formats: "row-col" and "row,col" for backwards compatibility
              const cellKeyAlt = `${row},${col}`;

              // Valeur AFFICHÉE de la source : modification en cours sinon
              // valeur d'origine décodée avec les règles de la source
              const sourceDisplay = sourceModifications?.[cellKey] !== undefined
                ? sourceModifications[cellKey]
                : (sourceModifications?.[cellKeyAlt] !== undefined
                  ? sourceModifications[cellKeyAlt]
                  : readDisplayValue(sourceMap, sourceStartAddress + fileOffset * sourceCellSize, sourceCellSize));
              const targetOriginalDisplay = readDisplayValue(targetMap, targetAddress + fileOffset * targetCellSize, targetCellSize);

              // Only add modification if source value differs from target's original value
              if (Math.abs(sourceDisplay - targetOriginalDisplay) > 1e-9) {
                targetModifications[cellKey] = sourceDisplay;
              } else {
                // Remove any existing modification if values are now equal
                delete targetModifications[cellKey];
              }
            }
          }

          // Only set if there are actual modifications
          if (Object.keys(targetModifications).length > 0) {
            next.set(targetAddress, targetModifications);
          } else {
            next.delete(targetAddress);
          }
        }
        return next;
      });
    }

    // Propager aussi les modifications d'axes de la source vers les cibles
    // de mêmes dimensions — avant, seules les cellules de la grille étaient
    // collées et les axes édités restaient propres à la map source.
    if (sourceAxes && (sourceAxes.x || sourceAxes.y)) {
      setMapAxisLabels(prev => {
        const next = new Map(prev);
        for (const targetAddress of targetMaps) {
          const targetMap = projectData.detectionResults.maps.find((m: MapData) => m.address === targetAddress);
          if (!targetMap || !dimsMatch(targetMap)) continue;
          propagateFlip(targetAddress);
          const existing = next.get(targetAddress) || {};
          next.set(targetAddress, {
            ...(sourceAxes.x ? { x: [...sourceAxes.x] } : (existing.x ? { x: existing.x } : {})),
            ...(sourceAxes.y ? { y: [...sourceAxes.y] } : (existing.y ? { y: existing.y } : {})),
          });
        }
        return next;
      });
    }

    // Mark as having unsaved changes
    setTimeout(() => setHasUnsavedChanges(true), 0);
  }, [projectData, allMapModifications, mapAxisLabels, mapDisplaySettingsStore]);

  // Handle modify apply from toolbar (add/fill to active map's selected cells)
  const handleModifyApply = useCallback((operation: 'add' | 'fill', value: number) => {
    if (!activeMapAddress) return;

    // Send command to MapViewer - it will handle the modification of selected cells
    setModifyCommand({ operation, value, timestamp: Date.now() });
  }, [activeMapAddress]);

  // Helper function to count unsaved modifications
  const getUnsavedModificationsCount = useCallback(() => {
    const mapCount = Array.from(allMapModifications.values()).reduce(
      (sum, mapMods) => sum + Object.keys(mapMods).length,
      0
    );
    return mapCount + binaryModifications.size;
  }, [allMapModifications, binaryModifications]);

  // Check if we're on the "Ori" version (unsaved changes would be lost)
  const isOnOriVersion = useCallback(() => {
    const currentVersion = versions.find(v => v.id === currentVersionId);
    return !currentVersion || currentVersion.name === "Ori";
  }, [versions, currentVersionId]);

  // Handle version switch with unsaved changes check
  const handleSelectVersion = (versionId: string) => {
    if (!projectData?.fileId) return;

    // Check if there are unsaved changes since last save/load
    if (hasUnsavedChanges) {
      // Show confirmation dialog
      setPendingVersionSwitch(versionId);
      setShowUnsavedChangesDialog(true);
      return;
    }

    // No unsaved changes, proceed directly
    executeVersionSwitch(versionId);
  };

  // Actually perform the version switch
  const executeVersionSwitch = async (versionId: string) => {
    if (!projectData?.fileId) return;

    // Set flag to prevent marking changes as unsaved during load
    isLoadingVersionRef.current = true;
    setLoadingAction("save");

    try {
      // 1. Marquer cette version comme courante dans PocketBase
      await axios.patch(`/api/versioning/versions/${versionId}`, { isCurrent: true });

      // 2. Rafraîchir la liste des versions
      await refreshVersions(projectData.fileId);

      // 3. Charger les modifications de cette version
      const selectedVersion = versions.find(v => v.id === versionId);
      if (selectedVersion && selectedVersion.name !== "Ori") {
        // Pour une version sauvegardée, charger ses modifications depuis PocketBase
        await loadVersionModifications(versionId);
      } else {
        // Pour "Ori", réinitialiser les modifications (pas de restauration sessionStorage)
        setAllMapModifications(new Map());
        setMapAxisLabels(new Map());
      }

      setShowVersionDropdown(false);

      showInlineNotification(t.notifications.versionLoaded);
    } catch (error: any) {
      console.error("❌ [FRONTEND] selectVersion error", error?.message || error);
      toast({
        title: t.errors.versionSwitchError,
        description: t.errors.versionSwitchFailed,
        variant: "destructive",
      });
    } finally {
      setLoadingAction(null);
      // Reset the loading flag and dirty flag AFTER MapViewer has synced
      setTimeout(() => {
        setHasUnsavedChanges(false);
        isLoadingVersionRef.current = false;
      }, 1000);
    }
  };

  // Handle confirmation dialog actions
  const handleDiscardChanges = () => {
    setAllMapModifications(new Map());
    setMapAxisLabels(new Map());
    setHasUnsavedChanges(false);
    setShowUnsavedChangesDialog(false);

    if (pendingVersionSwitch) {
      executeVersionSwitch(pendingVersionSwitch);
      setPendingVersionSwitch(null);
    }

    if (pendingNavigation) {
      // Clear session storage before navigating away
      sessionStorage.removeItem("currentProject");
      router.push(pendingNavigation);
      setPendingNavigation(null);
    }
  };

  const handleCancelDialog = () => {
    setShowUnsavedChangesDialog(false);
    setPendingVersionSwitch(null);
    setPendingNavigation(null);
  };

  // Handle save and continue with pending action
  const handleSaveAndContinue = async () => {
    const currentVersion = versions.find(v => v.id === currentVersionId);
    const isOri = !currentVersion || currentVersion.name === "Ori";

    if (isOri) {
      // On Ori version - need to create a new version
      if (!checkVersionLimit()) return;
      const newName = await promptForText(t.versionDialogs.newVersionTitle, nextVersionName(), t.versionDialogs.newVersionName);
      if (!newName?.trim()) {
        // User cancelled, stay on dialog
        return;
      }

      setLoadingAction("save");
      try {
        // Create new version
        const versionResponse = await axios.post("/api/versioning/versions", {
          fileId: projectData?.fileId,
          name: newName.trim(),
          baseVersionId: currentVersionId,
          setCurrent: true,
        });

        const newVersionId = versionResponse.data.version.id;

        // Save all map modifications (cell edits + axis label edits) to new
        // version. A map may have cells, axis labels, or both — merge them
        // into a single map-edits row per address.
        const mapAddressesToSave = new Set<number>([
          ...Array.from(allMapModifications.keys()),
          ...Array.from(mapAxisLabels.keys()),
        ]);
        const mapSavePromises = Array.from(mapAddressesToSave).map(async (mapAddress) => {
          const cells = allMapModifications.get(mapAddress);
          const axes = mapAxisLabels.get(mapAddress);
          const payload: Record<string, unknown> = {};
          if (cells && Object.keys(cells).length > 0) {
            payload.changedCells = Object.entries(cells).map(([key, value]) => {
              const [rowStr, colStr] = key.includes(',') ? key.split(',') : key.split('-');
              return { row: parseInt(rowStr), col: parseInt(colStr), value };
            });
          }
          if (axes && (axes.x || axes.y)) {
            payload.axisLabels = {
              ...(axes.x ? { x: axes.x } : {}),
              ...(axes.y ? { y: axes.y } : {}),
            };
          }
          return axios.post("/api/versioning/map-edits", {
            versionId: newVersionId,
            mapAddress,
            payload,
          });
        });

        // Save binary modifications (DTC) to new version
        const binarySavePromise = binaryModifications.size > 0
          ? axios.post("/api/versioning/map-edits", {
              versionId: newVersionId,
              mapAddress: -1, // Adresse spéciale pour les modifications binaires
              payload: {
                type: "binary",
                changes: Array.from(binaryModifications.entries()).map(([addr, { oldValue, newValue }]) => ({
                  address: addr,
                  oldValue,
                  newValue,
                })),
              },
            })
          : Promise.resolve();

        await Promise.all([...mapSavePromises, binarySavePromise]);
        skipNextVersionChangeRef.current = true;
        if (projectData?.fileId) {
          await refreshVersions(projectData.fileId);
        }
        setAllMapModifications(new Map());
        setBinaryModifications(new Map());
        // Note: mapAxisLabels is intentionally NOT cleared here. Clearing it
        // would unset MapViewer's `initialXAxisLabels` and let the next mount
        // re-read the original labels from the file, wiping the user's edits
        // visually until a page reload. The edits are persisted in
        // PocketBase; the local Map mirrors that state.
        setHasUnsavedChanges(false);

        // Show version created notification
        setSaveNotification({ type: 'version', visible: true, fading: false });
        setTimeout(() => {
          setSaveNotification(prev => ({ ...prev, fading: true }));
        }, 2500);
        setTimeout(() => {
          setSaveNotification(prev => ({ ...prev, visible: false, fading: false }));
        }, 3000);
      } catch (error: any) {
        console.error("❌ [FRONTEND] save error", error?.message || error);
        toast({
          title: t.errors.saveError,
          description: t.errors.saveFailed,
          variant: "destructive",
        });
        setLoadingAction(null);
        return; // Don't continue if save failed
      }
      setLoadingAction(null);
    } else {
      // On existing version - save to it
      setLoadingAction("save");
      try {
        // Save map modifications (cell edits + axis label edits merged per map)
        const mapAddressesToSaveExisting = new Set<number>([
          ...Array.from(allMapModifications.keys()),
          ...Array.from(mapAxisLabels.keys()),
        ]);
        const mapSavePromises = Array.from(mapAddressesToSaveExisting).map(async (mapAddress) => {
          const cells = allMapModifications.get(mapAddress);
          const axes = mapAxisLabels.get(mapAddress);
          const payload: Record<string, unknown> = {};
          if (cells && Object.keys(cells).length > 0) {
            payload.changedCells = Object.entries(cells).map(([key, value]) => {
              const [rowStr, colStr] = key.includes(',') ? key.split(',') : key.split('-');
              return { row: parseInt(rowStr), col: parseInt(colStr), value };
            });
          }
          if (axes && (axes.x || axes.y)) {
            payload.axisLabels = {
              ...(axes.x ? { x: axes.x } : {}),
              ...(axes.y ? { y: axes.y } : {}),
            };
          }
          return axios.post("/api/versioning/map-edits", {
            versionId: currentVersionId,
            mapAddress,
            payload,
          });
        });

        // Save binary modifications (DTC) to current version
        const binarySavePromise = binaryModifications.size > 0
          ? axios.post("/api/versioning/map-edits", {
              versionId: currentVersionId,
              mapAddress: -1, // Adresse spéciale pour les modifications binaires
              payload: {
                type: "binary",
                changes: Array.from(binaryModifications.entries()).map(([addr, { oldValue, newValue }]) => ({
                  address: addr,
                  oldValue,
                  newValue,
                })),
              },
            })
          : Promise.resolve();

        await Promise.all([...mapSavePromises, binarySavePromise]);
        skipNextVersionChangeRef.current = true;
        if (projectData?.fileId) {
          await refreshVersions(projectData.fileId);
        }
        setAllMapModifications(new Map());
        setBinaryModifications(new Map());
        // Note: mapAxisLabels intentionally kept; see comment in handleSaveAndContinue.
        setHasUnsavedChanges(false);

        // Show project saved notification
        setSaveNotification({ type: 'save', visible: true, fading: false });
        setTimeout(() => {
          setSaveNotification(prev => ({ ...prev, fading: true }));
        }, 2500);
        setTimeout(() => {
          setSaveNotification(prev => ({ ...prev, visible: false, fading: false }));
        }, 3000);
      } catch (error: any) {
        console.error("❌ [FRONTEND] save error", error?.message || error);
        toast({
          title: t.errors.saveError,
          description: t.errors.saveFailed,
          variant: "destructive",
        });
        setLoadingAction(null);
        return; // Don't continue if save failed
      }
      setLoadingAction(null);
    }

    // Now continue with pending action
    setShowUnsavedChangesDialog(false);

    if (pendingVersionSwitch) {
      executeVersionSwitch(pendingVersionSwitch);
      setPendingVersionSwitch(null);
    }

    if (pendingNavigation) {
      sessionStorage.removeItem("currentProject");
      router.push(pendingNavigation);
      setPendingNavigation(null);
    }
  };

  // Handle close project with unsaved changes check
  const handleCloseProject = () => {
    if (hasUnsavedChanges) {
      // Show confirmation dialog
      setPendingNavigation("/dashboard");
      setShowUnsavedChangesDialog(true);
      return;
    }

    // No unsaved changes, proceed to dashboard
    sessionStorage.removeItem("currentProject");
    router.push("/dashboard");
  };

  // Export the project's ORIGINAL mappack as a WinOLS-compatible JSON file.
  // Server-side rules: mappack unlocked, once per project, daily limit,
  // export must be enabled for the ECU, price set per-ECU (blue coins).
  // Uses detection_data (per-project display customizations are not included).
  const [isMappackExportModalOpen, setIsMappackExportModalOpen] = useState(false);
  const [isMappackExportModalClosing, setIsMappackExportModalClosing] = useState(false);
  const [isExportingMappack, setIsExportingMappack] = useState(false);
  const [isMappackExportComplete, setIsMappackExportComplete] = useState(false);

  // Toolbar button: validate then show the confirmation modal
  // (same style/flow as the checksum export modal)
  const handleExportMappack = () => {
    if (isExportingMappack) return;
    if (!projectData?.fileId) {
      toast({ title: t.toolbar.exportMappackError, variant: "destructive" });
      return;
    }
    if (mappackExported) {
      toast({ title: t.toolbar.exportMappackAlready, variant: "destructive" });
      return;
    }
    if (!mappackUnlocked) {
      toast({ title: t.toolbar.exportMappackLocked, variant: "destructive" });
      return;
    }
    if (!mappackExportEnabled) {
      toast({ title: t.toolbar.exportMappackDisabled, variant: "destructive" });
      return;
    }
    setIsMappackExportModalOpen(true);
  };

  // Close mappack export modal with animation (same timing as checksum modal)
  const closeMappackExportModal = () => {
    setIsMappackExportModalClosing(true);
    setTimeout(() => {
      setIsMappackExportModalOpen(false);
      setIsMappackExportModalClosing(false);
      setIsExportingMappack(false);
      setIsMappackExportComplete(false);
    }, 200);
  };

  // Confirmed from the modal: perform the export
  const handleConfirmMappackExport = async () => {
    if (isExportingMappack || !projectData?.fileId) return;

    setIsExportingMappack(true);
    try {
      const response = await fetch("/api/mappack/export", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        // sortMode : l'export reprend l'ordre courant de la liste des maps
        body: JSON.stringify({ fileId: projectData.fileId, sortMode: mapSortMode }),
      });

      if (!response.ok) {
        let errCode = "";
        let errData: { error?: string; required?: number } = {};
        try {
          errData = await response.json();
          errCode = errData.error || "";
        } catch {}
        const message =
          errCode === "mappack_locked"
            ? t.toolbar.exportMappackLocked
            : errCode === "export_disabled_for_ecu"
              ? t.toolbar.exportMappackDisabled
              : errCode === "already_exported"
                ? t.toolbar.exportMappackAlready
                : errCode === "insufficient_credits"
                  ? t.toolbar.exportMappackNoCredits.replace(
                      "{required}",
                      String(errData.required ?? mappackPrice)
                    )
                  : errCode === "daily_limit_reached"
                    ? t.toolbar.exportMappackLimit
                    : t.toolbar.exportMappackError;
        if (errCode === "export_disabled_for_ecu") {
          setMappackExportEnabled(false);
        }
        if (errCode === "already_exported") {
          setMappackExported(true);
        }
        closeMappackExportModal();
        toast({ title: message, variant: "destructive" });
        return;
      }

      // Save the file exactly as served (latin-1 bytes) via the native dialog
      const blob = await response.blob();
      const headerName = response.headers.get("X-Mappack-Filename");
      const fileName = headerName
        ? decodeURIComponent(headerName)
        : `Mappack ${projectData.project_name} ZedSuite.json`;
      const bytes = new Uint8Array(await blob.arrayBuffer());
      await saveBytesToFile(bytes, fileName);

      // Show the success state in the modal, then auto-close (checksum-modal flow)
      setIsExportingMappack(false);
      setIsMappackExportComplete(true);
      setMappackExported(true);
      const mapsCount = response.headers.get("X-Maps-Count") ?? "?";
      setTimeout(() => {
        closeMappackExportModal();
        toast({
          title: t.toolbar.exportMappackSuccess.replace("{maps}", mapsCount),
        });
      }, 800);
    } catch {
      closeMappackExportModal();
      toast({ title: t.toolbar.exportMappackError, variant: "destructive" });
    }
  };

  const handleRenameVersion = async () => {
    if (!currentVersionId) return;

    // Trouver la version courante
    const currentVersion = versions.find(v => v.id === currentVersionId);
    if (!currentVersion) return;

    // Vérifier si c'est la version "Ori"
    if (currentVersion.name === "Ori") {
      toast({
        title: t.versionDialogs.cannotRenameOriTitle,
        description: t.versionDialogs.cannotRenameOriDescription,
        variant: "destructive",
      });
      return;
    }

    // Open the themed rename modal (same theme as the settings window)
    setRenameVersionValue(currentVersion.name);
  };

  // Actually rename the current version (called after modal confirmation)
  const performRenameVersion = async (rawName: string) => {
    const currentVersion = versions.find(v => v.id === currentVersionId);
    setRenameVersionValue(null);
    const newName = rawName.trim();
    if (!currentVersionId || !newName || newName === currentVersion?.name) return;

    setLoadingAction("rename");
    try {
      await axios.patch(`/api/versioning/versions/${currentVersionId}`, { name: newName });
      if (projectData?.fileId) {
        await refreshVersions(projectData.fileId);
      }
      showInlineNotification(t.notifications.versionRenamed);
    } catch (error: any) {
      console.error("❌ [FRONTEND] renameVersion error", error?.message || error);
      toast({
        title: t.versionDialogs.renameError,
        description: t.versionDialogs.cannotRenameOri,
        variant: "destructive",
      });
    } finally {
      setLoadingAction(null);
    }
  };

  const checkVersionLimit = (): boolean => {
    if (versions.length >= maxVersionsPerProject) {
      setVersionLimitNotification({ visible: true, fading: false });
      setTimeout(() => {
        setVersionLimitNotification(prev => ({ ...prev, fading: true }));
        setTimeout(() => {
          setVersionLimitNotification({ visible: false, fading: false });
        }, 500);
      }, 3000);
      return false;
    }
    return true;
  };

  const handleCreateVersion = async () => {
    if (!projectData?.fileId) return;
    if (!mappackUnlocked) {
      toast({ title: t.mappack?.unlockRequired || "Mappack Locked", description: t.mappack?.unlockRequiredVersion || "You must unlock the mappack before creating a new version.", variant: "destructive" });
      return;
    }
    if (!checkVersionLimit()) return;

    // Compter les modifications (maps + binaires/DTC + axis labels) pour le message de confirmation
    const mapModificationsCount = Array.from(allMapModifications.values()).reduce(
      (sum, mapMods) => sum + Object.keys(mapMods).length,
      0
    );
    const totalModifications = mapModificationsCount + binaryModifications.size + mapAxisLabels.size;

    const newName = await promptForText(t.versionDialogs.newVersionTitle, nextVersionName(), t.versionDialogs.newVersionName);
    if (!newName?.trim()) return;

    // Set flag to prevent marking changes as unsaved during save
    isLoadingVersionRef.current = true;
    setLoadingAction("save");

    try {
      // 1. Créer la nouvelle version
      const versionResponse = await axios.post("/api/versioning/versions", {
        fileId: projectData.fileId,
        name: newName.trim(),
        baseVersionId: currentVersionId,
        setCurrent: true,
      });

      const newVersionId = versionResponse.data.version.id;

      // 2. Sauvegarder toutes les modifications de maps (cells + axis labels)
      // dans cette version, fusionnées par adresse.
      const mapAddressesToSaveCreate = new Set<number>([
        ...Array.from(allMapModifications.keys()),
        ...Array.from(mapAxisLabels.keys()),
      ]);
      const mapSavePromises = Array.from(mapAddressesToSaveCreate).map(async (mapAddress) => {
        const cells = allMapModifications.get(mapAddress);
        const axes = mapAxisLabels.get(mapAddress);
        const payload: Record<string, unknown> = {};
        if (cells && Object.keys(cells).length > 0) {
          payload.changedCells = Object.entries(cells).map(([key, value]) => {
            const [rowStr, colStr] = key.includes(',') ? key.split(',') : key.split('-');
            return { row: parseInt(rowStr), col: parseInt(colStr), value };
          });
        }
        if (axes && (axes.x || axes.y)) {
          payload.axisLabels = {
            ...(axes.x ? { x: axes.x } : {}),
            ...(axes.y ? { y: axes.y } : {}),
          };
        }
        return axios.post("/api/versioning/map-edits", {
          versionId: newVersionId,
          mapAddress,
          payload,
        });
      });

      // 3. Sauvegarder les modifications binaires (DTC) dans cette version
      const binarySavePromise = binaryModifications.size > 0
        ? axios.post("/api/versioning/map-edits", {
            versionId: newVersionId,
            mapAddress: -1, // Adresse spéciale pour les modifications binaires
            payload: {
              type: "binary",
              changes: Array.from(binaryModifications.entries()).map(([addr, { oldValue, newValue }]) => ({
                address: addr,
                oldValue,
                newValue,
              })),
            },
          })
        : Promise.resolve();

      await Promise.all([...mapSavePromises, binarySavePromise]);

      // Reset binary modifications after save.
      // Note: mapAxisLabels intentionally kept so the user's axis edits stay
      // visible across map close/reopen until the next version switch.
      setBinaryModifications(new Map());

      // 4. Rafraîchir les versions - let the useEffect reload modifications from DB
      // This ensures consistency between local state and database
      isLoadingVersionRef.current = true;
      await refreshVersions(projectData.fileId);

      // Show version created notification
      setSaveNotification({ type: 'version', visible: true, fading: false });
      setTimeout(() => {
        setSaveNotification(prev => ({ ...prev, fading: true }));
      }, 2500);
      setTimeout(() => {
        setSaveNotification(prev => ({ ...prev, visible: false, fading: false }));
      }, 3000);
    } catch (error: any) {
      console.error("❌ [FRONTEND] createVersion error", error?.message || error);
      console.error("❌ [FRONTEND] Error details:", error?.response?.data || error);
      toast({
        title: t.errors.createVersionError,
        description: t.errors.createVersionFailed,
        variant: "destructive",
      });
    } finally {
      setLoadingAction(null);
      // Reset the loading flag and dirty flag AFTER MapViewer has synced
      setTimeout(() => {
        setHasUnsavedChanges(false);
        isLoadingVersionRef.current = false;
      }, 1000);
    }
  };

  // Handle save: update existing version or create new one if on "Ori"
  const handleSave = async () => {
    if (!projectData?.fileId) return;
    if (!mappackUnlocked) {
      toast({ title: t.mappack?.unlockRequired || "Mappack Locked", description: t.mappack?.unlockRequiredVersion || "You must unlock the mappack before creating a new version.", variant: "destructive" });
      return;
    }

    // Check if there are modifications (maps + binaires/DTC + axis labels)
    const mapModificationsCount = Array.from(allMapModifications.values()).reduce(
      (sum, mapMods) => sum + Object.keys(mapMods).length,
      0
    );
    const axisLabelModificationsCount = mapAxisLabels.size;
    const totalModifications = mapModificationsCount + binaryModifications.size + axisLabelModificationsCount;

    if (totalModifications === 0) {
      toast({
        title: t.errors.noChanges,
        description: t.errors.noChangesDescription,
        variant: "destructive",
      });
      return;
    }

    // Find current version
    const currentVersion = versions.find(v => v.id === currentVersionId);

    // If on "Ori" version, create a new version (awaited so callers can
    // chain work after the save — e.g. the checksum recalculation)
    if (!currentVersion || currentVersion.name === "Ori") {
      await handleCreateVersion();
      return;
    }

    // Otherwise, update the existing version
    isLoadingVersionRef.current = true;
    setLoadingAction("save");
    try {
      // Save all map modifications (cells + axis labels merged per map) to
      // the current version.
      const mapAddressesToSaveUpdate = new Set<number>([
        ...Array.from(allMapModifications.keys()),
        ...Array.from(mapAxisLabels.keys()),
      ]);
      const mapSavePromises = Array.from(mapAddressesToSaveUpdate).map(async (mapAddress) => {
        const cells = allMapModifications.get(mapAddress);
        const axes = mapAxisLabels.get(mapAddress);
        const payload: Record<string, unknown> = {};
        if (cells && Object.keys(cells).length > 0) {
          payload.changedCells = Object.entries(cells).map(([key, value]) => {
            const [rowStr, colStr] = key.includes(',') ? key.split(',') : key.split('-');
            return { row: parseInt(rowStr), col: parseInt(colStr), value };
          });
        }
        if (axes && (axes.x || axes.y)) {
          payload.axisLabels = {
            ...(axes.x ? { x: axes.x } : {}),
            ...(axes.y ? { y: axes.y } : {}),
          };
        }
        return axios.post("/api/versioning/map-edits", {
          versionId: currentVersionId,
          mapAddress,
          payload,
        });
      });

      // Save binary modifications (DTC) to the current version
      const binarySavePromise = binaryModifications.size > 0
        ? axios.post("/api/versioning/map-edits", {
            versionId: currentVersionId,
            mapAddress: -1, // Adresse spéciale pour les modifications binaires
            payload: {
              type: "binary",
              changes: Array.from(binaryModifications.entries()).map(([addr, { oldValue, newValue }]) => ({
                address: addr,
                oldValue,
                newValue,
              })),
            },
          })
        : Promise.resolve();

      await Promise.all([...mapSavePromises, binarySavePromise]);

      // Save detection_data to the local store
      if (projectData.detectionResults) {
        const updateData: Record<string, unknown> = {};
        updateData.detection_data = projectData.detectionResults;
        await axios.patch(`/api/files/${projectData.fileId}`, updateData);

        // Update the original refs so discard works correctly
        if (projectData.detectionResults?.maps) {
          originalDetectedMapsRef.current = projectData.detectionResults.maps.map((m: { name?: string; address?: number }) => ({
            name: m.name,
            address: m.address
          }));
        }
      }

      // Reset binary modifications after save.
      // Note: mapAxisLabels intentionally kept so axis edits remain visible
      // across map close/reopen until the next version switch.
      setBinaryModifications(new Map());

      // Refresh versions - skip reload since we're on the same version
      // The modifications are already in local state and now saved to DB
      skipNextVersionChangeRef.current = true;
      isLoadingVersionRef.current = true;
      await refreshVersions(projectData.fileId);

      // Show project saved notification
      setSaveNotification({ type: 'save', visible: true, fading: false });
      setTimeout(() => {
        setSaveNotification(prev => ({ ...prev, fading: true }));
      }, 2500);
      setTimeout(() => {
        setSaveNotification(prev => ({ ...prev, visible: false, fading: false }));
      }, 3000);
    } catch (error: any) {
      console.error("❌ [FRONTEND] save error", error?.message || error);
      toast({
        title: t.errors.saveChangesError,
        description: t.errors.saveChangesFailed,
        variant: "destructive",
      });
    } finally {
      setLoadingAction(null);
      // Reset the loading flag and dirty flag AFTER MapViewer has synced
      setTimeout(() => {
        setHasUnsavedChanges(false);
        isLoadingVersionRef.current = false;
      }, 1000);
    }
  };

  // Keep handleSave ref up to date for auto-save
  handleSaveRef.current = handleSave;

  // ── Recalcul manuel du checksum ────────────────────────────────────────
  // Corrige les checksums sur les octets courants (édits inclus), enregistre
  // les octets réécrits comme modifications binaires (même canal que les DTC)
  // puis ENREGISTRE la version — la pastille "projet enregistré" doit être
  // visible, le toast checksum n'arrive qu'après pour ne pas la masquer
  // (les deux s'affichent au même endroit, en bas au centre).
  const performChecksumRecalc = async () => {
    if (!projectData?.file_data?.length) return;
    const currentData = buildEditedFileData();
    const res = correctChecksumByEcuType(projectData.ecu_type, currentData);
    if (!res) return;
    const { correctedData, info } = res;

    if (info.fixed === 0) {
      setChecksumStatus('ok');
      toast({ title: t.checksum.alreadyValidTitle, description: t.checksum.alreadyValidDescription });
      return;
    }

    const changedAddresses: number[] = [];
    for (let i = 0; i < correctedData.length; i++) {
      if (correctedData[i] !== currentData[i]) changedAddresses.push(i);
    }

    const originalData = originalFileDataRef.current;
    setBinaryModifications(prev => {
      const mods = new Map(prev);
      for (const addr of changedAddresses) {
        mods.set(addr, {
          oldValue: originalData?.[addr] ?? currentData[addr],
          newValue: correctedData[addr],
        });
      }
      return mods;
    });
    setProjectData(prev => {
      if (!prev) return prev;
      const fileData = [...prev.file_data];
      for (const addr of changedAddresses) fileData[addr] = correctedData[addr];
      return { ...prev, file_data: fileData };
    });
    setHasUnsavedChanges(true);
    setChecksumStatus('ok');

    // Laisser React flusher les états pour que handleSaveRef voie les
    // nouveaux octets, puis enregistrer la version (pastille de sauvegarde)
    await new Promise((r) => setTimeout(r, 250));
    await handleSaveRef.current?.();

    // Toast checksum après que la pastille de sauvegarde a été visible
    setTimeout(() => {
      toast({ title: t.checksum.correctedTitle, description: `${info.fixed} ${t.checksum.correctedDescription}` });
    }, 1200);
  };

  // Refs TOUJOURS fraîches pour le flux chaîné « enregistrer puis
  // recalculer » : une closure capturée AVANT l'enregistrement ré-encode
  // les édits avec des états périmés (modifications rechargées, flips,
  // réglages d'affichage) — sur EDC16 les octets divergeaient et la
  // correction calculée ne validait pas les octets finaux : la pastille
  // restait NOK alors que le clic manuel (closure du dernier rendu)
  // fonctionnait toujours.
  const performChecksumRecalcRef = useRef(performChecksumRecalc);
  performChecksumRecalcRef.current = performChecksumRecalc;
  const buildEditedFileDataRef = useRef(buildEditedFileData);
  buildEditedFileDataRef.current = buildEditedFileData;

  const handleRecalcChecksum = () => {
    if (!projectData?.file_data?.length || !isChecksumSupported(projectData.ecu_type)) return;
    if (hasUnsavedChanges) {
      setShowChecksumSaveConfirm(true);
      return;
    }
    void performChecksumRecalc();
  };

  // Enregistre d'abord, corrige ensuite — exactement l'enchaînement manuel
  // (bouton SAVE puis recalcul), le seul qui donne un résultat correct.
  const handleSaveAndRecalcChecksum = async () => {
    setShowChecksumSaveConfirm(false);
    const ecuType = projectData?.ecu_type;
    if (!ecuType) return;
    await handleSaveRef.current?.();
    // Attendre la FIN réelle du rechargement de version (lancé en
    // fire-and-forget par l'effet de changement de version), puis le
    // relâchement des drapeaux de chargement — jamais un délai fixe.
    await new Promise((r) => setTimeout(r, 400));
    await versionLoadPromiseRef.current;
    const settleStart = Date.now();
    while (isLoadingVersionRef.current && Date.now() - settleStart < 10000) {
      await new Promise((r) => setTimeout(r, 200));
    }
    await new Promise((r) => setTimeout(r, 300));
    // Recalculer via la ref (closure du DERNIER rendu), puis CONTRE-VÉRIFIER
    // les octets finaux ; si une écriture tardive a écrasé la correction,
    // refaire une passe — l'équivalent exact du clic manuel.
    for (let attempt = 0; attempt < 3; attempt++) {
      await performChecksumRecalcRef.current?.();
      await new Promise((r) => setTimeout(r, 800));
      const finalData = buildEditedFileDataRef.current?.() ?? [];
      const check = finalData.length ? correctChecksumByEcuType(ecuType, finalData) : null;
      if (!check || check.info.fixed === 0) break;
    }
  };

  // Auto-save functionality.
  // Le compteur de modifications est lu via une ref : l'interval capture ses
  // closures au montage du timer, et les états React (Maps remplacées par les
  // setters) y resteraient figés à vide — l'auto-save ne déclencherait jamais.
  const autoSaveModsRef = useRef(0);
  useEffect(() => {
    const mapModificationsCount = Array.from(allMapModifications.values()).reduce(
      (sum, mapMods) => sum + Object.keys(mapMods).length,
      0
    );
    autoSaveModsRef.current =
      mapModificationsCount + binaryModifications.size + mapAxisLabels.size;
  }, [allMapModifications, binaryModifications, mapAxisLabels]);

  useEffect(() => {
    if (!settings.autoSave || !projectData?.fileId) return;

    // Parse interval to milliseconds
    const intervalMs = (() => {
      switch (settings.autoSaveInterval) {
        case '2min': return 2 * 60 * 1000;
        case '5min': return 5 * 60 * 1000;
        case '15min': return 15 * 60 * 1000;
        case '30min': return 30 * 60 * 1000;
        default: return 15 * 60 * 1000;
      }
    })();

    const autoSaveTimer = setInterval(async () => {
      // Même action que le bouton Enregistrer, seulement s'il y a des
      // modifications en attente (maps + binaires/DTC + axes).
      if (autoSaveModsRef.current > 0 && handleSaveRef.current) {
        await handleSaveRef.current();
      }
    }, intervalMs);

    return () => clearInterval(autoSaveTimer);
  }, [settings.autoSave, settings.autoSaveInterval, projectData?.fileId]);

  // Handler pour fermer le modal Project Info avec animation
  const handleCloseProjectInfoModal = () => {
    setIsClosingProjectInfoModal(true);
    setTimeout(() => {
      setShowProjectInfoModal(false);
      setIsClosingProjectInfoModal(false);
    }, 200);
  };

  // Handler pour fermer le Settings Menu avec animation
  const handleCloseSettings = () => {
    setIsClosingSettings(true);
    setTimeout(() => {
      setIsSettingsOpen(false);
      setIsClosingSettings(false);
    }, 200);
  };

  // Handler pour sauvegarder les informations du projet
  const handleSaveProjectInfo = async (updatedInfo: Partial<ProjectData>) => {
    if (!projectData?.fileId) return;

    try {
      // Mettre à jour PocketBase
      await axios.patch(`/api/files/${projectData.fileId}`, {
        project_name: updatedInfo.project_name,
        vehicle_brand: updatedInfo.vehicle_brand,
        vehicle_model: updatedInfo.vehicle_model,
        engine_type: updatedInfo.engine_type,
        transmission_type: updatedInfo.transmission_type,
        year: updatedInfo.year,
        power: updatedInfo.power,
        customer: updatedInfo.customer,
        stage: updatedInfo.stage,
        notes: updatedInfo.notes,
      });

      // Mettre à jour le state local et sessionStorage
      setProjectData((prev) => {
        if (!prev) return prev;
        const updated = { ...prev, ...updatedInfo };
        saveProjectToSession(updated);
        return updated;
      });

      showInlineNotification(t.notifications.projectUpdated);

      handleCloseProjectInfoModal();
    } catch (error: any) {
      console.error("❌ [FRONTEND] saveProjectInfo error", error?.message || error);
      toast({
        title: t.errors.saveInfoError,
        description: t.errors.saveInfoFailed,
        variant: "destructive",
      });
    }
  };

  // Handler pour ouvrir le modal Map Properties
  const handleOpenMapProperties = (map: MapData) => {
    setMapPropertiesTarget(map);
    setShowMapPropertiesModal(true);
  };

  // Handler pour fermer le modal Map Properties avec animation
  const handleCloseMapPropertiesModal = () => {
    setIsClosingMapPropertiesModal(true);
    setTimeout(() => {
      setShowMapPropertiesModal(false);
      setIsClosingMapPropertiesModal(false);
      setMapPropertiesTarget(null);
    }, 200);
  };

  // Handler pour sauvegarder les paramètres d'affichage d'une map
  const handleSaveMapDisplaySettings = (mapAddress: number, settings: MapDisplaySettings) => {
    setMapDisplaySettingsStore(prev => {
      const newStore = new Map(prev);
      newStore.set(mapAddress, settings);

      // Sauvegarder dans projectData (sans file_data) pour persister avec le projet
      const settingsObj: Record<string, MapDisplaySettings> = {};
      newStore.forEach((value, key) => {
        settingsObj[key.toString()] = value;
      });
      if (projectData) {
        const updatedData = { ...projectData, mapDisplaySettings: settingsObj };
        saveProjectToSession(updatedData);
      }

      // Persist to the backend (PocketBase files.map_display_settings) so the
      // per-project layout survives browser restarts and other devices.
      if (projectData?.fileId) {
        fetch(`/api/files/${projectData.fileId}/display-settings`, {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ settings: settingsObj }),
        }).catch(() => {
          // Session copy already saved — backend retry happens on next save
        });
      }

      return newStore;
    });

    showInlineNotification(t.notifications.settingsSaved);
  };

  // Getter pour les settings d'une map (retourne les valeurs par défaut si non définies)
  const getMapDisplaySettings = useCallback((map: MapData): MapDisplaySettings => {
    const stored = mapDisplaySettingsStore.get(map.address);
    if (stored) return stored;
    return getDefaultMapDisplaySettings(map);
  }, [mapDisplaySettingsStore]);

  // Toggle du bouton "inverser l'affichage" dans l'en-tête d'une map. Réutilise
  // le même chemin de persistance que la fenêtre Propriétés (store + PocketBase).
  const handleToggleInvertDisplay = useCallback((mapAddress: number, invert: boolean) => {
    const map = projectData?.detectionResults?.maps?.find((m: MapData) => m.address === mapAddress);
    if (!map) return;
    const base = mapDisplaySettingsStore.get(mapAddress) ?? getDefaultMapDisplaySettings(map);
    handleSaveMapDisplaySettings(mapAddress, { ...base, invertDisplay: invert });
  }, [projectData, mapDisplaySettingsStore, handleSaveMapDisplaySettings]);

  const toggleFolder = (folderName: string) => {
    // Block opening sub-folders when mappack is locked (only allow "all" root folder)
    if (!mappackUnlocked && folderName !== "all") {
      return;
    }
    const newExpanded = new Set(expandedFolders);
    if (newExpanded.has(folderName)) {
      newExpanded.delete(folderName);
    } else {
      newExpanded.add(folderName);
    }
    setExpandedFolders(newExpanded);
  };

  // Handle mappack unlock - show confirmation first
  const handleUnlockMappack = (e: React.MouseEvent<HTMLButtonElement>) => {
    (e.currentTarget as HTMLButtonElement).blur();
    if (!projectData?.fileId || mappackUnlocking) return;
    if (!mappackIsPro) {
      toast({ title: t.mappack?.proRequired || "Pro Required", description: t.mappack?.proRequiredDescription || "A Pro subscription is required to unlock mappacks.", variant: "destructive" });
      return;
    }
    setShowUnlockConfirm(true);
  };

  const confirmUnlockMappack = async () => {
    setShowUnlockConfirm(false);
    if (!projectData?.fileId || mappackUnlocking) return;
    setMappackUnlocking(true);
    try {
      const response = await axios.post("/api/mappack/unlock", { fileId: projectData.fileId });
      if (response.data.success) {
        // Reload file data with full detection_data
        const fileDataResponse = await axios.get(`/api/versioning/file-data/${projectData.fileId}`);
        if (fileDataResponse.data.detection_data) {
          const parsed = typeof fileDataResponse.data.detection_data === "string"
            ? JSON.parse(fileDataResponse.data.detection_data)
            : fileDataResponse.data.detection_data;
          if (parsed && parsed.maps) {
            setProjectData(prev => prev ? { ...prev, detectionResults: stripSoiTag(parsed) } : prev);
          }
        }
        setMappackUnlocked(true);
        setMappackJustUnlocked(true);
        setTimeout(() => setMappackJustUnlocked(false), 1500);
        toast({ title: t.mappack?.unlocked || "Mappack Unlocked", description: t.mappack?.unlockedDescription || "You can now view all maps." });
      }
    } catch (error: any) {
      if (error.response?.status === 429) {
        toast({ title: t.mappack?.limitReached || "Daily Limit Reached", description: t.mappack?.limitReachedDescription || "You have reached the daily mappack unlock limit. Try again in 24h.", variant: "destructive" });
      } else if (error.response?.status === 403) {
        toast({ title: t.mappack?.proRequired || "Pro Required", description: t.mappack?.proRequiredDescription || "A Pro subscription is required to unlock mappacks.", variant: "destructive" });
      } else {
        toast({ title: t.common?.error || "Error", description: t.mappack?.unlockFailed || "Failed to unlock mappack.", variant: "destructive" });
      }
    } finally {
      setMappackUnlocking(false);
    }
  };

  const handleMapClick = (map: MapData) => {
    // Block map clicks when mappack is locked
    if (!mappackUnlocked) return;

    setOpenMaps((prev) => {
      // Si déjà ouverte, on la remet en haut de pile
      const exists = prev.some((m) => m.address === map.address);
      if (exists) {
        const filtered = prev.filter((m) => m.address !== map.address);
        return [...filtered, map];
      }
      // Nouvelle map: enregistrer le statut EasyView au moment de l'ouverture
      setMapEasyViewStatus((prevStatus) => {
        const updated = new Map(prevStatus);
        updated.set(map.address, easyViewMode);
        return updated;
      });
      setMapLayouts((prevLayouts) => ensureLayoutForMap(map, prevLayouts));
      return [...prev, map];
    });
    // Définir cette map comme active pour le curseur global
    setActiveMapAddress(map.address);
  };

  const handleCloseMapWindow = (mapAddress: number) => {
    setOpenMaps((prev) => prev.filter((m) => m.address !== mapAddress));
    setMapViewModes((prev) => {
      const updated = new Map(prev);
      updated.delete(mapAddress);
      return updated;
    });
    setMapEasyViewStatus((prev) => {
      const updated = new Map(prev);
      updated.delete(mapAddress);
      return updated;
    });
    setMapLayouts((prev) => {
      const next = new Map(prev);
      next.delete(mapAddress);
      return next;
    });
    // Nettoyer le store des infos de sélection
    mapSelectionInfoRef.current.delete(mapAddress);
    // Si c'était la map active, réinitialiser le curseur
    if (activeMapAddress === mapAddress) {
      setActiveMapAddress(null);
      setGlobalCursorInfo(null);
    }
  };
  
  // Handler to update viewMode pour une map donnée
  const handleViewModeChange = (mapAddress: number, mode: "text" | "2d" | "3d") => {
    setMapViewModes((prev) => {
      const updated = new Map(prev);
      updated.set(mapAddress, mode);
      return updated;
    });
  };

  const bringMapToFront = (mapAddress: number) => {
    setOpenMaps((prev) => {
      const idx = prev.findIndex((m) => m.address === mapAddress);
      if (idx === -1) return prev;
      const selected = prev[idx];
      const rest = prev.filter((_, i) => i !== idx);
      return [...rest, selected];
    });
    // S'assurer que l'hexdump est derrière les maps
    setHexdumpZIndex(10);
    // Définir cette map comme active pour le curseur global
    setActiveMapAddress(mapAddress);
    setIsHexdumpActive(false);
    // Reset modify command to prevent it from being applied to the new active map
    setModifyCommand(null);
    // Mettre à jour le curseur avec les infos de cette map
    const mapInfo = mapSelectionInfoRef.current.get(mapAddress);
    setGlobalCursorInfo(mapInfo || null);
  };

  // Search function to find value in hexdump
  const handleSearch = useCallback(() => {
    if (!projectData?.file_data || !searchConfig.value.trim()) {
      setSearchResults([]);
      setCurrentSearchIndex(-1);
      return;
    }

    const fileData = projectData.file_data;
    const results: number[] = [];

    // Parse the search value using the hexdump display format
    let searchValue: number;
    if (hexdumpFormat === 'hex') {
      searchValue = parseInt(searchConfig.value, 16);
    } else {
      searchValue = parseInt(searchConfig.value, 10);
    }

    if (isNaN(searchValue)) {
      setSearchResults([]);
      setCurrentSearchIndex(-1);
      return;
    }

    // Parse address range
    let fromAddr = 0;
    let toAddr = fileData.length - 1;

    if (searchConfig.fromAddress.trim()) {
      const parsed = parseInt(searchConfig.fromAddress, 16);
      if (!isNaN(parsed)) fromAddr = parsed;
    }
    if (searchConfig.toAddress.trim()) {
      const parsed = parseInt(searchConfig.toAddress, 16);
      if (!isNaN(parsed)) toAddr = Math.min(parsed, fileData.length - 1);
    }

    // Search based on hexdump display size (8b or 16b)
    const bytesPerValue = hexdumpSize === '8b' ? 1 : 2;
    const bytesPerRow = 16;
    // The hexdump shows 8 values per row in both modes
    // In 8b mode: values at offsets 0-7 within each 16-byte row (only first 8 bytes visible)
    // In 16b mode: values at offsets 0,2,4,6,8,10,12,14 within each 16-byte row (all 16 bytes visible)
    const valuesPerRow = 8;
    const visibleBytesPerRow = valuesPerRow * bytesPerValue; // 8 in 8b mode, 16 in 16b mode

    for (let addr = fromAddr; addr <= toAddr - bytesPerValue + 1; addr++) {
      // Check if this address is visible in the current display mode
      const offsetInRow = addr % bytesPerRow;

      if (bytesPerValue === 1) {
        // In 8b mode, only offsets 0-7 are visible in each row
        if (offsetInRow >= visibleBytesPerRow) continue;

        const value = fileData[addr];
        if (value === searchValue) {
          results.push(addr);
        }
      } else {
        // In 16b mode, only even offsets (0,2,4,6,8,10,12,14) are visible
        if (offsetInRow % 2 !== 0) continue;

        const value = hexdumpByteOrder === "hilo"
          ? ((fileData[addr] << 8) | fileData[addr + 1])
          : (fileData[addr] | (fileData[addr + 1] << 8));
        if (value === searchValue) {
          results.push(addr);
        }
      }
    }

    setSearchResults(results);
    setCurrentSearchIndex(results.length > 0 ? 0 : -1);
    setHasSearched(true); // Mark that a search has been performed

    // Scroll to first result if found
    if (results.length > 0) {
      setHexdumpScrollToAddress(results[0]);
      setHexdumpScrollKey(prev => prev + 1); // Force scroll
    }
  }, [projectData?.file_data, searchConfig, hexdumpSize, hexdumpFormat, hexdumpByteOrder]);

  // Navigate to next/previous search result
  const navigateSearchResult = useCallback((direction: 'next' | 'prev') => {
    if (searchResults.length === 0) return;

    let newIndex: number;
    if (direction === 'next') {
      newIndex = (currentSearchIndex + 1) % searchResults.length;
    } else {
      newIndex = currentSearchIndex <= 0 ? searchResults.length - 1 : currentSearchIndex - 1;
    }

    setCurrentSearchIndex(newIndex);
    setHexdumpScrollToAddress(searchResults[newIndex]);
    setHexdumpScrollKey(prev => prev + 1); // Force scroll even if same address
  }, [searchResults, currentSearchIndex]);

  // Search modal drag handlers
  const handleSearchModalDragStart = (e: React.MouseEvent) => {
    e.preventDefault();
    const rect = searchModalRef.current?.getBoundingClientRect();
    if (!rect) return;

    // Initialize position if not set yet
    const currentX = searchModalPosition?.x ?? rect.left;
    const currentY = searchModalPosition?.y ?? rect.top;

    searchModalDragState.current = {
      startX: e.clientX,
      startY: e.clientY,
      originX: currentX,
      originY: currentY,
    };

    document.addEventListener('mousemove', handleSearchModalDragMove);
    document.addEventListener('mouseup', handleSearchModalDragEnd);
  };

  const handleSearchModalDragMove = (e: MouseEvent) => {
    if (!searchModalDragState.current) return;
    const { startX, startY, originX, originY } = searchModalDragState.current;
    const dx = e.clientX - startX;
    const dy = e.clientY - startY;

    // Clamp to workspace bounds
    const workspace = workspaceRef.current;
    if (workspace) {
      const workspaceRect = workspace.getBoundingClientRect();
      const modalWidth = searchModalRef.current?.offsetWidth || 340;
      const modalHeight = searchModalRef.current?.offsetHeight || 400;

      const newX = Math.max(workspaceRect.left, Math.min(originX + dx, workspaceRect.right - modalWidth));
      const newY = Math.max(workspaceRect.top, Math.min(originY + dy, workspaceRect.bottom - modalHeight));

      setSearchModalPosition({ x: newX, y: newY });
    } else {
      setSearchModalPosition({ x: originX + dx, y: originY + dy });
    }
  };

  const handleSearchModalDragEnd = () => {
    searchModalDragState.current = null;
    document.removeEventListener('mousemove', handleSearchModalDragMove);
    document.removeEventListener('mouseup', handleSearchModalDragEnd);
  };

  const bringHexdumpToFront = () => {
    // Mettre l'hexdump au premier plan (au-dessus de toutes les maps)
    const maxMapZIndex = 20 + openMaps.length;
    setHexdumpZIndex(maxMapZIndex + 1);
    // Définir l'hexdump comme actif pour le curseur global
    setIsHexdumpActive(true);
    setActiveMapAddress(null);
    setGlobalCursorInfo(null);
  };

  // Handler pour réduire l'hexdump
  const handleCollapseHexdump = () => {
    setHexdumpCollapsed(true);
    // Si l'hexdump était actif, réinitialiser le curseur
    if (isHexdumpActive) {
      setIsHexdumpActive(false);
    }
  };

  // Handler pour déplier l'hexdump
  const handleExpandHexdump = () => {
    setHexdumpCollapsed(false);
    // Remettre l'hexdump comme actif s'il n'y a pas de map active
    if (!activeMapAddress) {
      setIsHexdumpActive(true);
    }
  };

  // Handler pour toggle la fenêtre Preview
  const handlePreviewToggle = () => {
    if (showPreviewWindow) {
      // Fermer la fenêtre
      setShowPreviewWindow(false);
      // Vider le cache des positions de caméra pour réinitialiser à la prochaine ouverture
      Object.keys(previewCameraPositions).forEach(key => {
        delete previewCameraPositions[Number(key)];
      });
    } else {
      // Ouvrir la fenêtre avec une nouvelle clé pour réinitialiser la vue
      setPreviewWindowKey(prev => prev + 1);
      // Réinitialiser le layout à sa taille/position d'origine
      setPreviewLayout({ x: 100, y: 100, width: 500, height: 400 });
      setPreviewZIndex(Math.max(hexdumpZIndex, ...Array.from(mapLayouts.values()).map(() => 50)) + 10);
      setShowPreviewWindow(true);
    }
  };

  // Handler pour amener la fenêtre Preview au premier plan
  const bringPreviewToFront = () => {
    const maxZ = Math.max(hexdumpZIndex, ...openMaps.map((_, i) => 50 + i));
    setPreviewZIndex(maxZ + 1);
  };

  // Store pour garder les infos de sélection de chaque map
  const mapSelectionInfoRef = useRef<Map<number, {
    mapName: string;
    mapAddress: number;
    dimensions: string;
    selectedCount: number;
    selectedCells: Array<{ row: number; col: number; address: number; value: number }>;
  }>>(new Map());

  // Ref pour tracker la map active (évite les re-renders du callback)
  const activeMapAddressRef = useRef<number | null>(null);
  activeMapAddressRef.current = activeMapAddress;

  // Handler pour recevoir les infos de sélection d'une map (curseur global)
  // IMPORTANT: pas de dépendances pour éviter les boucles infinies
  const handleMapSelectionChange = useCallback((mapAddress: number, info: {
    mapName: string;
    mapAddress: number;
    dimensions: string;
    selectedCount: number;
    selectedCells: Array<{ row: number; col: number; address: number; value: number }>;
  } | null) => {
    // Stocker les infos de cette map
    if (info) {
      mapSelectionInfoRef.current.set(mapAddress, info);
      // Mettre à jour le curseur si c'est la map active (utiliser le ref pour éviter les re-renders)
      if (activeMapAddressRef.current === mapAddress) {
        setGlobalCursorInfo(info);
      }
    } else {
      mapSelectionInfoRef.current.delete(mapAddress);
      if (activeMapAddressRef.current === mapAddress) {
        setGlobalCursorInfo(null);
      }
    }
  }, []);

  // Handler pour recevoir les données 3D d'une map (pour Preview window).
  // Kept identity-stable (empty deps) — MapViewer's useEffect lists this in
  // its dependency array, so any new function identity would trigger a
  // re-notification and risk an update loop. We read showPreviewWindow via
  // a ref instead.
  const showPreviewWindowRef = useRef<boolean>(false);
  useEffect(() => { showPreviewWindowRef.current = showPreviewWindow; }, [showPreviewWindow]);
  const handlePlot3DDataChange = useCallback((mapAddress: number, data: {
    plot3DData: any[];
    xAxisLabels: string[];
    yAxisLabels: string[];
    canShow3D: boolean;
  }) => {
    mapPlot3DDataRef.current.set(mapAddress, data);
    if (activeMapAddressRef.current === mapAddress && showPreviewWindowRef.current) {
      setPreviewDataVersion(prev => prev + 1);
    }
  }, []);

  // Group maps by category from backend
  // VCDS Diagnostic maps use subcategory for grouping
  // "limp" (limp-home / recovery) maps are detected but never shown in the map
  // tree nor exported — they are backup limiters, not tuning targets.
  const isLimpMap = (name?: string | null) =>
    (name || "").toLowerCase().includes("(limp)");

  const groupedMaps = projectData?.detectionResults?.maps?.reduce((acc, map) => {
    if (isLimpMap(map.name)) return acc;
    // Insensible à la casse : les anciens projets stockent « VCDS Diagnostic »,
    // le détecteur écrit désormais « VCDS diagnostic » (D minuscule).
    const folder = (map.subcategory || "").toLowerCase() === "vcds diagnostic"
      ? "VCDS diagnostic"
      : (map.category || "Other");
    if (!acc[folder]) {
      acc[folder] = [];
    }
    acc[folder].push(map);
    return acc;
  }, {} as Record<string, MapData[]>) || {};

  // Sort maps within each folder: by address (default) or by name,
  // according to the user's toggle. Address is always the tiebreaker.
  const sortedGroupedMaps = Object.entries(groupedMaps).reduce((acc, [folder, maps]) => {
    acc[folder] = [...maps].sort((a, b) => {
      if (mapSortMode === "name" || mapSortMode === "name-desc") {
        const byName = (a.name || "").localeCompare(b.name || "");
        if (byName !== 0) return mapSortMode === "name-desc" ? -byName : byName;
      }
      return (a.address || 0) - (b.address || 0);
    });
    return acc;
  }, {} as Record<string, MapData[]>);

  // Sort folder names alphabetically A-Z, "Other" always last
  const sortedFolders = Object.keys(sortedGroupedMaps).sort((a, b) => {
    if (a === "Other") return 1;
    if (b === "Other") return -1;
    return a.localeCompare(b);
  });

  const currentVersionName =
    versions.find((v) => v.id === currentVersionId)?.name ||
    (versions[0]?.name ?? "Ori");
  // La version Ori n'est pas renommable — le bouton RENAME est grisé dessus
  const isOriVersion = currentVersionName === "Ori";

  // Animate MapPack folder opening when project loads
  useEffect(() => {
    if (projectData && sortedFolders.length > 0 && !hasAnimatedMappack) {
      // Small delay to allow the UI to render first
      const timer = setTimeout(() => {
        setExpandedFolders(new Set(["all"]));
        setHasAnimatedMappack(true);
      }, 300);
      return () => clearTimeout(timer);
    }
  }, [projectData, sortedFolders.length, hasAnimatedMappack]);


  // Show loading state while projectData or file_data is being fetched
  if (!projectData || !projectData.file_data || projectData.file_data.length === 0) {
    return (
      <div className="flex items-center justify-center min-h-screen" style={{ background: getBackgroundColor() }}>
        <div className="flex flex-col items-center gap-4">
          <div className={`loader loader-lg${theme === 'light' ? ' loader-light' : ''}`} />
          <p className={theme === 'light' ? 'text-slate-500' : 'text-slate-400'}>{t.common.loading}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-screen overflow-hidden relative" style={{ background: getBackgroundColor() }}>
      {/* Ambient glassmorphism halos — the editor's native wallpaper */}
      {editorWallpaper === 'editor' && (
        <div aria-hidden className="absolute inset-0 z-0 pointer-events-none overflow-hidden" style={{ opacity: theme === 'light' ? 0.14 : 0.65 }}>
          <div className="absolute rounded-full" style={{ width: 520, height: 520, left: -120, top: -140, filter: 'blur(90px)', background: 'radial-gradient(circle, #ef444488, transparent 70%)' }} />
          <div className="absolute rounded-full" style={{ width: 620, height: 620, right: -160, top: 120, filter: 'blur(90px)', background: 'radial-gradient(circle, #2563eb77, transparent 70%)' }} />
          <div className="absolute rounded-full" style={{ width: 480, height: 480, left: '32%', bottom: -200, filter: 'blur(90px)', background: 'radial-gradient(circle, #7c3aed66, transparent 70%)' }} />
        </div>
      )}
      {/* Traits animés clairs : canvas inversé, couleurs pré-inversées */}
      {editorWallpaper === 'lines-light' && (
        <div aria-hidden className="absolute inset-0 z-0 pointer-events-none overflow-hidden" style={{ opacity: 0.45, filter: 'invert(1)' }}>
          <FloatingLines
            linesGradient={['#046dc3', '#078e8e', '#7e7307']}
            enabledWaves={['top', 'middle', 'bottom']}
            lineCount={[6, 8, 6]}
            lineDistance={[5, 5, 6]}
            animationSpeed={0.8}
            interactive={false}
            parallax={false}
            parallaxStrength={0.15}
          />
        </div>
      )}
      {/* Fond « traits animés » (option manuelle) */}
      {editorWallpaper === 'lines' && (
        <div aria-hidden className="absolute inset-0 z-0 pointer-events-none overflow-hidden" style={{ opacity: 0.45 }}>
          <FloatingLines
            linesGradient={['#9a3412', '#7f1d1d', '#312e81']}
            enabledWaves={['top', 'middle', 'bottom']}
            lineCount={[6, 8, 6]}
            lineDistance={[5, 5, 6]}
            animationSpeed={0.8}
            interactive={false}
            parallax={false}
            parallaxStrength={0.15}
          />
        </div>
      )}
      {/* Image personnalisée de l'utilisateur : plein écran en cover, léger
          voile suivant le thème pour garder fenêtres et panneaux lisibles */}
      {editorWallpaper === 'custom' && customEditorWallpaper && (
        <>
          <div
            aria-hidden
            className="absolute inset-0 z-0 pointer-events-none"
            style={{
              backgroundImage: `url(${customEditorWallpaper})`,
              backgroundSize: 'cover',
              backgroundPosition: 'center',
            }}
          />
          <div
            aria-hidden
            className="absolute inset-0 z-0 pointer-events-none"
            style={{ backgroundColor: theme === 'light' ? 'rgba(255,255,255,0.22)' : 'rgba(0,0,0,0.35)' }}
          />
        </>
      )}
      {/* Film grain for glass tactility (off on pure black) */}
      {editorWallpaper !== 'black' && (
        <div aria-hidden className="absolute inset-0 z-0 pointer-events-none" style={{
          opacity: theme === 'light' ? 0.025 : 0.05,
          backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='140' height='140'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2'/%3E%3C/filter%3E%3Crect width='140' height='140' filter='url(%23n)' opacity='0.55'/%3E%3C/svg%3E")`
        }} />
      )}
      {/* Left Sidebar - Project Info & Maps Tree - FULL HEIGHT */}
      <div
        ref={sidebarRef}
        className="flex flex-col h-screen relative"
        style={{
          width: `${sidebarWidth}px`,
          minWidth: '338px',
          maxWidth: '500px',
          background: getSidebarBg(),
          backdropFilter: getGlassBlur(),
          WebkitBackdropFilter: getGlassBlur(),
          borderRight: `1px solid ${getBorderColor()}`,
          zIndex: 1
        }}
      >
        {/* Logo & Project Info — zone de déplacement de la fenêtre, comme la
            barre d'outils : cliquer-glisser ici déplace l'application
            (les champs interactifs plus bas gardent leurs propres clics). */}
        <div data-tauri-drag-region className="p-4 flex-shrink-0" style={{ borderBottom: `1px solid ${getBorderColor()}` }}>
          <div data-tauri-drag-region className="mb-4 relative inline-block">
  <div data-tauri-drag-region className="text-xl font-bold">
    <span className="bg-gradient-to-r from-red-600 via-red-500 to-orange-500 bg-clip-text text-transparent">Zed</span><span style={{ color: getTextColor() }}>Suite</span>
  </div>
</div>

          <div data-tauri-drag-region className="space-y-2 text-sm">
            <div>
              <div className="text-xs" style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)' }}>{t.sidebar.project}</div>
              <div className="relative" onMouseEnter={() => showTooltip('project')} onMouseLeave={hideTooltip}>
                <div className="font-medium truncate" style={{ color: getTextColor() }}>
                  {projectData.project_name}
                </div>
                {tooltipVisible === 'project' && (
                  <div className="pointer-events-none absolute left-0 top-full mt-1 z-[9999] rounded px-2 py-1 text-xs font-normal shadow-lg whitespace-nowrap"
                    style={{ backgroundColor: theme === 'light' ? '#1e1e1e' : '#e5e5e5', color: theme === 'light' ? '#fff' : '#000' }}>
                    {projectData.project_name}
                  </div>
                )}
              </div>
            </div>

            <div>
              <div className="text-xs" style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)' }}>{t.sidebar.ecuType}</div>
              <div className="truncate" style={{ color: getTextColor() }}>
                {formatEcuWithManufacturer(projectData.ecu_type) || t.sidebar.unknown}
              </div>
            </div>

            <div className="relative" onMouseEnter={() => showTooltip('version')} onMouseLeave={hideTooltip}>
              <div className="text-xs mb-1" style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)' }}>{t.sidebar.version}</div>
              <div className="flex items-center gap-3">
                <button
                  onClick={() => setShowVersionDropdown(!showVersionDropdown)}
                  className={`flex-1 flex items-center gap-2 px-2 py-1 rounded-lg overflow-hidden ${getButtonHoverClass()} transition-colors`}
                  style={{ backgroundColor: getGlassSurface(), border: `1px solid ${getBorderColor()}` }}
                >
                    <span className="text-sm truncate" style={{ color: getTextColor() }}>
                      {loadingVersions ? t.sidebar.loading : currentVersionName}
                    </span>
                  <ChevronDownIcon className="w-3 h-3 flex-shrink-0 ml-auto" style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.7)' : 'rgba(255, 255, 255, 0.7)' }} />
                </button>
                <button
                  onClick={() => setShowProjectInfoModal(true)}
                  className={`p-1 rounded ${getButtonHoverClass()} transition-colors`}
                  title={t.sidebar.projectInfo}
                  style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.7)' : 'rgba(255, 255, 255, 0.7)' }}
                >
                  <MoreVertical className="w-4 h-4" />
                </button>
              </div>
              
              {showVersionDropdown && (
                <div
                  className="absolute top-full left-0 right-0 mt-1 border rounded-lg shadow-lg z-50 overflow-hidden"
                  style={{
                    backgroundColor: theme === 'light' ? 'rgba(255,255,255,0.92)' : theme === 'oled' ? 'rgba(16,16,19,0.96)' : 'rgba(24,27,37,0.92)',
                    backdropFilter: 'blur(14px)',
                    WebkitBackdropFilter: 'blur(14px)',
                    borderColor: getBorderColor()
                  }}
                >
                    <div className="max-h-48 overflow-y-auto">
                      {versions.map((version) => (
                        <button
                          key={version.id}
                          onClick={() => handleSelectVersion(version.id)}
                          className={`w-full px-2 py-1.5 text-left text-sm ${getButtonHoverClass()} transition-colors flex items-center gap-2`}
                          style={{
                            color: getTextColor(),
                            backgroundColor: version.id === currentVersionId
                              ? (theme === 'light' ? 'rgba(0, 0, 0, 0.1)' : 'rgba(255, 255, 255, 0.1)')
                              : undefined
                          }}
                        >
                          <span>{version.name}</span>
                          {version.isCurrent && (
                            <span
                              className="text-[10px] px-1 rounded"
                              style={{
                                color: getTextColor(),
                                backgroundColor: theme === 'light' ? 'rgba(0, 0, 0, 0.15)' : 'rgba(255, 255, 255, 0.15)'
                              }}
                            >
                              {t.sidebar.current}
                            </span>
                          )}
                        </button>
                      ))}
                    </div>
                    <div
                      className="border-t"
                      style={{ borderColor: theme === 'light' ? 'rgba(0, 0, 0, 0.1)' : 'rgba(255, 255, 255, 0.1)' }}
                    >
                      <button
                        onClick={() => {
                          setShowVersionDropdown(false);
                          handleCreateVersion();
                        }}
                        className={`w-full px-2 py-1.5 text-left text-sm ${getButtonHoverClass()} transition-colors`}
                        style={{ color: getTextColor() }}
                      >
                        {t.sidebar.newVersion}
                      </button>
                    </div>
                </div>
              )}
              {tooltipVisible === 'version' && (
                <div className="pointer-events-none absolute left-0 top-full mt-1 z-[9999] rounded px-2 py-1 text-xs font-normal shadow-lg whitespace-nowrap"
                  style={{ backgroundColor: theme === 'light' ? '#1e1e1e' : '#e5e5e5', color: theme === 'light' ? '#fff' : '#000' }}>
                  {loadingVersions ? t.sidebar.loading : currentVersionName}
                </div>
              )}
            </div>

            {/* État du checksum — surveillé en continu, recalcul manuel à droite */}
            <div className="flex items-center justify-between mt-2">
              <div className="flex items-center gap-1.5 min-w-0">
                <span className="text-xs truncate" style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.55)' : 'rgba(255, 255, 255, 0.55)' }}>
                  {checksumStatus === 'ok' ? t.checksum.statusOk
                    : checksumStatus === 'bad' ? t.checksum.statusInvalid
                    : checksumStatus === 'checking' ? t.checksum.statusChecking
                    : t.checksum.statusUnsupported}
                </span>
                <span className={`w-2 h-2 rounded-full flex-shrink-0 ${
                  checksumStatus === 'ok' ? 'bg-emerald-500'
                    : checksumStatus === 'bad' ? 'bg-red-500'
                    : checksumStatus === 'checking' ? 'bg-slate-400 animate-pulse'
                    : 'bg-slate-500'
                }`} />
              </div>
              {isChecksumSupported(projectData?.ecu_type) && (
                <button
                  onClick={handleRecalcChecksum}
                  disabled={loadingAction !== null}
                  className={`p-1 rounded flex-shrink-0 ${getButtonHoverClass()} transition-colors disabled:opacity-40`}
                  title={t.checksum.recalc}
                  style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.7)' : 'rgba(255, 255, 255, 0.7)' }}
                >
                  <RefreshCw className="w-3.5 h-3.5" />
                </button>
              )}
            </div>
          </div>
        </div>

        {/* Action Buttons - 5 BUTTONS ON ONE LINE WITH ICON + TEXT */}
        <div className="px-1 py-2 flex-shrink-0" style={{ borderBottom: `1px solid ${getBorderColor()}` }}>
          <div className="flex gap-0.5">
            <Button
              variant="ghost"
              size="sm"
              className={`flex-1 h-10 flex flex-col items-center justify-center gap-0.5 p-0 ${getButtonHoverClass()}`}
              onClick={handleSave}
              disabled={loadingAction !== null || !hasUnsavedChanges}
              style={{ color: getTextColor() }}
            >
              <Save className="w-3 h-3" />
              <span className="text-[8px] leading-tight">{t.sidebar.save}</span>
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className={`flex-1 h-10 flex flex-col items-center justify-center gap-0.5 p-0 ${getButtonHoverClass()}`}
              onClick={handleImport}
              disabled={loadingAction !== null}
              style={{ color: getTextColor() }}
            >
              <Upload className="w-3 h-3" />
              <span className="text-[8px] leading-tight">{t.sidebar.import}</span>
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className={`flex-1 h-10 flex flex-col items-center justify-center gap-0.5 p-0 ${getButtonHoverClass()}`}
              onClick={handleExport}
              disabled={loadingAction !== null}
              style={{ color: getTextColor() }}
            >
              <Download className="w-3 h-3" />
              <span className="text-[8px] leading-tight">{t.sidebar.export}</span>
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className={`flex-1 h-10 flex flex-col items-center justify-center gap-0.5 p-0 ${getButtonHoverClass()}`}
              onClick={handleRenameVersion}
              disabled={loadingAction !== null || isOriVersion}
              style={{ color: getTextColor() }}
            >
              <Edit className="w-3 h-3" />
              <span className="text-[8px] leading-tight">{t.sidebar.rename}</span>
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className={`flex-1 h-10 flex flex-col items-center justify-center gap-0.5 p-0 hover:bg-red-500/20 text-red-400 ${
                theme === 'light' ? 'hover:text-black' : ''
              }`}
              onClick={handleDelete}
              // La version Ori ne peut pas être supprimée : bouton grisé,
              // comme SAVE/RENAME (le disabled de <Button> applique opacity-50)
              disabled={loadingAction !== null || isOriVersion}
            >
              <Trash2 className="w-3 h-3" />
              <span className="text-[8px] leading-tight">{t.sidebar.delete}</span>
            </Button>
          </div>
        </div>

        {/* Boutons Solutions / DTC codes - POSITION FIXE */}
        <div className="p-2 flex-shrink-0" style={{ borderBottom: `1px solid ${getBorderColor()}` }}>
          <div className="grid grid-cols-2 gap-2">
            {/* Bouton Solutions — toujours présent : sur un calculateur sans
                solution (EDC16…), la fenêtre affiche « aucune solution
                disponible » plutôt que de masquer le bouton */}
            <button
              onClick={() => {
                if (!mappackUnlocked) return;
                setIsSolutionsOpen(true);
              }}
              disabled={!mappackUnlocked}
              className={`relative overflow-hidden rounded-xl backdrop-blur-md border px-4 py-2 flex items-center justify-center gap-2.5 transition-all duration-500 group ${
                !mappackUnlocked
                  ? 'opacity-40 cursor-not-allowed'
                  : 'hover:scale-105 hover:shadow-2xl'
              } ${
                theme === 'light'
                  ? 'bg-gradient-to-l from-red-600 via-red-500 to-orange-500 border-black/10 hover:shadow-red-600/35'
                  : 'bg-gradient-to-l from-red-600/90 via-red-500/90 to-orange-500/90 border-white/15 hover:shadow-red-500/35'
              }`}
            >
              {mappackUnlocked && <div className="absolute inset-0 bg-gradient-to-l from-transparent via-white/20 to-transparent translate-x-full group-hover:-translate-x-full transition-transform duration-1000" />}
              {/* Même icône que la version web */}
              <PiHeadCircuit className="w-5 h-5 transition-colors duration-300" style={{ color: '#ffffff' }} />
              <span className="font-medium text-sm" style={{ color: '#ffffff' }}>{t.sidebar.solutions}</span>
            </button>

            {/* Bouton DTC codes - Dégradé ambre */}
            <button
              onClick={() => {
                if (!mappackUnlocked) return;
                setIsDTCOpen(true);
              }}
              disabled={!mappackUnlocked}
              className={`relative overflow-hidden rounded-xl backdrop-blur-md border px-4 py-2 flex items-center justify-center gap-2.5 transition-all duration-500 group ${
                !mappackUnlocked
                  ? 'opacity-40 cursor-not-allowed'
                  : 'hover:scale-105 hover:shadow-2xl'
              } ${
                theme === 'light'
                  ? 'bg-gradient-to-l from-amber-500 via-yellow-500 to-amber-400 border-black/10 hover:shadow-amber-600/35'
                  : 'bg-gradient-to-l from-amber-500/90 via-yellow-500/90 to-amber-400/90 border-white/15 hover:shadow-amber-500/35'
              }`}
            >
              {/* Effet brillant inversé */}
              {mappackUnlocked && <div className="absolute inset-0 bg-gradient-to-l from-transparent via-white/20 to-transparent translate-x-full group-hover:-translate-x-full transition-transform duration-1000" />}

              {/* Icône triangle — blanche sur le dégradé plein, comme l'icône du menu DTC */}
              <AlertTriangle className="w-5 h-5 transition-colors duration-300" style={{ color: '#ffffff' }} />
              <span className="font-medium text-sm" style={{ color: '#ffffff' }}>{t.sidebar.dtcCodes}</span>
            </button>
          </div>
        </div>

        {/* Maps Tree - SCROLLABLE */}
        <div className={`flex-1 min-h-0 overflow-y-auto p-2 maps-sidebar-scroll ${theme === 'light' ? 'light-theme' : ''}`}>
          <div className="space-y-1">
            {/* Hexdump — rouvre la fenêtre hexdump quand elle a été fermée */}
            <div>
              <button
                onClick={() => {
                  handleExpandHexdump();
                  bringHexdumpToFront();
                }}
                className={`flex items-center gap-2 w-full min-w-0 px-2 py-1.5 rounded ${theme === 'light' ? 'hover:bg-black/5' : 'hover:bg-white/5'} transition-colors group`}
                title="Ouvrir le hexdump"
              >
                {/* Point d'état lié au checksum — même couleur que la ligne Checksum */}
                <span className="w-4 h-4 flex-shrink-0 flex items-center justify-center">
                  <span className={`w-1.5 h-1.5 rounded-full transition-colors ${
                    checksumStatus === 'ok' ? 'bg-emerald-500'
                      : checksumStatus === 'bad' ? 'bg-red-500'
                      : checksumStatus === 'checking' ? 'bg-slate-400 animate-pulse'
                      : 'bg-slate-500'
                  }`} />
                </span>
                <Cpu className={`w-4 h-4 flex-shrink-0 transition-colors ${hexdumpCollapsed ? 'text-slate-500' : 'text-yellow-500'}`} />
                <span className="text-sm" style={{ color: theme === 'light' ? '#000000' : 'rgba(255, 255, 255, 0.7)' }}>Hexdump</span>
              </button>
            </div>

            {/* User maps Folder - ne change jamais de couleur */}
            <div>
              <div className="flex items-center gap-1">
                <button
                  onClick={() => toggleFolder("all")}
                  className={`flex items-center gap-2 flex-1 min-w-0 px-2 py-1.5 rounded ${theme === 'light' ? 'hover:bg-black/5' : 'hover:bg-white/5'} transition-colors group`}
                >
                  <ChevronRight className={`w-4 h-4 transition-transform duration-200 ${expandedFolders.has("all") ? "rotate-90" : ""}`} style={{ color: getTextColor() }} />
                  {expandedFolders.has("all") ? (
                    <FolderOpen className="w-4 h-4 text-yellow-500 transition-colors" />
                  ) : (
                    <Folder className="w-4 h-4 text-yellow-500 transition-colors" />
                  )}
                  <span className="text-sm" style={{ color: theme === 'light' ? '#000000' : 'rgba(255, 255, 255, 0.7)' }}>{t.sidebar.mappack}</span>
                </button>
                {/* État du mappack : % de règles de complétude satisfaites
                    (EDC16). Vert = tout est là, orange = maps manquantes.
                    Clic → fenêtre d'explication détaillée. */}
                {mappackConfidence !== null && (
                  <button
                    onClick={() => setShowMappackHealth(true)}
                    title={t.mappackHealth.title}
                    className={`flex-shrink-0 px-1.5 h-5 flex items-center rounded-full text-[10px] font-semibold tabular-nums transition-colors ${
                      missingExpected.length === 0
                        ? 'bg-emerald-500/15 text-emerald-400 hover:bg-emerald-500/30'
                        : 'bg-orange-500/15 text-orange-400 hover:bg-orange-500/30'
                    }`}
                  >
                    {mappackConfidence}%
                  </button>
                )}
                {/* Cycle du tri des maps dans les dossiers : adresse → nom A→Z
                    → nom Z→A. L'icône montre le mode ACTIF ; le title annonce
                    le mode suivant. */}
                {mappackUnlocked && (
                  <button
                    onClick={toggleMapSortMode}
                    title={
                      mapSortMode === "address"
                        ? t.sidebar.sortByName
                        : mapSortMode === "name"
                          ? t.sidebar.sortByNameDesc
                          : t.sidebar.sortByAddress
                    }
                    className={`flex-shrink-0 h-6 w-6 flex items-center justify-center rounded-md transition-colors ${
                      theme === 'light' ? 'hover:bg-black/10' : 'hover:bg-white/10'
                    }`}
                    style={{ color: theme === 'light' ? 'rgba(0,0,0,0.55)' : 'rgba(255,255,255,0.55)' }}
                  >
                    {mapSortMode === "address" ? (
                      <ArrowDown01 className="w-4 h-4" />
                    ) : mapSortMode === "name" ? (
                      <ArrowDownAZ className="w-4 h-4" />
                    ) : (
                      <ArrowDownZA className="w-4 h-4" />
                    )}
                  </button>
                )}
                {/* Export mappack (WinOLS JSON) - contextual to the Mappack tree.
                    Collapsed to the icon; the label slides out on hover, and
                    for 2s when the mappack gets unlocked (discovery hint). */}
                {mappackUnlocked && (
                  <button
                    onClick={handleExportMappack}
                    title={t.toolbar.exportMappack}
                    className={`group flex-shrink-0 flex items-center px-2 py-0.5 mr-1 rounded-md bg-gradient-to-r from-red-600 via-red-500 to-orange-500 text-xs font-medium ${theme === 'light' ? 'text-black' : 'text-white'}`}
                  >
                    <FileJson className={`w-3.5 h-3.5 flex-shrink-0 ${theme === 'light' ? 'text-black' : 'text-white'}`} />
                    <span
                      className={`overflow-hidden whitespace-nowrap transition-all duration-300 group-hover:max-w-[80px] group-hover:opacity-100 group-hover:ml-1.5 ${
                        mappackExportHint ? 'max-w-[80px] opacity-100 ml-1.5' : 'max-w-0 opacity-0 ml-0'
                      }`}
                    >
                      {t.sidebar.exportMappackShort}
                    </span>
                  </button>
                )}
              </div>

              {/* Unlock Mappack Button - shown when locked */}
              {!mappackUnlocked && expandedFolders.has("all") && (
                <div className="ml-6 mt-2 mb-2">
                  <button
                    onClick={handleUnlockMappack}
                    disabled={mappackUnlocking}
                    className={`w-full relative overflow-hidden rounded-xl backdrop-blur-md border px-4 py-2.5 flex items-center justify-center gap-2.5 transition-all duration-500 ${
                      mappackUnlocking ? 'opacity-60 cursor-wait' : 'hover:scale-[1.02] hover:shadow-xl'
                    } ${
                      theme === 'light'
                        ? 'bg-gradient-to-r from-red-600/90 via-red-500/90 to-orange-500/90 border-red-500/50 hover:shadow-red-500/30'
                        : 'bg-gradient-to-r from-red-600/90 via-red-500/90 to-orange-500/90 border-white/10 hover:shadow-red-500/30'
                    }`}
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ color: theme === 'light' ? '#000' : '#fff' }}>
                        <rect width="18" height="11" x="3" y="11" rx="2" ry="2"/>
                        <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
                      </svg>
                    <span className="font-medium text-sm" style={{ color: theme === 'light' ? '#000' : '#fff' }}>
                      {mappackUnlocking ? (t.mappack?.unlocking || "Unlocking...") : (t.mappack?.unlock || "Unlock Mappack")}
                    </span>
                  </button>
                </div>
              )}

              <div className={`overflow-hidden transition-all duration-300 ease-in-out ${
                expandedFolders.has("all") ? "max-h-[20000px] opacity-100" : "max-h-0 opacity-0"
              }`}>
                <div className={`ml-1 mt-1 space-y-1 ${mappackJustUnlocked ? 'mappack-unlock-anim' : ''}`}>
                  {sortedFolders.map((folder) => {
                    const maps = sortedGroupedMaps[folder] || [];
                    // Vérifier si au moins une map dans ce dossier a été modifiée
                    // (cells modifications OR axis-label edits)
                    // Nombre de cartes modifiées du dossier : affiché « X / Y »
                    // dans le compteur (X en dégradé rouge), Y seul si aucune
                    const folderModifiedCount = mappackUnlocked
                      ? maps.filter(
                          map => allMapModifications.has(map.address) || mapAxisLabels.has(map.address)
                        ).length
                      : 0;
                    const folderHasModifiedMap = folderModifiedCount > 0;
                    const folderTextColor = !mappackUnlocked
                      ? (theme === 'light' ? 'rgba(0, 0, 0, 0.4)' : 'rgba(255, 255, 255, 0.4)')
                      : (theme === 'light' ? '#000000' : 'rgba(255, 255, 255, 0.7)');
                    // Le compteur garde toujours sa couleur normale
                    const folderCountColor = theme === 'light' ? 'rgba(0, 0, 0, 0.4)' : 'rgba(255, 255, 255, 0.4)';
                    return (
                      <div key={folder}>
                        <button
                          onClick={() => toggleFolder(folder)}
                          className={`flex items-center gap-2 w-full px-2 py-1.5 rounded transition-colors group ${
                            !mappackUnlocked
                              ? 'cursor-not-allowed opacity-60'
                              : (theme === 'light' ? 'hover:bg-black/5' : 'hover:bg-white/5')
                          }`}
                        >
                          {mappackUnlocked ? (
                            <ChevronRight className={`w-3 h-3 transition-transform duration-200 ${expandedFolders.has(folder) ? "rotate-90" : ""}`} style={{ color: getTextColor() }} />
                          ) : (
                            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ color: theme === 'light' ? 'rgba(0,0,0,0.3)' : 'rgba(255,255,255,0.3)' }}>
                              <rect width="18" height="11" x="3" y="11" rx="2" ry="2"/>
                              <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
                            </svg>
                          )}
                          {expandedFolders.has(folder) ? (
                            <FolderOpen className="w-3 h-3 text-yellow-500 transition-colors" />
                          ) : (
                            <Folder className="w-3 h-3 text-yellow-500 transition-colors" />
                          )}
                          <span
                            className={`text-xs ${folderHasModifiedMap ? 'bg-gradient-to-r from-red-600 via-red-500 to-orange-500 bg-clip-text text-transparent' : ''}`}
                            style={folderHasModifiedMap ? undefined : { color: folderTextColor }}
                          >{folder}</span>
                          <span
                            className="text-[10px] ml-auto px-1.5 rounded-full border tabular-nums text-center"
                            style={{
                              color: folderCountColor,
                              backgroundColor: theme === 'light' ? 'rgba(255,255,255,0.7)' : 'rgba(255,255,255,0.045)',
                              borderColor: getBorderColor(),
                              minWidth: '26px'
                            }}
                          >
                            {folderHasModifiedMap && (
                              <>
                                <span className="bg-gradient-to-r from-red-600 via-red-500 to-orange-500 bg-clip-text text-transparent font-semibold">
                                  {folderModifiedCount}
                                </span>
                                <span className="mx-0.5 opacity-60">/</span>
                              </>
                            )}
                            {maps.length}
                          </span>
                        </button>

                        <div className={`overflow-hidden transition-all duration-300 ease-in-out ${
                          expandedFolders.has(folder) ? "max-h-[20000px] opacity-100" : "max-h-0 opacity-0"
                        }`}>
                          <div className="ml-1 mt-1 space-y-0.5">
                            {maps.map((map, index) => {
                              const isOpen = openMaps.some((openMap) => openMap.address === map.address);
                              const isModified = allMapModifications.has(map.address) || mapAxisLabels.has(map.address);
                              // Use codeblock_id directly from backend
                              const edcsuiteCodeblockId = map.codeblock_id || null;

                              // Texte : dégradé du logo si modifié (bg-clip-text), sinon normal.
                              // L'icône (SVG currentColor) ne peut pas prendre le dégradé -> rouge uni.
                              const modifiedGradient = 'bg-gradient-to-r from-red-600 via-red-500 to-orange-500 bg-clip-text text-transparent';
                              const textColor = isModified
                                ? '#ef4444' // icône + fallback sans adresse
                                : (theme === 'light' ? 'rgba(0, 0, 0, 0.7)' : 'rgba(255, 255, 255, 0.7)');
                              const textColorStrong = theme === 'light' ? 'rgba(0, 0, 0, 0.9)' : 'rgba(255, 255, 255, 0.9)';
                              const textColorMuted = theme === 'light' ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.5)';

                              return (
                                <button
                                  key={index}
                                  onClick={() => handleMapClick(map)}
                                  onContextMenu={(e) => {
                                    e.preventDefault();
                                    setMapTreeContextMenu({ x: e.clientX, y: e.clientY, map });
                                  }}
                                  className={`flex items-center gap-2 w-full px-2 py-1.5 rounded text-left transition-colors focus:outline-none ${
                                    isOpen
                                      ? "bg-primary/20"
                                      : (theme === 'light' ? 'hover:bg-black/5' : 'hover:bg-white/5')
                                  }`}
                                  style={{ color: textColor }}
                                >
                                  <FileText className="w-3 h-3 flex-shrink-0" />
                                  <span className="text-xs truncate flex-1">
                                    {map.address ? (
                                      <>
                                        <span className={`font-mono ${isModified ? modifiedGradient : ''}`} style={isModified ? undefined : { color: getTextColor() }}>{map.address.toString(16).toUpperCase()}</span>
                                        <span className={`mx-1 ${isModified ? modifiedGradient : ''}`} style={isModified ? undefined : { color: textColor }}>-</span>
                                        <span className={isModified ? modifiedGradient : ''} style={isModified ? undefined : { color: textColorStrong }}>{map.name || `Map ${index + 1}`}</span>
                                        {edcsuiteCodeblockId !== null && (
                                          <span className={`ml-1 ${isModified ? modifiedGradient : ''}`} style={isModified ? undefined : { color: textColorMuted }}>[codeblock {edcsuiteCodeblockId}]</span>
                                        )}
                                      </>
                                    ) : (
                                      map.name || `Map ${index + 1}`
                                    )}
                                  </span>
                                  {/* Map dimensions - aligned right like folder count */}
                                  {map.dimensions && (
                                    <span className="text-xs font-mono flex-shrink-0" style={{ color: theme === 'light' ? 'rgba(0, 0, 0, 0.4)' : 'rgba(255, 255, 255, 0.4)' }}>
                                      {map.dimensions.TwoDimensional
                                        ? `${map.dimensions.TwoDimensional.cols}x${map.dimensions.TwoDimensional.rows}`
                                        : map.dimensions.OneDimensional
                                          ? `${map.dimensions.OneDimensional.length}x1`
                                          : ''}
                                    </span>
                                  )}
                                </button>
                              );
                            })}
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Resize Handle */}
        <div
          onMouseDown={handleMouseDown}
          className="absolute top-0 right-0 w-1 h-full cursor-ew-resize hover:bg-primary/50 transition-colors"
          style={{ background: 'transparent' }}
        />
      </div>

      {/* Main Content Area - STARTS AFTER SIDEBAR (z-1: above the ambient halos) */}
      <div className="flex-1 flex flex-col h-screen overflow-hidden min-w-0 relative z-[1]">
        {/* Top Toolbar - Fixed width, no overflow */}
        <div className="flex-shrink-0 w-full min-w-0 overflow-hidden">
          <EditorToolbar
            projectName={projectData.project_name}
            hexdumpSize={hexdumpSize}
            hexdumpFormat={hexdumpFormat}
            easyViewMode={easyViewMode}
            previewOpen={showPreviewWindow}
            onHexdumpSizeChange={setHexdumpSize}
            hexdumpByteOrder={hexdumpByteOrder}
            onHexdumpByteOrderChange={setHexdumpByteOrder}
            onHexdumpFormatChange={setHexdumpFormat}
            onEasyViewModeChange={setEasyViewMode}
            onPreviewClick={handlePreviewToggle}
            onSettingsClick={() => setIsSettingsOpen(true)}
            zoomPercent={editorZoom}
            onZoomIn={() => changeEditorZoom(10)}
            onZoomOut={() => changeEditorZoom(-10)}
            onCloseProject={handleCloseProject}
            hasActiveMap={activeMapAddress !== null}
            onModifyApply={handleModifyApply}
            modifyValue={modifyValue}
            onModifyValueChange={setModifyValue}
            onCompareClick={() => setIsCompareOpen(true)}
          />
        </div>

        {/* Workspace en fenêtres (style WinOLS) */}
        <div className="flex-1 flex overflow-auto min-h-0 min-w-0" style={{
          marginTop: '-1px', zIndex: 5, borderTop: `1px solid ${getBorderColor()}`,
          // Longhand-only background (mixing the `background` shorthand with
          // backgroundImage/backgroundSize trips React's style diffing).
          // Quadrillage de points derrière les fenêtres : il fait partie du
          // fond d'écran par défaut de l'éditeur (les halos) et ne doit donc
          // pas se superposer aux autres fonds (traits animés, blanc, noir).
          backgroundColor: getWorkspaceBg(),
          backgroundImage: editorWallpaper === 'editor'
            ? 'radial-gradient(rgba(255,255,255,0.05) 1px, transparent 1px)'
            : undefined,
          backgroundSize: editorWallpaper === 'editor' ? '26px 26px' : undefined
        }}>
          <div className="flex-1 overflow-hidden relative min-h-0" ref={workspaceRef}>
            {/* Settings Menu Overlay */}
            <SettingsMenu
              isOpen={isSettingsOpen}
              onClose={handleCloseSettings}
              onSuccess={(message) => showInlineNotification(message)}
              isClosing={isClosingSettings}
            />

            {/* Search Modal */}
            {showSearchModal && (() => {
              const L = theme === 'light';
              return (
              <div
                ref={searchModalRef}
                className="absolute z-[9999] rounded-lg shadow-2xl select-none"
                style={{
                  top: searchModalPosition ? searchModalPosition.y : '50%',
                  left: searchModalPosition ? searchModalPosition.x : '50%',
                  transform: searchModalPosition ? 'none' : 'translate(-50%, -50%)',
                  background: L ? 'rgba(255, 255, 255, 0.94)' : 'rgba(0, 0, 0, 0.92)',
                  border: L ? '1px solid rgba(0, 0, 0, 0.15)' : '1px solid rgba(128, 128, 128, 0.5)',
                  minWidth: '340px',
                }}
              >
                {/* Header - Draggable */}
                <div
                  className="px-4 py-3 flex items-center justify-between cursor-move"
                  style={{ borderBottom: L ? '1px solid rgba(0, 0, 0, 0.12)' : '1px solid rgba(128, 128, 128, 0.3)' }}
                  onMouseDown={handleSearchModalDragStart}
                >
                  <span className={`text-[14px] font-semibold ${L ? 'text-black' : 'text-white'}`}>{t.search.title}</span>
                  <button
                    onClick={() => {
                      setShowSearchModal(false);
                      setSearchResults([]);
                      setCurrentSearchIndex(-1);
                      setHasSearched(false);
                      setHexdumpScrollToAddress(null);
                    }}
                    onMouseDown={(e) => e.stopPropagation()}
                    className={`p-1 rounded transition-colors ${L ? 'hover:bg-black/10 text-gray-500 hover:text-black' : 'hover:bg-white/10 text-gray-400 hover:text-white'}`}
                  >
                    <X className="w-4 h-4" />
                  </button>
                </div>

                <div className="p-4 flex flex-col gap-4">
                  {/* Current format indicator */}
                  <div className={`flex items-center justify-center gap-2 text-[11px] ${L ? 'text-gray-500' : 'text-gray-400'}`}>
                    <span className={`px-2 py-1 rounded font-medium ${L ? 'bg-blue-600/15 text-blue-700' : 'bg-blue-600/30 text-blue-300'}`}>
                      {hexdumpSize === '8b' ? '8b' : '16b'}
                    </span>
                    <span className={`px-2 py-1 rounded font-medium ${L ? 'bg-red-600/15 text-red-700' : 'bg-red-600/30 text-red-300'}`}>
                      {hexdumpFormat === 'hex' ? 'Hex' : 'Dec'}
                    </span>
                  </div>

                  {/* Value input */}
                  <div className="flex gap-2 items-center">
                    <input
                      type="text"
                      autoFocus
                      value={searchConfig.value}
                      onChange={(e) => {
                        setSearchConfig(prev => ({ ...prev, value: e.target.value }));
                        setHasSearched(false); // Reset search state when typing
                      }}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                          e.preventDefault();
                          handleSearch();
                        } else if (e.key === 'Escape') {
                          setShowSearchModal(false);
                          setSearchResults([]);
                          setCurrentSearchIndex(-1);
                          setHasSearched(false);
                          setHexdumpScrollToAddress(null);
                        }
                      }}
                      className={`flex-1 h-9 px-3 rounded text-[12px] focus:outline-none ${L ? 'text-black bg-black/5 border border-black/15 placeholder:text-gray-400' : 'text-white bg-white/10 border border-white/20'}`}
                      placeholder={`${t.search.value} (${hexdumpFormat === 'hex' ? 'Hex' : 'Dec'})`}
                      spellCheck={false}
                    />
                  </div>

                  {/* Address range */}
                  <div className="flex gap-3">
                    <div className="flex-1">
                      <label className={`block text-[11px] mb-1 ${L ? 'text-gray-600' : 'text-gray-400'}`}>{t.search.fromAddress}</label>
                      <input
                        type="text"
                        value={searchConfig.fromAddress}
                        onChange={(e) => setSearchConfig(prev => ({ ...prev, fromAddress: e.target.value.toUpperCase() }))}
                        className={`w-full h-8 px-2 rounded text-[12px] focus:outline-none font-mono ${L ? 'text-black bg-black/5 border border-black/15 placeholder:text-gray-400' : 'text-white bg-white/10 border border-white/20'}`}
                        placeholder="00000"
                        spellCheck={false}
                      />
                    </div>
                    <div className="flex-1">
                      <label className={`block text-[11px] mb-1 ${L ? 'text-gray-600' : 'text-gray-400'}`}>{t.search.toAddress}</label>
                      <input
                        type="text"
                        value={searchConfig.toAddress}
                        onChange={(e) => setSearchConfig(prev => ({ ...prev, toAddress: e.target.value.toUpperCase() }))}
                        className={`w-full h-8 px-2 rounded text-[12px] focus:outline-none font-mono ${L ? 'text-black bg-black/5 border border-black/15 placeholder:text-gray-400' : 'text-white bg-white/10 border border-white/20'}`}
                        placeholder="FFFFF"
                        spellCheck={false}
                      />
                    </div>
                  </div>

                  {/* Results info */}
                  {searchResults.length > 0 && (
                    <div className={`text-[11px] text-center ${L ? 'text-gray-600' : 'text-gray-400'}`}>
                      {currentSearchIndex + 1} / {searchResults.length} {t.search.resultsFound}
                      <span className="ml-2 font-mono text-gray-500">
                        @ 0x{searchResults[currentSearchIndex]?.toString(16).toUpperCase().padStart(5, '0')}
                      </span>
                    </div>
                  )}
                  {searchResults.length === 0 && hasSearched && (
                    <div className={`text-[11px] text-center ${L ? 'text-red-600' : 'text-red-400'}`}>
                      {t.search.noResults}
                    </div>
                  )}

                  {/* Buttons */}
                  <div className="flex gap-2 mt-1">
                    <button
                      type="button"
                      onClick={() => {
                        setShowSearchModal(false);
                        setSearchResults([]);
                        setCurrentSearchIndex(-1);
                        setHasSearched(false);
                        setHexdumpScrollToAddress(null);
                      }}
                      className={`px-4 py-2 rounded text-[12px] font-medium transition-colors border ${L ? 'text-gray-700 hover:bg-black/5 border-black/15' : 'text-gray-300 hover:bg-white/10 border-white/20'}`}
                    >
                      {t.common.close}
                    </button>
                    <button
                      type="button"
                      disabled={searchResults.length === 0}
                      onClick={() => navigateSearchResult('prev')}
                      className={`px-3 py-2 rounded text-[12px] font-medium transition-colors border ${
                        searchResults.length > 0
                          ? (L ? 'text-gray-700 hover:bg-black/5 border-black/15' : 'text-gray-300 hover:bg-white/10 border-white/20')
                          : (L ? 'text-gray-400 border-gray-300 cursor-not-allowed' : 'text-gray-600 border-gray-700 cursor-not-allowed')
                      }`}
                      title={t.search.previous}
                    >
                      ←
                    </button>
                    <button
                      type="button"
                      disabled={searchResults.length === 0}
                      onClick={() => navigateSearchResult('next')}
                      className={`px-3 py-2 rounded text-[12px] font-medium transition-colors border ${
                        searchResults.length > 0
                          ? (L ? 'text-gray-700 hover:bg-black/5 border-black/15' : 'text-gray-300 hover:bg-white/10 border-white/20')
                          : (L ? 'text-gray-400 border-gray-300 cursor-not-allowed' : 'text-gray-600 border-gray-700 cursor-not-allowed')
                      }`}
                      title={t.search.next}
                    >
                      →
                    </button>
                    <button
                      type="button"
                      disabled={!searchConfig.value.trim()}
                      onClick={handleSearch}
                      className={`flex-1 px-4 py-2 rounded text-[12px] font-medium transition-colors ${
                        searchConfig.value.trim()
                          ? 'text-white bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400'
                          : (L ? 'text-gray-400 bg-gray-200 cursor-not-allowed' : 'text-gray-500 bg-gray-700 cursor-not-allowed')
                      }`}
                    >
                      {t.toolbar.search}
                    </button>
                  </div>
                </div>
              </div>
              );
            })()}

            {/* Map Properties Modal - dans le workspace */}
            {showMapPropertiesModal && mapPropertiesTarget && (
              <MapPropertiesModal
                mapData={mapPropertiesTarget}
                settings={getMapDisplaySettings(mapPropertiesTarget)}
                onClose={handleCloseMapPropertiesModal}
                onSave={(settings) => handleSaveMapDisplaySettings(mapPropertiesTarget.address, settings)}
                isClosing={isClosingMapPropertiesModal}
                theme={theme}
                workspaceRef={workspaceRef}
              />
            )}

            {/* Unsaved Changes Confirmation Dialog */}
            {showUnsavedChangesDialog && (
              <ConfirmModal
                title={t.versionDialogs.quitTitle}
                description={t.versionDialogs.quitDescription}
                cancelLabel={t.versionDialogs.cancel}
                middleLabel={t.versionDialogs.quitDiscard}
                confirmLabel={t.versionDialogs.quitSave}
                loadingLabel={t.versionDialogs.quitSaving}
                isLoading={loadingAction === "save"}
                onCancel={handleCancelDialog}
                onMiddle={handleDiscardChanges}
                onConfirm={handleSaveAndContinue}
              />
            )}

            {/* Confirmation avant recalcul du checksum : les modifications non
                sauvegardées doivent d'abord être enregistrées */}
            {showChecksumSaveConfirm && (
              <ConfirmModal
                title={t.checksum.saveBeforeRecalcTitle}
                description={t.checksum.saveBeforeRecalcDescription}
                cancelLabel={t.versionDialogs.cancel}
                confirmLabel={t.checksum.saveAndRecalc}
                onCancel={() => setShowChecksumSaveConfirm(false)}
                onConfirm={handleSaveAndRecalcChecksum}
              />
            )}

            {/* Delete-version confirmation (same theme as the settings window) */}
            {deleteVersionName !== null && (
              <ConfirmModal
                title={t.versionDialogs.deleteVersionTitle}
                description={`${t.versionDialogs.deleteVersionConfirm} "${deleteVersionName}"? ${t.versionDialogs.deleteVersionWarning}`}
                cancelLabel={t.versionDialogs.cancel}
                confirmLabel={t.versionDialogs.deleteVersionButton}
                onCancel={() => setDeleteVersionName(null)}
                onConfirm={performDeleteVersion}
              />
            )}

            {/* Rename-version prompt (same theme as the settings window) */}
            {renameVersionValue !== null && (
              <PromptModal
                title={t.versionDialogs.renameVersionTitle}
                description={t.versionDialogs.renameVersionPrompt}
                initialValue={renameVersionValue}
                cancelLabel={t.versionDialogs.cancel}
                confirmLabel={t.versionDialogs.renameVersionButton}
                onCancel={() => setRenameVersionValue(null)}
                onConfirm={performRenameVersion}
              />
            )}

            {/* Generic text prompt — used for "new version name" (same theme) */}
            {textPrompt !== null && (
              <PromptModal
                title={textPrompt.title}
                description={textPrompt.description}
                initialValue={textPrompt.initialValue}
                cancelLabel={t.versionDialogs.cancel}
                confirmLabel={t.common.apply}
                onCancel={() => {
                  textPrompt.resolve(null);
                  setTextPrompt(null);
                }}
                onConfirm={(value) => {
                  textPrompt.resolve(value);
                  setTextPrompt(null);
                }}
              />
            )}

            <div className="relative w-full h-full min-h-[420px]">
              {/* Fenêtre Hexdump */}
              {projectData && (
                <>
                  {/* Fenêtre Hexdump — fermable (croix) ; se rouvre via la
                      ligne « Hexdump » de l'arbre des maps */}
                  <div
                    ref={hexdumpRef}
                    style={{
                      position: "absolute",
                      top: hexdumpLayout.y,
                      left: hexdumpLayout.x,
                      width: hexdumpLayout.width,
                      height: hexdumpLayout.height,
                      // Largeur min = contenu complet du mode courant : l'ASCII
                      // et la minimap ne peuvent jamais être rognés au resize.
                      minWidth: HEXDUMP_WINDOW_WIDTH[hexdumpSize],
                      minHeight: 105,
                      maxWidth: HEXDUMP_WINDOW_WIDTH[hexdumpSize] + 80,
                      resize: "both",
                      overflow: "hidden",
                      zIndex: hexdumpZIndex,
                      background: getWindowBg(),
                      borderColor: getBorderColor(),
                    }}
                    className={`border rounded-lg shadow-lg shadow-black/30 flex flex-col transition-[opacity,transform] duration-300 ease-out ${
                      hexdumpCollapsed
                        ? 'opacity-0 scale-95 pointer-events-none'
                        : 'opacity-100 scale-100'
                    }`}
                    onMouseUp={handleHexdumpResizeStop}
                    onMouseDownCapture={bringHexdumpToFront}
                  >
                    <div
                      className="flex items-center justify-between px-3 py-[0.3rem] border-b cursor-move select-none"
                      style={{ background: getWindowHeaderBg(), borderColor: getBorderColor() }}
                      onMouseDown={handleHexdumpDragStart}
                    >
                      <div className="flex items-center gap-2" style={{ color: getWindowHeaderTextColor() }}>
                        <Cpu className="w-5 h-5" />
                        <span className="text-sm font-semibold">Hexdump</span>
                      </div>
                      <div className="flex items-center gap-2">
                        <Button
                          variant="ghost"
                          size="sm"
                          className={`h-7 w-7 p-0 ${getButtonHoverClass()}`}
                          style={{ color: getWindowHeaderTextColor() }}
                          onClick={handleCollapseHexdump}
                          onMouseDown={(e) => e.stopPropagation()}
                          title="Fermer le hexdump"
                        >
                          <X className="w-4 h-4" />
                        </Button>
                      </div>
                    </div>
                    <div className="flex-1 min-h-[260px] overflow-hidden relative">
                      <HexdumpViewer
                        fileData={hexdumpDisplayData.length > 0 ? hexdumpDisplayData : projectData.file_data}
                        originalFileData={originalFileDataRef.current ?? undefined}
                        fileName={projectData.file_name}
                        size={hexdumpSize}
                        byteOrder={hexdumpByteOrder}
                        format={hexdumpFormat}
                        containerWidth="100%"
                        minWidthOverride="0px"
                        theme={theme}
                        mapRegions={mappackUnlocked ? projectData.detectionResults.maps.map(m => ({
                          name: m.name,
                          address: m.address,
                          size: m.size,
                          codeblock_id: m.codeblock_id,
                          dimensions: m.dimensions,
                        })) : []}
                        selectedMapAddress={hexdumpScrollToAddress}
                        onScrollComplete={() => setHexdumpScrollToAddress(null)}
                        onMapClick={(mapRegion) => {
                          const map = projectData.detectionResults.maps.find(m => m.address === mapRegion.address);
                          if (map) handleMapClick(map);
                        }}
                        searchResults={searchResults}
                        currentSearchIndex={currentSearchIndex}
                        searchDataSize={hexdumpSize}
                        scrollKey={hexdumpScrollKey}
                        onSearchClick={() => setShowSearchModal(true)}
                        searchButtonLabel={t.search?.button || "Search"}
                      />
                      {/* Resize handle overlay - prevents scroll interference */}
                      <div
                        className="absolute bottom-0 right-0 w-4 h-4 cursor-se-resize"
                        style={{ zIndex: 10 }}
                        onMouseDown={(e) => e.stopPropagation()}
                      />
                    </div>
                  </div>
                </>
              )}

              {/* Fenêtres maps */}
              {openMaps.length === 0 ? (
                <div className="flex items-center justify-center h-full min-h-[320px]" />
              ) : (
                <div className="relative w-full h-full min-h-[360px]">
                  {openMaps.map((map, idx) => {
                    const addressHex = map.address ? `0x${map.address.toString(16).toUpperCase()}` : "Map";
                    const getDefaultLayoutForMap = (m: any, order: number) => {
                      const mapValuesAny = m?.map_values;
                      const dim = m?.dimensions?.TwoDimensional;
                      const colCount =
                        (Array.isArray(mapValuesAny?.[0]) && mapValuesAny[0].length)
                          ? mapValuesAny[0].length
                          : (typeof dim?.cols === "number" ? dim.cols : 1);
                      const rowCount =
                        Array.isArray(mapValuesAny)
                          ? mapValuesAny.length
                          : (typeof dim?.rows === "number" ? dim.rows : 1);

                    // Largeur basée sur X (colonnes), hauteur basée sur Y (lignes)
                    const {
                      stickyWidth,
                      colWidth,
                      headerHeight,
                      rowHeight,
                      chromeHeight,
                      paddingHeight,
                    } = TEXT_VIEW_SIZING;
                    const paddingWidth = getTextPaddingWidth(colCount);

                      const tableWidth = stickyWidth + colCount * colWidth;
                      const tableHeight = headerHeight + rowCount * rowHeight;

                    const baseWidth = tableWidth + paddingWidth;
                    const baseHeight = tableHeight + chromeHeight + paddingHeight;

                    const width = Math.max(240, Math.min(3200, baseWidth));
                    const height = Math.min(1800, Math.max(180, baseHeight));

                      // cascade douce selon l'ordre d'ouverture
                      const offset = order * 24;
                      return {
                        x: 40 + offset,
                        y: 30 + offset,
                        width,
                        height,
                        minWidth: Math.max(155, baseWidth),
                        minHeight: Math.max(180, baseHeight)
                      };
                    };
                    const existingLayout = mapLayouts.get(map.address);
                    const autoLayout = getDefaultLayoutForMap(map, idx);
                    // Ne pas écraser un layout déjà mesuré (autoSize ou resize user)
                    const layout = existingLayout ?? autoLayout;
                    const zIndex = 20 + idx;

                    // Largeur minimale basée sur le nombre de colonnes pour éviter que la fenêtre recouvre le tableau
                    // 56px (colonne Y-axis) + colCount * 42px (largeur min des cellules) + 20px (bordures/padding)
                    const mapValuesForMinWidth = (map as any)?.map_values;
                    const dimForMinWidth = (map as any)?.dimensions?.TwoDimensional;

                    // Détecter si c'est une map EGR (axes inversés à l'affichage)
                    const mapNameLowerForMinWidth = (map.name || '').toLowerCase();
                    const isEgrMapForMinWidth = mapNameLowerForMinWidth.includes("egr") &&
                      !mapNameLowerForMinWidth.includes("temperature") &&
                      !mapNameLowerForMinWidth.includes("temp");
                    // Drivers wish (MJD6) : stockée [pedal][rpm] mais affichée transposée
                    // par MapViewer (RPM en lignes) — mêmes dimensions affichées que EGR.
                    // Sans ça, le minWidth CSS est calculé sur les 16 colonnes FICHIER et
                    // clampe la fenêtre (~500px) : léger espace à droite à l'ouverture, et
                    // fenêtre qui ne suit plus le tableau quand on la rétrécit.
                    const isDriversWishForMinWidth = mapNameLowerForMinWidth.includes("drivers wish");
                    // Torque limiter EDC15P : stockée 21x3 (colonne-major) mais affichée
                    // transposée 3x21 par MapViewer (même condition que sa règle d'affichage).
                    // Sans ce swap, le minHeight CSS est calculé sur les 21 lignes FICHIER
                    // et laisse un grand espace vide sous le tableau de 3 lignes.
                    // L'EDC16 (4x25, non transposée) ne matche pas cette condition.
                    const isTransposedTorqueLimiterForMinWidth =
                      mapNameLowerForMinWidth.includes("torque limiter") &&
                      typeof dimForMinWidth?.rows === "number" &&
                      typeof dimForMinWidth?.cols === "number" &&
                      dimForMinWidth.rows > dimForMinWidth.cols &&
                      dimForMinWidth.rows >= 15 &&
                      dimForMinWidth.cols <= 5;

                    // Pour les maps EGR, les colonnes affichées correspondent aux rows de l'API
                    // car MapViewer transpose les données pour l'affichage
                    let colCountForMinWidth: number;
                    let rowCountForMinHeight: number;
                    if ((isEgrMapForMinWidth || isDriversWishForMinWidth || isTransposedTorqueLimiterForMinWidth) && dimForMinWidth) {
                      // EGR / Drivers wish: displayed cols = API rows (et inversement)
                      colCountForMinWidth = typeof dimForMinWidth.rows === "number" ? dimForMinWidth.rows : 1;
                      rowCountForMinHeight = typeof dimForMinWidth.cols === "number" ? dimForMinWidth.cols : 1;
                    } else {
                      colCountForMinWidth =
                        (Array.isArray(mapValuesForMinWidth?.[0]) && mapValuesForMinWidth[0].length)
                          ? mapValuesForMinWidth[0].length
                          : (typeof dimForMinWidth?.cols === "number" ? dimForMinWidth.cols : 1);
                      rowCountForMinHeight = Array.isArray(mapValuesForMinWidth)
                        ? mapValuesForMinWidth.length
                        : (typeof dimForMinWidth?.rows === "number" ? dimForMinWidth.rows : 1);
                    }
                    // Bouton d'inversion (invertDisplay) : la vue est re-transposée par
                    // rapport au défaut, les bornes min doivent suivre l'orientation
                    // AFFICHÉE — sinon toute map inversée garde le clamp de l'ancienne
                    // orientation et la fenêtre ne se réduit plus jusqu'au tableau.
                    if (mapDisplaySettingsStore.get(map.address)?.invertDisplay === true) {
                      const swapTmp = colCountForMinWidth;
                      colCountForMinWidth = rowCountForMinHeight;
                      rowCountForMinHeight = swapTmp;
                    }

                    // Bornes minimales CSS = simple garde-fou, volontairement SOUS la taille
                    // réelle minimale : c'est la poignée custom de MapViewer qui cale la
                    // fenêtre exactement sur le tableau à chaque instant du resize (une borne
                    // CSS trop haute laisserait un espace résiduel impossible à combler).
                    const viewModeForMinSize = mapViewModes.get(map.address) || "text";
                    const isTextForMinSize = viewModeForMinSize === "text" && !(mapEasyViewStatus.get(map.address) || false);
                    const calculatedMinWidth = 20 + colCountForMinWidth * 30;
                    const calculatedMinHeight = 55 + rowCountForMinHeight * 12;
                    // 175 = largeur mini pour que la rangée de boutons Text/2D/3D tienne toujours
                    const finalMinWidth = isTextForMinSize ? Math.max(175, calculatedMinWidth) : 240;
                    const finalMinHeight = isTextForMinSize ? Math.max(120, calculatedMinHeight) : 180;

                    // Mémoiser les modifications pour éviter de recréer l'objet à chaque render
                    const mapModifications = allMapModifications.get(map.address);
                    const persistedAxes = mapAxisLabels.get(map.address);

                    return (
                      <div
                        key={`map-window-${map.address}`}
                        ref={(el) => { mapRefs.current[map.address] = el; }}
                        data-map-address={map.address}
                        style={{
                          position: "absolute",
                          top: layout.y,
                          left: layout.x,
                          width: layout.width,
                          height: layout.height,
                          overflow: "hidden",
                          minWidth: finalMinWidth,
                          minHeight: finalMinHeight,
                          zIndex,
                          background: getWindowBg(),
                          borderColor: getBorderColor(),
                        }}
                        className="border rounded-lg shadow-lg shadow-black/30 flex flex-col"
                        onMouseDownCapture={() => bringMapToFront(map.address)}
                      >
                        <div className="flex-1 bg-transparent relative overflow-hidden">
                          <MapViewer
                            key={map.address}
                            mapData={map}
                            fileData={projectData.file_data}
                            projectName={projectData.project_name}
                            fileName={projectData.file_name}
                            viewMode={mapViewModes.get(map.address) || "text"}
                            easyViewMode={mapEasyViewStatus.get(map.address) || false}
                            onViewModeChange={(mode) => handleViewModeChange(map.address, mode)}
                            onAutoSize={(w, h) => handleMapAutoSize(map.address, w, h)}
                            onDragStart={(e) => handleMapDragStart(map.address, e)}
                            onResizeActiveChange={(active) => {
                              setOverlayCursor('se-resize');
                              setIsWindowDragActive(active);
                            }}
                            onClose={() => handleCloseMapWindow(map.address)}
                            currentVersionId={currentVersionId}
                            initialChangedCells={mapModifications}
                            onModificationsChange={(changedCells) => handleMapModifications(map.address, changedCells)}
                            initialXAxisLabels={persistedAxes?.x}
                            initialYAxisLabels={persistedAxes?.y}
                            onAxisLabelsChange={(axes) => handleAxisLabelsChange(map.address, axes)}
                            onAxesFlipChange={handleAxesFlipChange}
                            theme={theme}
                            onSelectionChange={(info) => handleMapSelectionChange(map.address, info)}
                            onPlot3DDataChange={handlePlot3DDataChange}
                            onOpenProperties={() => handleOpenMapProperties(map)}
                            disableTableColors={settings.disableTableColors}
                            disableGraphColors={settings.disableGraphColors}
                            allMaps={projectData.detectionResults.maps}
                            onApplyToSimilarMaps={(targetMaps, copyType) => handleApplyToSimilarMaps(map.address, targetMaps, copyType)}
                            displaySettings={(() => {
                              const mapSettings = mapDisplaySettingsStore.get(map.address);
                              if (!mapSettings) return undefined;
                              // Les défauts d'axes (getDefaultMapDisplaySettings) sont en
                              // orientation BACKEND, alors que MapViewer échange lui-même les
                              // corrections des maps affichées transposées (Drivers wish MJD6…).
                              // Or le toggle d'inversion persiste les settings complets par
                              // défaut : forwarder ces facteurs tels quels écraserait les
                              // corrections échangées (pédale affichée brute 0..25000, RPM
                              // ×0.004). On ne forwarde un override d'axe QUE s'il diffère du
                              // défaut détecté — un défaut inchangé laisse MapViewer décider.
                              const defaults = getDefaultMapDisplaySettings(map);
                              const axisOverride = (
                                saved: MapDisplaySettings['xAxis'],
                                def: MapDisplaySettings['xAxis']
                              ) => ({
                                mirror: saved.mirror,
                                factor: saved.factor !== def.factor ? saved.factor : undefined,
                                offset: saved.offset !== def.offset ? saved.offset : undefined,
                                divisor: saved.divisor !== def.divisor ? saved.divisor : undefined,
                                precision: saved.precision !== def.precision ? saved.precision : undefined,
                              });
                              return {
                                xAxis: axisOverride(mapSettings.xAxis, defaults.xAxis),
                                yAxis: axisOverride(mapSettings.yAxis, defaults.yAxis),
                                map: {
                                  factor: mapSettings.factor,
                                  offset: mapSettings.offset,
                                  divisor: mapSettings.divisor,
                                  precision: mapSettings.precision,
                                  invertDisplay: mapSettings.invertDisplay,
                                },
                              };
                            })()}
                            onToggleInvertDisplay={handleToggleInvertDisplay}
                            incrementValue={(() => {
                              const parsed = Number(modifyValue.replace(",", "."));
                              return Number.isNaN(parsed) ? 1 : parsed;
                            })()}
                            modifyCommand={map.address === activeMapAddress ? modifyCommand : null}
                            isActive={map.address === activeMapAddress}
                            onViewInHexdump={() => setHexdumpScrollToAddress(map.address)}
                            ecuType={projectData?.ecu_type}
                          />
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}

              {/* Preview Window - dans le même container que les maps */}
              {showPreviewWindow && (
                <PreviewWindow
                  key={previewWindowKey}
                  activeMapAddress={activeMapAddress}
                  openMaps={openMaps}
                  plot3DDataMap={mapPlot3DDataRef.current}
                  dataVersion={previewDataVersion}
                  mapEasyViewStatus={mapEasyViewStatus}
                  zIndex={previewZIndex}
                  layout={previewLayout}
                  onLayoutChange={setPreviewLayout}
                  onClose={handlePreviewToggle}
                  onFocus={bringPreviewToFront}
                  theme={theme}
                  getWindowHeaderBg={getWindowHeaderBg}
                  getWindowHeaderTextColor={getWindowHeaderTextColor}
                  getBorderColor={getBorderColor}
                  getButtonHoverClass={getButtonHoverClass}
                  getWindowBg={getWindowBg}
                  workspaceRef={workspaceRef}
                  t={t}
                  onDragActiveChange={(active) => {
                    setOverlayCursor('move');
                    setIsWindowDragActive(active);
                  }}
                />
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Project Info Modal */}
      {showProjectInfoModal && projectData && (
        <ProjectInfoEditModal
          projectData={projectData}
          onClose={handleCloseProjectInfoModal}
          onSave={handleSaveProjectInfo}
          isClosing={isClosingProjectInfoModal}
        />
      )}


      {/* Overlay plein écran pendant le drag d'une fenêtre : maintient le curseur
          "grabbing" et empêche la souris d'atteindre les graphiques (rotation 3D)
          même si elle sort de la barre de titre pendant un déplacement rapide */}
      {isWindowDragActive && (
        <div
          className="fixed inset-0"
          style={{ zIndex: 9000, cursor: overlayCursor }}
          onContextMenu={(e) => e.preventDefault()}
          onMouseUp={() => setIsWindowDragActive(false)}
        />
      )}

      {/* Barre d'état globale (style WinOLS) - toujours visible */}
      <div
        className="fixed bottom-0 right-0 px-3 py-1.5 text-xs z-50 border shadow-lg shadow-black/30 font-mono"
        style={{
          background: getWindowHeaderBg(),
          borderColor: getBorderColor(),
          color: getWindowHeaderTextColor(),
          borderTopLeftRadius: '6px',
          borderTopRightRadius: '0px',
          borderBottomLeftRadius: '0px',
          borderBottomRightRadius: '0px',
          fontVariantNumeric: 'tabular-nums',
        }}
      >
        <span>
          {isHexdumpActive ? (
            // Affichage pour l'hexdump
            <>
              {t.cursor.hexdump} {projectData ? `${(projectData.file_size / 1024).toFixed(2)} KB` : ''}
            </>
          ) : globalCursorInfo ? (
            // Affichage pour une map
            <>
              {globalCursorInfo.mapName} {globalCursorInfo.dimensions}
              {globalCursorInfo.selectedCount > 0 && (() => {
                const cells = globalCursorInfo.selectedCells;
                // Cellule de référence = première dans l'ordre de lecture (ligne puis colonne)
                const sorted = [...cells].sort((a, b) => a.row - b.row || a.col - b.col);
                const first = sorted[0];
                if (globalCursorInfo.selectedCount === 1 && first) {
                  return (
                    <>
                      {' | '}0x{first.address.toString(16).toUpperCase()}
                      {' | '}{t.cursor.value}: {first.value.toFixed(1)}
                    </>
                  );
                }
                // Sélection multiple : stats min / max / moyenne
                let min = Infinity, max = -Infinity, sum = 0;
                for (const c of cells) {
                  if (c.value < min) min = c.value;
                  if (c.value > max) max = c.value;
                  sum += c.value;
                }
                const avg = sum / cells.length;
                return (
                  <>
                    {' | '}
                    {globalCursorInfo.selectedCount} {t.cursor.cellsSelected}
                    {first && <>{' | '}0x{first.address.toString(16).toUpperCase()}</>}
                    {' | '}Min: {min.toFixed(1)}
                    {' | '}Max: {max.toFixed(1)}
                    {' | '}{t.cursor.avg}: {avg.toFixed(1)}
                  </>
                );
              })()}
              {globalCursorInfo.selectedCount === 0 && (
                <> | {t.cursor.clickCellDetails}</>
              )}
            </>
          ) : (
            // Affichage par défaut
            <>
              {t.cursor.clickMapDetails}
            </>
          )}
        </span>
      </div>

      {/* Map Tree Context Menu */}
      {mapTreeContextMenu && (
        <div
          ref={mapTreeContextMenuRef}
          style={{
            position: 'fixed',
            left: mapTreeContextMenu.x,
            top: mapTreeContextMenu.y,
            background: theme === 'light' ? 'rgba(255, 255, 255, 0.9)' : 'rgba(22, 25, 34, 0.92)',
            border: `1px solid ${theme === 'light' ? 'rgba(0, 0, 0, 0.1)' : 'rgba(255, 255, 255, 0.1)'}`,
            backdropFilter: 'blur(18px) saturate(140%)',
            WebkitBackdropFilter: 'blur(18px) saturate(140%)',
            color: theme === 'light' ? '#000000' : '#ffffff',
            zIndex: 9999,
          }}
          className="rounded-lg shadow-lg p-1.5 flex flex-col gap-0.5 text-[12px] min-w-[180px]"
          onClick={(e) => e.stopPropagation()}
          onMouseDown={(e) => e.stopPropagation()}
        >
          {/* Open Map */}
          <button
            className={`px-3 py-1.5 text-left rounded transition-colors flex items-center gap-2 ${theme === 'light' ? 'hover:bg-black/5' : 'hover:bg-white/10'}`}
            onClick={() => {
              handleMapClick(mapTreeContextMenu.map);
              setMapTreeContextMenu(null);
            }}
          >
            <Eye className="w-4 h-4" />
            {t.mapViewer.openMap}
          </button>

          {/* View in Hexdump */}
          <button
            className={`px-3 py-1.5 text-left rounded transition-colors flex items-center gap-2 ${theme === 'light' ? 'hover:bg-black/5' : 'hover:bg-white/10'}`}
            onClick={() => {
              setHexdumpScrollToAddress(mapTreeContextMenu.map.address);
              setMapTreeContextMenu(null);
            }}
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
            </svg>
            {t.mapViewer.viewInHexdump}
          </button>

          {/* Properties */}
          <button
            className={`px-3 py-1.5 text-left rounded transition-colors flex items-center gap-2 ${theme === 'light' ? 'hover:bg-black/5' : 'hover:bg-white/10'}`}
            onClick={() => {
              handleOpenMapProperties(mapTreeContextMenu.map);
              setMapTreeContextMenu(null);
            }}
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
            </svg>
            {t.mapViewer.properties}
          </button>
        </div>
      )}

      {/* Mappack Unlock Confirmation Popup */}
      {showUnlockConfirm && (
        <div
          className="fixed inset-0 z-[80] flex items-center justify-center backdrop-blur-sm"
          style={{
            backgroundColor: '#000000a2',
            animation: 'backdropFadeIn 0.2s ease-out forwards'
          }}
        >
          <div
            className="relative w-full max-w-md overflow-hidden"
            style={{
              animation: 'modalExpand 0.2s ease-out forwards'
            }}
          >
            {/* Close button */}
            <button
              onClick={() => setShowUnlockConfirm(false)}
              className="absolute top-4 right-4 z-10 p-2 rounded-lg transition-colors hover:bg-white/5"
              style={{ color: 'rgba(255, 255, 255, 0.6)' }}
            >
              <X className="w-5 h-5" />
            </button>

            <div className="border rounded-lg p-8" style={MODAL_GLASS}>
              {/* Header with icon */}
              <div className="flex flex-col items-center mb-6">
                <div
                  className="w-20 h-20 rounded-full flex items-center justify-center mb-6"
                  style={{ background: 'linear-gradient(135deg, rgba(59, 130, 246, 0.15), rgba(99, 102, 241, 0.15))' }}
                >
                  <svg xmlns="http://www.w3.org/2000/svg" width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="url(#lockGradient)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <defs>
                      <linearGradient id="lockGradient" x1="0%" y1="0%" x2="100%" y2="100%">
                        <stop offset="0%" stopColor="#3b82f6" />
                        <stop offset="100%" stopColor="#6366f1" />
                      </linearGradient>
                    </defs>
                    <rect width="18" height="11" x="3" y="11" rx="2" ry="2"/>
                    <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
                  </svg>
                </div>

                <h2 className="text-xl font-semibold text-center text-white">
                  {t.mappack?.confirmUnlock || "Confirm Unlock"}
                </h2>
                <p className="text-sm mt-2 text-center" style={{ color: 'rgba(255,255,255,0.5)' }}>
                  {t.mappack?.confirmUnlockQuestion || "Do you want to confirm the mappack unlock?"}
                </p>
              </div>

              {/* Action buttons */}
              <div className="flex gap-3">
                <Button
                  onClick={() => setShowUnlockConfirm(false)}
                  variant="outline"
                  className="flex-1"
                >
                  {t.common?.cancel || "Cancel"}
                </Button>
                <Button
                  onClick={confirmUnlockMappack}
                  className="flex-1 bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400 text-white"
                >
                  {t.mappack?.confirmUnlockButton || "Unlock"}
                </Button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Solutions Modal */}
      {isSolutionsOpen && projectData && (
        <SolutionsModal
          ecuType={projectData.ecu_type}
          theme={theme}
          isClosing={isSolutionsClosing}
          usedSolutions={usedSolutions}
          // État « déjà actif » : les cartes issues de la détection — le
          // détecteur n'expose « Launch control map » que si elle est activée
          detectedMaps={projectData.detectionResults?.maps || []}
          onClose={() => {
            setIsSolutionsClosing(true);
            setTimeout(() => {
              setIsSolutionsOpen(false);
              setIsSolutionsClosing(false);
            }, 200);
          }}
          onApplySolutions={(solutionIds: string[]) => {
            if (!projectData || solutionIds.length === 0) return;

            const originalData = originalFileDataRef.current
              ? new Uint8Array(originalFileDataRef.current)
              : new Uint8Array(projectData.file_data);

            const currentData = new Uint8Array(projectData.file_data);
            const allChangedAddresses: number[] = [];
            const newMapsToAdd: any[] = [];
            const appliedSolutions: string[] = [];

            for (const solutionId of solutionIds) {
              if (solutionId in usedSolutions) continue;

              const impl = getSolutionImplementation(solutionId);
              if (!impl) continue;

              const patches = impl.applyBinaryPatches(currentData);
              if (patches.length === 0) {
                toast({
                  title: t.errors.noMapsFound,
                  description: impl.name,
                  variant: "destructive",
                });
                continue;
              }

              for (const patch of patches) {
                for (let i = 0; i < patch.data.length; i++) {
                  const addr = patch.address + i;
                  if (addr < currentData.length) {
                    currentData[addr] = patch.data[i];
                    allChangedAddresses.push(addr);
                  }
                }

                // La carte devient utilisable : on l'ajoute à la liste si le
                // détecteur ne l'avait pas (fichier chargé avant activation)
                const info = patch.createsMap;
                if (!info) continue;
                const existing = projectData.detectionResults?.maps?.find(
                  (m: any) => m.address === info.address
                );
                if (!existing) {
                  newMapsToAdd.push({
                    name: info.name,
                    address: info.address,
                    size: info.size,
                    dimensions: { TwoDimensional: { rows: info.rows, cols: info.cols } },
                    category: info.category,
                    subcategory: info.subcategory,
                    x_axis_address: info.x_axis_address,
                    y_axis_address: info.y_axis_address,
                    x_axis_correction: info.x_axis_correction,
                    y_axis_correction: info.y_axis_correction,
                    correction_factor: info.correction_factor,
                    x_label: info.x_label,
                    y_label: info.y_label,
                    unit: info.unit,
                    description: info.description,
                    y_axis_inverted: info.y_axis_inverted,
                  });
                }
              }

              appliedSolutions.push(solutionId);
            }

            if (appliedSolutions.length === 0) return;

            if (newMapsToAdd.length > 0 && projectData.detectionResults?.maps) {
              projectData.detectionResults.maps.push(...newMapsToAdd);
              if (typeof projectData.detectionResults.total_maps === "number") {
                projectData.detectionResults.total_maps += newMapsToAdd.length;
              }
            }

            clearMapDataCache();

            setBinaryModifications(prev => {
              const newMods = new Map(prev);
              for (const addr of allChangedAddresses) {
                newMods.set(addr, {
                  oldValue: originalData[addr],
                  newValue: currentData[addr],
                });
              }
              return newMods;
            });

            setProjectData({
              ...projectData,
              file_data: Array.from(currentData),
            });

            setDtcRefreshKey(prev => prev + 1);

            const versionName = currentVersionId
              ? versions.find(v => v.id === currentVersionId)?.name || 'Ori'
              : 'Ori';
            setUsedSolutions(prev => {
              const next = { ...prev };
              for (const id of appliedSolutions) next[id] = versionName;
              return next;
            });
            setHasUnsavedChanges(true);

            setSolutionNotification({ count: appliedSolutions.length, visible: true, fading: false });
            setTimeout(() => {
              setSolutionNotification(prev => ({ ...prev, fading: true }));
            }, 2500);
            setTimeout(() => {
              setSolutionNotification(prev => ({ ...prev, visible: false, fading: false }));
            }, 3000);
          }}
        />
      )}

      {/* DTC Modal */}
      {isDTCOpen && projectData && (
        <DTCModal
          ecuType={projectData.ecu_type}
          fileData={projectData.file_data}
          refreshKey={dtcRefreshKey}
          onClose={() => {
            setIsDTCClosing(true);
            setTimeout(() => {
              setIsDTCOpen(false);
              setIsDTCClosing(false);
            }, 200);
          }}
          onDisableDTCs={(dtcs: DetectedDTC[], codeblocks: CodeblockInfo[]) => {
            if (!projectData || dtcs.length === 0) return;

            // Get original data from ref (for tracking changes)
            const originalData = originalFileDataRef.current
              ? new Uint8Array(originalFileDataRef.current)
              : new Uint8Array(projectData.file_data);

            // Get current file data as Uint8Array
            let currentData = new Uint8Array(projectData.file_data) as Uint8Array<ArrayBuffer>;
            const allChangedAddresses: number[] = [];
            const ecuFamily = projectData.ecu_type || 'EDC15';

            // Disable each selected DTC
            for (const dtc of dtcs) {
              const { modifiedData, changedAddresses } = disableDTC(currentData, dtc, codeblocks, ecuFamily);
              currentData = modifiedData as Uint8Array<ArrayBuffer>;
              allChangedAddresses.push(...changedAddresses);
            }

            // Track binary modifications for saving later (like maps)
            setBinaryModifications(prev => {
              const newMods = new Map(prev);
              for (const addr of allChangedAddresses) {
                newMods.set(addr, {
                  oldValue: originalData[addr],
                  newValue: currentData[addr],
                });
              }
              return newMods;
            });

            // Update the project data with modified file
            setProjectData({
              ...projectData,
              file_data: Array.from(currentData),
            });

            // Mark as having unsaved changes (like maps)
            setHasUnsavedChanges(true);

            // Force DTC modal to refresh and show updated status
            setDtcRefreshKey(prev => prev + 1);

            // Show DTC notification badge
            setDtcNotification({ count: dtcs.length, visible: true, fading: false });
            // Start fade out after 2.5 seconds
            setTimeout(() => {
              setDtcNotification(prev => ({ ...prev, fading: true }));
            }, 2500);
            // Hide completely after fade animation (0.5s)
            setTimeout(() => {
              setDtcNotification(prev => ({ ...prev, visible: false, fading: false }));
            }, 3000);

          }}
          isClosing={isDTCClosing}
          theme={theme}
        />
      )}

      {/* Compare Modal */}
      <CompareModal
        isOpen={isCompareOpen}
        onClose={() => setIsCompareOpen(false)}
        versions={versions}
        fileId={projectData?.fileId || ""}
        originalFileData={originalFileDataRef.current || []}
        currentFileData={hexdumpDisplayData.length > 0 ? hexdumpDisplayData : undefined}
        resolveVersionData={buildVersionFileData}
        hexdumpSize={hexdumpSize}
        hexdumpFormat={hexdumpFormat}
        hexdumpByteOrder={hexdumpByteOrder}
        mapRegions={projectData?.detectionResults?.maps?.map(m => ({
          name: m.name,
          address: m.address,
          size: m.size,
          codeblock_id: m.codeblock_id,
          dimensions: m.dimensions,
        })) || []}
        ecuType={projectData?.ecu_type}
      />

      {/* Checksum Modal */}
      {isChecksumModalOpen && (
        <ChecksumModal
          onClose={closeChecksumModal}
          onExportWithoutChecksum={handleExportWithoutChecksum}
          onExportChecksumOk={handleExportChecksumAlreadyOk}
          onExportWithChecksum={handleExportWithChecksum}
          status={exportChecksumStatus}
          isClosing={isChecksumModalClosing}
          isCalculating={isChecksumCalculating}
          calculationComplete={isChecksumComplete}
        />
      )}

      {/* État du mappack : rapport de complétude EDC16. S'ouvre au clic sur
          le badge %, et automatiquement au chargement s'il manque des maps. */}
      {showMappackHealth && expectedMaps && (
        <div
          className="fixed inset-0 z-[70] flex items-center justify-center backdrop-blur-sm"
          style={{ backgroundColor: '#000000a2' }}
          onClick={() => setShowMappackHealth(false)}
        >
          <div
            className="border rounded-lg p-6 max-w-md w-full mx-4 max-h-[85vh] overflow-y-auto upload-scroll"
            style={theme === 'light' ? MODAL_GLASS_LIGHT : MODAL_GLASS}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between mb-3">
              <h2 className="text-lg font-bold" style={{ color: getTextColor() }}>
                {t.mappackHealth.title}
              </h2>
              <span
                className={`px-2.5 py-0.5 rounded-full text-sm font-bold tabular-nums ${
                  missingExpected.length === 0
                    ? 'bg-emerald-500/15 text-emerald-400'
                    : 'bg-orange-500/15 text-orange-400'
                }`}
              >
                {mappackConfidence}%
              </span>
            </div>
            <p className="text-sm mb-3" style={{ color: theme === 'light' ? 'rgba(0,0,0,0.6)' : 'rgba(255,255,255,0.6)' }}>
              {t.mappackHealth.intro}
            </p>
            {missingExpected.length === 0 ? (
              <p className="text-sm mb-4 text-emerald-400">{t.mappackHealth.allGood}</p>
            ) : (
              <>
                <p className="text-sm mb-2 text-orange-400 font-medium">{t.mappackHealth.missingIntro}</p>
                <div className="rounded-md border mb-3 px-3 py-1.5" style={{ borderColor: theme === 'light' ? 'rgba(0,0,0,0.1)' : 'rgba(255,255,255,0.1)' }}>
                  {missingExpected.map((e) => (
                    <div key={e.label} className="flex items-center justify-between py-1 text-sm">
                      <span className="text-orange-400">{e.label}</span>
                      <span className="font-mono text-xs text-orange-400">{e.found}/{e.expected}</span>
                    </div>
                  ))}
                </div>
                <p className="text-sm mb-4" style={{ color: theme === 'light' ? 'rgba(0,0,0,0.6)' : 'rgba(255,255,255,0.6)' }}>
                  {t.mappackHealth.advice}
                </p>
              </>
            )}
            {/* Détail complet des familles vérifiées */}
            <div className="rounded-md border px-3 py-1.5 mb-4" style={{ borderColor: theme === 'light' ? 'rgba(0,0,0,0.08)' : 'rgba(255,255,255,0.08)' }}>
              {expectedMaps.map((e) => {
                const ok = e.found >= e.expected;
                return (
                  <div key={e.label} className="flex items-center justify-between py-0.5 text-xs">
                    <span style={{ color: ok ? (theme === 'light' ? 'rgba(0,0,0,0.7)' : 'rgba(255,255,255,0.7)') : '#fb923c' }}>
                      {e.label}
                    </span>
                    <span className={`font-mono ${ok ? 'text-emerald-400' : 'text-orange-400'}`}>
                      {e.found}/{e.expected} {ok ? '✓' : '✗'}
                    </span>
                  </div>
                );
              })}
            </div>
            <div className="flex justify-end">
              <Button
                onClick={() => setShowMappackHealth(false)}
                className="bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400 text-white"
              >
                {t.mappackHealth.close}
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* Mappack Export Modal */}
      {isMappackExportModalOpen && (
        <MappackExportModal
          onClose={closeMappackExportModal}
          onConfirm={handleConfirmMappackExport}
          cost={mappackPrice}
          isClosing={isMappackExportModalClosing}
          isExporting={isExportingMappack}
          exportComplete={isMappackExportComplete}
        />
      )}


      {/* DTC Disabled Notification */}
      {dtcNotification.visible && (
        <div
          className={`fixed bottom-6 z-[100] flex items-center gap-2.5 px-3 py-1.5 rounded-full ${theme === "light" ? "text-slate-900" : "text-white"}`}
          style={{
            ...(theme === "light" ? TOAST_GLASS_LIGHT : TOAST_GLASS),
            left: '50%',
            transform: 'translateX(-50%)',
            animation: dtcNotification.fading ? 'fadeOutDown 0.5s ease-out forwards' : 'slideUp 0.3s ease-out forwards',
          }}
        >
          <span className="w-6 h-6 rounded-full flex items-center justify-center flex-shrink-0" style={{ backgroundColor: 'rgba(249, 115, 22, 0.15)' }}>
            <svg className="w-3.5 h-3.5" fill="none" stroke="#f97316" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728A9 9 0 015.636 5.636m12.728 12.728L5.636 5.636" />
            </svg>
          </span>
          <span className="font-medium">
            {dtcNotification.count} {dtcNotification.count > 1 ? t.notifications.dtcsDisabled : t.notifications.dtcDisabled}
          </span>
        </div>
      )}

      {/* Solution Applied Notification */}
      {solutionNotification.visible && (
        <div
          className={`fixed bottom-6 z-[100] flex items-center gap-2.5 px-3 py-1.5 rounded-full ${theme === "light" ? "text-slate-900" : "text-white"}`}
          style={{
            ...(theme === "light" ? TOAST_GLASS_LIGHT : TOAST_GLASS),
            left: '50%',
            transform: 'translateX(-50%)',
            animation: solutionNotification.fading ? 'fadeOutDown 0.5s ease-out forwards' : 'slideUp 0.3s ease-out forwards',
          }}
        >
          <span className="w-6 h-6 rounded-full flex items-center justify-center flex-shrink-0" style={{ backgroundColor: 'rgba(239, 68, 68, 0.15)' }}>
            <PiHeadCircuit className="w-3.5 h-3.5" style={{ color: '#ef4444' }} />
          </span>
          <span className="font-medium">
            {solutionNotification.count} {t.sidebar.solutions}
          </span>
        </div>
      )}

      {/* Save/Version Notification */}
      {saveNotification.visible && (
        <div
          className={`fixed bottom-6 z-[100] flex items-center gap-2.5 px-3 py-1.5 rounded-full ${theme === "light" ? "text-slate-900" : "text-white"}`}
          style={{
            ...(theme === "light" ? TOAST_GLASS_LIGHT : TOAST_GLASS),
            left: '50%',
            transform: 'translateX(-50%)',
            animation: saveNotification.fading ? 'fadeOutDown 0.5s ease-out forwards' : 'slideUp 0.3s ease-out forwards',
          }}
        >
          <span className="w-6 h-6 rounded-full flex items-center justify-center flex-shrink-0" style={{ backgroundColor: 'rgba(34, 197, 94, 0.15)' }}>
            <svg className="w-3.5 h-3.5" fill="none" stroke="#22c55e" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
            </svg>
          </span>
          <span className="font-medium">
            {saveNotification.message || (saveNotification.type === 'save' ? t.notifications.projectSaved : saveNotification.type === 'deleted' ? (t.notifications.versionDeleted || 'Version supprimée') : t.notifications.versionCreated)}
          </span>
        </div>
      )}

      {/* Version Limit Notification */}
      {versionLimitNotification.visible && (
        <div
          className={`fixed bottom-6 z-[100] flex items-center gap-2.5 px-3 py-1.5 rounded-full ${theme === "light" ? "text-slate-900" : "text-white"}`}
          style={{
            ...(theme === "light" ? TOAST_GLASS_LIGHT : TOAST_GLASS),
            left: '50%',
            transform: 'translateX(-50%)',
            animation: versionLimitNotification.fading ? 'fadeOutDown 0.5s ease-out forwards' : 'slideUp 0.3s ease-out forwards',
          }}
        >
          <span className="w-6 h-6 rounded-full flex items-center justify-center flex-shrink-0" style={{ backgroundColor: 'rgba(239, 68, 68, 0.15)' }}>
            <AlertTriangle className="w-3.5 h-3.5" style={{ color: '#ef4444' }} />
          </span>
          <div className="flex flex-col pr-1">
            <span className="font-medium text-sm">{t.versionDialogs.limitReachedTitle}</span>
            <span className={`text-xs ${theme === "light" ? "text-black/60" : "text-white/60"}`}>{t.versionDialogs.limitReachedDescription}</span>
          </div>
        </div>
      )}

    </div>
  );
}

/** Largeur (px CSS) nécessaire à la BARRE D'OUTILS seule, marge comprise.
 *  La largeur de la barre latérale — que l'utilisateur peut élargir — s'y
 *  ajoute à l'exécution, et le total est converti selon le zoom courant. */
// +60 px depuis l'ajout du bouton HiLo/LoHi dans la pastille 8b/16b : en
// dessous, la barre recouvrait les contrôles de fenêtre à largeur minimale.
const TOOLBAR_MIN_CSS_WIDTH = 1095;

export default function EditorPage() {
  return (
    <ThemeProvider scope="editor">
      <ZedGradientDefs />
      <EditorPageContent />
    </ThemeProvider>
  );
}
