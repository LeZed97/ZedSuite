"use client";

import { useState, useEffect, useMemo, useRef, useCallback } from "react";
import { ChevronLeft, ChevronRight, ExternalLink, Search } from "lucide-react";
import { useI18n } from "@/contexts/i18n-context";
import { resolveAxisLabels } from "@/lib/map-cell-layout";
import { MapCompare3D } from "@/components/map-compare-3d";
import type { ExtractMapDisplaySettings } from "@/lib/map-extract";
import {
  compareAllMaps,
  compareMapDetail,
  cellsDiffer,
  percentChange,
  type CompareMapInput,
  type MapCompareSummary,
} from "@/lib/map-compare";

export type { CompareMapInput };

type ValueView = "left" | "right" | "diff" | "pct" | "3d";

interface MapCompareViewProps {
  maps: CompareMapInput[];
  leftData: number[];
  rightData: number[];
  leftName: string;
  rightName: string;
  ecuType?: string;
  theme: string;
  hairline: string;
  getDisplaySettings?: (map: CompareMapInput) => ExtractMapDisplaySettings | undefined;
  onOpenMap?: (address: number) => void;
  onBack: () => void;
}

// Même convention que l'hexdump / WinOLS : valeur plus HAUTE à droite en
// rouge, plus BASSE en bleu. L'intensité du fond suit l'ampleur de l'écart.
const DIFF_RGB = {
  dark: { up: [255, 82, 82], down: [77, 163, 255] },
  light: { up: [198, 40, 40], down: [21, 101, 192] },
};
const AXIS_CHANGED_BG = "rgba(255, 193, 7, 0.28)";

const fmtSigned = (v: number, decimals: number) => {
  const s = v.toFixed(decimals);
  return v > 0 ? `+${s}` : s;
};

export function MapCompareView({
  maps,
  leftData,
  rightData,
  leftName,
  rightName,
  ecuType,
  theme,
  hairline,
  getDisplaySettings,
  onOpenMap,
  onBack,
}: MapCompareViewProps) {
  const { t } = useI18n();
  const light = theme === "light";
  const textColor = light ? "#000000" : "#ffffff";
  const mutedColor = light ? "rgba(0,0,0,0.5)" : "rgba(255,255,255,0.45)";
  const hoverClass = light ? "hover:bg-black/10" : "hover:bg-white/10";
  const buttonBorder = `1px solid ${light ? "#dee2e6" : "rgba(255, 255, 255, 0.2)"}`;

  const [changedOnly, setChangedOnly] = useState(true);
  const [query, setQuery] = useState("");
  const [selectedAddress, setSelectedAddress] = useState<number | null>(null);
  const [view, setView] = useState<ValueView>("right");
  const [hovered, setHovered] = useState<{ row: number; col: number } | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Résumé de toutes les maps (pré-filtre par octets, puis lecture complète
  // des seules maps touchées)
  const summaries = useMemo(
    () => compareAllMaps(maps, leftData, rightData, ecuType, getDisplaySettings),
    [maps, leftData, rightData, ecuType, getDisplaySettings],
  );

  const changedSummaries = useMemo(
    () => summaries.filter((s) => s.changedCells > 0 || s.axisChanged),
    [summaries],
  );

  const visibleSummaries = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (changedOnly ? changedSummaries : summaries).filter((s) => {
      if (!q) return true;
      return (
        s.map.name.toLowerCase().includes(q) ||
        s.map.address.toString(16).toLowerCase().includes(q.replace(/^0x/, ""))
      );
    });
  }, [summaries, changedSummaries, changedOnly, query]);

  // Première map modifiée sélectionnée d'office
  useEffect(() => {
    if (selectedAddress !== null && summaries.some((s) => s.map.address === selectedAddress)) return;
    const first = changedSummaries[0] ?? summaries[0];
    setSelectedAddress(first ? first.map.address : null);
  }, [summaries, changedSummaries, selectedAddress]);

  const selected = useMemo<MapCompareSummary | null>(
    () => summaries.find((s) => s.map.address === selectedAddress) ?? null,
    [summaries, selectedAddress],
  );

  const detail = useMemo(() => {
    if (!selected) return null;
    try {
      return compareMapDetail(selected.map, leftData, rightData, ecuType, getDisplaySettings?.(selected.map));
    } catch (error) {
      console.error("[map-compare] failed to read map", selected.map.name, error);
      return null;
    }
  }, [selected, leftData, rightData, ecuType, getDisplaySettings]);

  const axisLabels = useMemo(() => (selected ? resolveAxisLabels(selected.map) : null), [selected]);

  // Plus grand écart absolu de la map : échelle d'intensité des couleurs
  const maxAbsDelta = useMemo(() => {
    if (!detail) return 0;
    let max = 0;
    for (let r = 0; r < detail.rows; r++) {
      for (let c = 0; c < detail.cols; c++) {
        const d = Math.abs((detail.right[r]?.[c] ?? 0) - detail.left[r][c]);
        if (d > max) max = d;
      }
    }
    return max;
  }, [detail]);

  // Navigation entre maps modifiées (boutons + flèches gauche/droite)
  const changedIndex = changedSummaries.findIndex((s) => s.map.address === selectedAddress);
  const goToChanged = useCallback((delta: number) => {
    if (changedSummaries.length === 0) return;
    const base = changedIndex === -1 ? (delta > 0 ? -1 : 0) : changedIndex;
    const next = (base + delta + changedSummaries.length) % changedSummaries.length;
    setSelectedAddress(changedSummaries[next].map.address);
    setHovered(null);
  }, [changedSummaries, changedIndex]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement) return;
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        goToChanged(-1);
      } else if (e.key === "ArrowRight") {
        e.preventDefault();
        goToChanged(1);
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [goToChanged]);

  // Garder la map sélectionnée visible dans la liste
  useEffect(() => {
    if (selectedAddress === null) return;
    const el = listRef.current?.querySelector(`[data-map-address="${selectedAddress}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [selectedAddress]);

  const cellText = (a: number, b: number, decimals: number): string => {
    switch (view) {
      case "left":
        return a.toFixed(decimals);
      case "right":
        return b.toFixed(decimals);
      case "diff":
        return cellsDiffer(a, b) ? fmtSigned(b - a, decimals) : "0";
      case "pct": {
        if (!cellsDiffer(a, b)) return "0";
        const pct = percentChange(a, b);
        return pct === null ? "—" : `${fmtSigned(pct, 1)}%`;
      }
      default:
        return b.toFixed(decimals);
    }
  };

  const cellStyle = (a: number, b: number): React.CSSProperties => {
    if (!cellsDiffer(a, b)) {
      return { color: view === "diff" || view === "pct" ? mutedColor : textColor };
    }
    const palette = light ? DIFF_RGB.light : DIFF_RGB.dark;
    const [r, g, bl] = b > a ? palette.up : palette.down;
    const ratio = maxAbsDelta > 0 ? Math.abs(b - a) / maxAbsDelta : 1;
    const alpha = 0.12 + 0.38 * ratio;
    return {
      color: `rgb(${r}, ${g}, ${bl})`,
      background: `rgba(${r}, ${g}, ${bl}, ${alpha.toFixed(3)})`,
      fontWeight: 600,
    };
  };

  const axisCell = (leftLabel: string | undefined, rightLabel: string | undefined, isIndex: boolean) => {
    const changed = leftLabel !== rightLabel;
    const shown = view === "left" ? leftLabel : rightLabel;
    return {
      text: isIndex ? "·" : (shown ?? ""),
      title: changed ? `${leftName}: ${leftLabel ?? "?"} → ${rightName}: ${rightLabel ?? "?"}` : undefined,
      style: changed ? { background: AXIS_CHANGED_BG, fontWeight: 700 } : undefined,
    };
  };

  const segmentButton = (value: ValueView, label: string) => (
    <button
      key={value}
      type="button"
      onClick={() => setView(value)}
      title={label}
      className={`h-7 px-3 text-xs rounded transition-colors max-w-[140px] truncate ${
        view === value ? "bg-blue-600/40" : hoverClass
      }`}
      style={{ color: textColor }}
    >
      {label}
    </button>
  );

  const hoveredInfo = (() => {
    if (!detail || !hovered) return null;
    const a = detail.left[hovered.row]?.[hovered.col];
    const b = detail.right[hovered.row]?.[hovered.col];
    if (a === undefined || b === undefined) return null;
    const pct = percentChange(a, b);
    const y = detail.yAxisIsIndex ? `#${hovered.row + 1}` : detail.leftYLabels[hovered.row];
    const x = detail.xAxisIsIndex ? `#${hovered.col + 1}` : detail.leftXLabels[hovered.col];
    const yUnit = axisLabels?.yUnit ? ` ${axisLabels.yUnit}` : "";
    const xUnit = axisLabels?.xUnit ? ` ${axisLabels.xUnit}` : "";
    const delta = cellsDiffer(a, b)
      ? ` (${fmtSigned(b - a, detail.decimals)}${pct !== null ? `, ${fmtSigned(pct, 1)}%` : ""})`
      : ` (${t.compare.unchanged})`;
    return `Y ${y}${yUnit} · X ${x}${xUnit} — ${a.toFixed(detail.decimals)} → ${b.toFixed(detail.decimals)}${delta}`;
  })();

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <div className="flex-1 flex overflow-hidden">
        {/* Liste des maps */}
        <div className="flex flex-col flex-shrink-0" style={{ width: 260, borderRight: `1px solid ${hairline}` }}>
          <div className="p-2 flex flex-col gap-1.5" style={{ borderBottom: `1px solid ${hairline}` }}>
            <div
              className="flex items-center gap-1.5 h-7 px-2 rounded"
              style={{
                background: light ? "#f1f3f5" : "rgba(255,255,255,0.06)",
                border: `1px solid ${light ? "#dee2e6" : "rgba(255,255,255,0.12)"}`,
              }}
            >
              <Search className="w-3 h-3 flex-shrink-0" style={{ color: mutedColor }} />
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={t.compare.searchMap}
                className="flex-1 min-w-0 bg-transparent outline-none text-[11px] select-text"
                style={{ color: textColor }}
              />
            </div>
            <label className="flex items-center gap-1.5 text-[11px] cursor-pointer" style={{ color: textColor }}>
              <input type="checkbox" checked={changedOnly} onChange={(e) => setChangedOnly(e.target.checked)} />
              {t.compare.changedOnly}
              <span className="ml-auto font-mono" style={{ color: mutedColor }}>
                {changedSummaries.length} / {summaries.length}
              </span>
            </label>
          </div>
          <div ref={listRef} className="flex-1 overflow-y-auto p-1">
            {maps.length === 0 ? (
              <div className="p-3 text-[11px] text-center" style={{ color: mutedColor }}>{t.compare.noMaps}</div>
            ) : visibleSummaries.length === 0 ? (
              <div className="p-3 text-[11px] text-center" style={{ color: mutedColor }}>
                {changedOnly && !query ? t.compare.noChangedMaps : t.compare.noMapMatch}
              </div>
            ) : (
              visibleSummaries.map((s) => {
                const isSelected = s.map.address === selectedAddress;
                const changed = s.changedCells > 0 || s.axisChanged;
                const cb = typeof s.map.codeblock_id === "number" ? `CB${s.map.codeblock_id}` : "";
                return (
                  <button
                    key={s.map.address}
                    type="button"
                    data-map-address={s.map.address}
                    onClick={() => { setSelectedAddress(s.map.address); setHovered(null); }}
                    onDoubleClick={() => onOpenMap?.(s.map.address)}
                    className={`w-full text-left px-2 py-1 rounded flex items-center gap-2 transition-colors ${hoverClass} ${
                      isSelected ? (light ? "bg-black/10" : "bg-white/10") : ""
                    }`}
                    style={{ color: changed ? textColor : mutedColor }}
                  >
                    <div className="flex-1 min-w-0">
                      <div className="text-[11px] truncate" title={s.map.name}>
                        {s.map.name} {cb && <span style={{ color: mutedColor }}>[{cb}]</span>}
                      </div>
                      <div className="text-[9px] font-mono" style={{ color: mutedColor }}>
                        {s.map.address.toString(16).toUpperCase().padStart(5, "0")}
                        {s.axisChanged && <span className="ml-1.5" style={{ color: "#f59e0b" }}>{t.compare.axisChanged}</span>}
                      </div>
                    </div>
                    {s.changedCells > 0 && (
                      <span
                        className="text-[10px] font-mono px-1.5 rounded flex-shrink-0"
                        style={{ background: "rgba(239, 68, 68, 0.2)", color: light ? "#991b1b" : "#fca5a5" }}
                        title={`${s.changedCells} / ${s.totalCells} ${t.compare.cellsChanged}`}
                      >
                        {s.changedCells}
                      </span>
                    )}
                  </button>
                );
              })
            )}
          </div>
        </div>

        {/* Map sélectionnée */}
        <div className="flex-1 flex flex-col min-w-0">
          {selected && detail ? (
            <>
              <div className="px-3 py-1.5 flex items-center gap-3 flex-shrink-0" style={{ borderBottom: `1px solid ${hairline}` }}>
                <div className="min-w-0 flex-1">
                  <div className="text-[12px] font-semibold truncate" style={{ color: textColor }} title={selected.map.name}>
                    {selected.map.name}
                  </div>
                  <div className="text-[10px] font-mono" style={{ color: mutedColor }}>
                    0x{selected.map.address.toString(16).toUpperCase()} · {detail.rows}×{detail.cols}
                    {" · "}{selected.changedCells} / {selected.totalCells} {t.compare.cellsChanged}
                    {selected.maxIncreasePct > 0 && <span style={{ color: light ? "#c62828" : "#ff5252" }}> · max {fmtSigned(selected.maxIncreasePct, 1)}%</span>}
                    {selected.maxDecreasePct < 0 && <span style={{ color: light ? "#1565c0" : "#4da3ff" }}> · min {fmtSigned(selected.maxDecreasePct, 1)}%</span>}
                  </div>
                </div>
                <div
                  className="flex items-center rounded-lg px-1 gap-0.5 flex-shrink-0"
                  style={{ background: light ? "#f1f3f5" : "rgba(255,255,255,0.07)", border: `1px solid ${light ? "rgba(0,0,0,0.12)" : "rgba(255,255,255,0.12)"}` }}
                >
                  {segmentButton("left", leftName)}
                  {segmentButton("right", rightName)}
                  {segmentButton("diff", t.compare.viewDiff)}
                  {segmentButton("pct", t.compare.viewPercent)}
                  {segmentButton("3d", "3D")}
                </div>
                {onOpenMap && (
                  <button
                    type="button"
                    onClick={() => onOpenMap(selected.map.address)}
                    className={`p-1.5 rounded transition-colors flex-shrink-0 ${hoverClass}`}
                    style={{ color: mutedColor, border: buttonBorder }}
                    title={t.compare.openInEditor}
                  >
                    <ExternalLink className="w-3.5 h-3.5" />
                  </button>
                )}
              </div>

              {view === "3d" ? (
                <MapCompare3D
                  detail={detail}
                  mapAddress={selected.map.address}
                  axisLabels={axisLabels}
                  leftName={leftName}
                  rightName={rightName}
                  theme={theme}
                  hairline={hairline}
                />
              ) : (
              /* Grille : axe X en haut, axe Y à gauche (orientation de l'éditeur) */
              <div className="flex-1 overflow-auto p-2" onMouseLeave={() => setHovered(null)}>
                <table className="font-mono text-[11px] border-collapse" style={{ color: textColor }}>
                  <thead>
                    <tr>
                      <th
                        className="sticky top-0 left-0 z-20 px-1.5 text-[9px] font-normal text-left"
                        style={{ background: light ? "#e9ecef" : "#23262f", color: mutedColor }}
                        title={axisLabels ? `Y: ${axisLabels.yLabel}\nX: ${axisLabels.xLabel}` : undefined}
                      >
                        {axisLabels?.yUnit || "Y"} \ {axisLabels?.xUnit || "X"}
                      </th>
                      {Array.from({ length: detail.cols }, (_, c) => {
                        const ax = axisCell(detail.leftXLabels[c], detail.rightXLabels[c], detail.xAxisIsIndex);
                        return (
                          <th
                            key={c}
                            className="sticky top-0 z-10 px-1.5 py-0.5 font-semibold text-center"
                            style={{
                              background: light ? "#e9ecef" : "#23262f",
                              outline: hovered?.col === c ? "1px solid #3b82f6" : undefined,
                              ...ax.style,
                            }}
                            title={ax.title}
                          >
                            {ax.text}
                          </th>
                        );
                      })}
                    </tr>
                  </thead>
                  <tbody>
                    {detail.left.map((row, r) => {
                      const ax = axisCell(detail.leftYLabels[r], detail.rightYLabels[r], detail.yAxisIsIndex);
                      return (
                        <tr key={r}>
                          <th
                            className="sticky left-0 z-10 px-1.5 font-semibold text-right"
                            style={{
                              background: light ? "#e9ecef" : "#23262f",
                              outline: hovered?.row === r ? "1px solid #3b82f6" : undefined,
                              ...ax.style,
                            }}
                            title={ax.title}
                          >
                            {ax.text}
                          </th>
                          {row.map((a, c) => {
                            const b = detail.right[r]?.[c] ?? a;
                            const isHovered = hovered?.row === r && hovered?.col === c;
                            return (
                              <td
                                key={c}
                                className="px-1.5 text-right whitespace-nowrap"
                                style={{
                                  ...cellStyle(a, b),
                                  border: `1px solid ${light ? "rgba(0,0,0,0.06)" : "rgba(255,255,255,0.05)"}`,
                                  outline: isHovered ? "1px solid #3b82f6" : undefined,
                                  height: 18,
                                }}
                                onMouseEnter={() => setHovered({ row: r, col: c })}
                              >
                                {cellText(a, b, detail.decimals)}
                              </td>
                            );
                          })}
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
              )}
              <div
                className="px-3 py-1 text-[10px] font-mono truncate flex-shrink-0"
                style={{ borderTop: `1px solid ${hairline}`, color: mutedColor, minHeight: 22 }}
              >
                {view === "3d" ? t.compare.sync3DHint : (hoveredInfo ?? t.compare.hoverHint)}
              </div>
            </>
          ) : (
            <div className="flex-1 flex items-center justify-center text-[12px]" style={{ color: mutedColor }}>
              {maps.length === 0 ? t.compare.noMaps : t.compare.selectMap}
            </div>
          )}
        </div>
      </div>

      {/* Navigation entre maps modifiées */}
      <div
        className="px-4 py-2 flex items-center justify-center gap-3 flex-shrink-0"
        style={{ borderTop: `1px solid ${hairline}`, background: light ? "rgba(0, 0, 0, 0.03)" : "rgba(255, 255, 255, 0.04)" }}
      >
        <button
          onClick={onBack}
          className={`p-1 rounded transition-colors ${hoverClass}`}
          style={{ color: light ? "#666666" : "#999999", border: buttonBorder }}
          title={t.compare.backToSelection}
        >
          <ChevronLeft className="w-4 h-4" />
        </button>
        <button
          onClick={() => goToChanged(-1)}
          disabled={changedSummaries.length === 0}
          className={`p-1 rounded transition-colors disabled:opacity-50 ${hoverClass}`}
          style={{ color: light ? "#666666" : "#999999", border: buttonBorder }}
          title={t.compare.previousMap}
        >
          <ChevronLeft className="w-4 h-4" />
        </button>
        <span className="text-[11px] font-mono min-w-[160px] text-center" style={{ color: textColor }}>
          {changedSummaries.length > 0
            ? `${changedIndex >= 0 ? changedIndex + 1 : "–"} / ${changedSummaries.length} ${t.compare.mapsChanged}`
            : t.compare.noChangedMaps}
        </span>
        <button
          onClick={() => goToChanged(1)}
          disabled={changedSummaries.length === 0}
          className={`p-1 rounded transition-colors disabled:opacity-50 ${hoverClass}`}
          style={{ color: light ? "#666666" : "#999999", border: buttonBorder }}
          title={t.compare.nextMap}
        >
          <ChevronRight className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}
