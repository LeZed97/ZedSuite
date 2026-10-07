"use client";

// Fenêtre « Fin d'injection » de l'éditeur (menu Outils) : la fin
// d'injection de chaque point régime × quantité d'une map d'avance EDC15P,
// calculée comme le calculateur (durée choisie par le sélecteur). Même cadre
// flottant et même présentation que la fenêtre de puissance ; lit l'état en
// mémoire de la version ouverte, modifications non enregistrées comprises.
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { RefreshCw } from "lucide-react";
import type { FileRecord } from "@/lib/types";
import * as localStore from "@/lib/local/store";
import { useI18n } from "@/contexts/i18n-context";
import { useThemeOptional } from "@/contexts/theme-context";
import { StyledSelect } from "@/components/styled-select";
import {
  computeEoiMatrix,
  getAvailableSoiMaps,
  getEoiCodeblocks,
  type EoiCellResult,
  type EoiMatrixResult,
  type EoiStatus,
} from "@/lib/ecu/bosch/eoi-calculation";
import type { DetectedMapLite, MapEditLite } from "@/lib/power-estimation";

export interface LiveEoiSource {
  versionId: string;
  getState: () => { bytes: Uint8Array; edits: MapEditLite[] };
  refreshKey: number;
}

interface EoiModalProps {
  file: FileRecord;
  live?: LiveEoiSource;
  onMinWidthChange?: (px: number) => void;
  onContentHeightChange?: (px: number) => void;
}

const MIN_WIDTH = 720;
const STATUS_ORDER: EoiStatus[] = ["early", "ok", "late", "veryLate"];

export function EoiModal({ file, live, onMinWidthChange, onContentHeightChange }: EoiModalProps) {
  const liveRef = useRef<LiveEoiSource | undefined>(live);
  liveRef.current = live;
  const { t } = useI18n();
  const themeCtx = useThemeOptional();
  const L = (themeCtx?.theme ?? "default") === "light";

  const maps = useMemo<DetectedMapLite[]>(() => {
    try {
      const detection =
        typeof file.detection_data === "string" ? JSON.parse(file.detection_data) : file.detection_data;
      return detection?.maps || [];
    } catch {
      return [];
    }
  }, [file.detection_data]);

  const codeblocks = useMemo(() => getEoiCodeblocks(maps), [maps]);
  const [codeblockId, setCodeblockId] = useState<number | null>(() => codeblocks[0] ?? null);
  const soiMaps = useMemo(() => getAvailableSoiMaps(maps, codeblockId), [maps, codeblockId]);
  const [soiAddress, setSoiAddress] = useState<number | undefined>(() => soiMaps[0]?.address);
  useEffect(() => {
    if (soiMaps.length > 0 && !soiMaps.some((m) => m.address === soiAddress)) {
      setSoiAddress(soiMaps[0].address);
    }
  }, [soiMaps, soiAddress]);

  const [durationChoice, setDurationChoice] = useState<string>("auto");
  const [unit, setUnit] = useState<"atdc" | "btdc">("atdc");
  const [applyLimiter, setApplyLimiter] = useState(false);
  const [refreshTick, setRefreshTick] = useState(0);

  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<"noMaps" | "notSupported" | null>(null);
  const [result, setResult] = useState<EoiMatrixResult | null>(null);
  const [selected, setSelected] = useState<EoiCellResult | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    (async () => {
      try {
        let bytes: Uint8Array;
        let edits: MapEditLite[] = [];
        const source = liveRef.current;
        if (source) {
          const state = source.getState();
          bytes = state.bytes;
          edits = state.edits;
        } else {
          const binary = await localStore.readBinary(file.id);
          if (!binary) throw new Error("no_binary");
          bytes = new Uint8Array(binary);
        }
        const supported = (file.ecu_type || "").toUpperCase().includes("EDC15P");
        const res = supported
          ? computeEoiMatrix(bytes, maps, edits, file.ecu_type || "", {
              codeblockId,
              soiMapAddress: soiAddress,
              durationMode: durationChoice === "auto" ? "auto" : "manual",
              manualDurationIndex: durationChoice === "auto" ? 0 : parseInt(durationChoice, 10),
              applySoiLimiter: applyLimiter,
            })
          : null;
        if (cancelled) return;
        if (!res) {
          setError(supported ? "noMaps" : "notSupported");
          setResult(null);
          setSelected(null);
        } else {
          setError(null);
          setResult(res);
          // La cellule la plus tardive est celle qu'on veut voir en premier
          const point = res.maxEoiPoint;
          const cell = point
            ? res.cells.flat().find((c) => c.rpm === point.rpm && c.iq === point.iq) ?? null
            : res.cells[0]?.[0] ?? null;
          setSelected(cell);
        }
      } catch {
        if (!cancelled) {
          setError("noMaps");
          setResult(null);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [file, maps, codeblockId, soiAddress, durationChoice, applyLimiter, live?.refreshKey, refreshTick]);

  // Largeur minimale et hauteur du contenu, remontées au cadre flottant
  useEffect(() => {
    onMinWidthChange?.(MIN_WIDTH);
  }, [onMinWidthChange]);
  const contentRef = useRef<HTMLDivElement>(null);
  const lastHeightRef = useRef(0);
  useEffect(() => {
    if (!onContentHeightChange) return;
    const el = contentRef.current;
    if (!el) return;
    const report = () => {
      const h = Math.ceil(el.getBoundingClientRect().height + 32);
      if (h === lastHeightRef.current) return;
      lastHeightRef.current = h;
      onContentHeightChange(h);
    };
    report();
    const ro = typeof ResizeObserver !== "undefined" ? new ResizeObserver(report) : null;
    ro?.observe(el);
    return () => ro?.disconnect();
  }, [onContentHeightChange, result, error, loading]);

  const statusLabel = useCallback(
    (s: EoiStatus) =>
      s === "early"
        ? t.eoiModal.statusEarly
        : s === "ok"
          ? t.eoiModal.statusOk
          : s === "late"
            ? t.eoiModal.statusLate
            : t.eoiModal.statusVeryLate,
    [t],
  );
  // Teintes sobres par état : la couleur porte l'information, rien ne clignote
  const statusTint = (s: EoiStatus): string => {
    switch (s) {
      case "early":
        return L ? "rgba(59, 130, 246, 0.14)" : "rgba(96, 165, 250, 0.16)";
      case "ok":
        return L ? "rgba(34, 197, 94, 0.16)" : "rgba(34, 197, 94, 0.18)";
      case "late":
        return L ? "rgba(245, 158, 11, 0.22)" : "rgba(245, 158, 11, 0.22)";
      default:
        return L ? "rgba(239, 68, 68, 0.24)" : "rgba(239, 68, 68, 0.28)";
    }
  };
  const fmt = (eoiAtdc: number): string => {
    const v = unit === "atdc" ? eoiAtdc : -eoiAtdc;
    return `${v > 0 ? "+" : ""}${v.toFixed(1)}`;
  };
  const unitLabel = unit === "atdc" ? t.eoiModal.unitAtdc : t.eoiModal.unitBtdc;

  const border = L ? "rgba(0,0,0,0.12)" : "rgba(255,255,255,0.12)";
  const muted = L ? "rgba(0,0,0,0.55)" : "rgba(255,255,255,0.6)";
  const headerBg = L ? "rgba(0,0,0,0.04)" : "rgba(255,255,255,0.05)";
  const durationOptions = [
    { value: "auto", label: t.eoiModal.durationAuto },
    ...[0, 1, 2, 3, 4, 5].map((i) => ({ value: String(i), label: t.eoiModal.durationManual.replace("{n}", `0${i}`) })),
  ];

  return (
    <div className="w-full h-full p-4 overflow-y-auto">
      <div ref={contentRef} style={{ display: "flow-root" }} className={L ? "text-slate-800" : "text-slate-100"}>
        {/* Réglages */}
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2 mb-3">
          {codeblocks.length > 1 && (
            <div className="flex items-center gap-2">
              <span className="text-sm" style={{ color: muted }}>{t.eoiModal.codeblock}:</span>
              <StyledSelect
                appearance="auto"
                value={String(codeblockId ?? "")}
                onChange={(v) => setCodeblockId(v === "" ? null : Number(v))}
                minWidth={80}
                options={codeblocks.map((id) => ({ value: String(id ?? ""), label: String(id ?? "—") }))}
              />
            </div>
          )}
          <div className="flex items-center gap-2">
            <span className="text-sm" style={{ color: muted }}>{t.eoiModal.soiMap}:</span>
            <StyledSelect
              appearance="auto"
              value={String(soiAddress ?? "")}
              onChange={(v) => setSoiAddress(Number(v))}
              minWidth={230}
              options={soiMaps.map((m) => ({ value: String(m.address), label: m.name }))}
            />
          </div>
          <div className="flex items-center gap-2">
            <span className="text-sm" style={{ color: muted }}>{t.eoiModal.duration}:</span>
            <StyledSelect appearance="auto" value={durationChoice} onChange={setDurationChoice} minWidth={200} options={durationOptions} />
          </div>
          <div className="flex items-center gap-2">
            <span className="text-sm" style={{ color: muted }}>{t.eoiModal.unit}:</span>
            <StyledSelect
              appearance="auto"
              value={unit}
              onChange={(v) => setUnit(v === "btdc" ? "btdc" : "atdc")}
              minWidth={130}
              options={[
                { value: "atdc", label: t.eoiModal.unitAtdc },
                { value: "btdc", label: t.eoiModal.unitBtdc },
              ]}
            />
          </div>
          <label className="flex items-center gap-2 text-sm cursor-pointer select-none" style={{ color: muted }}>
            <input type="checkbox" checked={applyLimiter} onChange={(e) => setApplyLimiter(e.target.checked)} className="cursor-pointer" />
            {t.eoiModal.applyLimiter}
          </label>
          {live && (
            <button
              onClick={() => setRefreshTick((n) => n + 1)}
              disabled={loading}
              title={t.dashboard.powerRefresh}
              className={`ml-auto flex items-center gap-2 px-3 py-2 rounded-lg text-sm font-medium transition-colors border ${
                L ? "border-black/10 bg-black/5 text-slate-700 hover:bg-black/10" : "border-white/10 bg-white/5 text-slate-200 hover:bg-white/10"
              } ${loading ? "opacity-60 cursor-wait" : ""}`}
            >
              <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin" : ""}`} />
              {t.dashboard.powerRefresh}
            </button>
          )}
        </div>

        {loading && !result ? (
          <div className="py-10 text-center text-sm" style={{ color: muted }}>{t.eoiModal.computing}</div>
        ) : error ? (
          <div className="py-10 text-center text-sm" style={{ color: muted }}>
            {error === "notSupported" ? t.eoiModal.notSupported : t.eoiModal.noMaps}
          </div>
        ) : result ? (
          <>
            {/* Résumé : point le plus tardif et répartition */}
            <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 mb-3 text-sm">
              <span>
                <span style={{ color: muted }}>{t.eoiModal.maxEoi} : </span>
                <span className="font-mono font-medium">{fmt(result.maxEoiAtdc)}{unitLabel}</span>
                {result.maxEoiPoint && (
                  <span style={{ color: muted }}>
                    {" "}{t.eoiModal.at} {result.maxEoiPoint.rpm} rpm, {result.maxEoiPoint.iq} mg/st
                  </span>
                )}
              </span>
              <span style={{ color: muted }}>
                {STATUS_ORDER.filter((s) => result.stats[s] > 0)
                  .map((s) => `${result.stats[s]} ${statusLabel(s)}`)
                  .join(" · ")}{" "}
                ({result.stats.total} {t.eoiModal.cells})
              </span>
            </div>

            {/* Grille régime × quantité, régime décroissant comme dans les fenêtres de maps */}
            <div className="overflow-auto rounded-lg border" style={{ borderColor: border, maxHeight: 420 }}>
              <table className="border-collapse text-xs font-mono" style={{ minWidth: "100%" }}>
                <thead>
                  <tr style={{ background: headerBg }}>
                    <th className="px-2 py-1.5 text-left font-medium sticky left-0 z-10" style={{ background: L ? "#f3f4f6" : "#1a1d27", color: muted, borderBottom: `1px solid ${border}` }}>
                      rpm \ mg/st
                    </th>
                    {result.iqAxis.map((iq, i) => (
                      <th key={i} className="px-2 py-1.5 text-center font-medium" style={{ color: muted, borderBottom: `1px solid ${border}` }}>
                        {iq}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {result.cells.map((_, i) => result.cells.length - 1 - i).map((r) => (
                    <tr key={r}>
                      <td className="px-2 py-1 font-medium sticky left-0 z-10" style={{ background: L ? "#f3f4f6" : "#1a1d27", color: muted, borderRight: `1px solid ${border}` }}>
                        {result.rpmAxis[r]}
                      </td>
                      {result.cells[r].map((cell, c) => {
                        const isSelected = selected?.rpm === cell.rpm && selected?.iq === cell.iq;
                        return (
                          <td
                            key={c}
                            onClick={() => setSelected(cell)}
                            onMouseEnter={() => setSelected(cell)}
                            className="px-2 py-1 text-center cursor-default select-none"
                            style={{
                              background: statusTint(cell.status),
                              outline: isSelected ? `1px solid ${L ? "#000" : "#fff"}` : undefined,
                              outlineOffset: -1,
                            }}
                          >
                            {fmt(cell.eoiAtdc)}
                          </td>
                        );
                      })}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            {/* Cellule survolée : avance, durée, fin, table de durée utilisée */}
            <div className="mt-3 text-sm min-h-[1.5rem]">
              {selected && (
                <>
                  <span className="font-mono">{selected.rpm} rpm · {selected.iq} mg/st</span>
                  <span style={{ color: muted }}> — {t.eoiModal.soi} </span>
                  <span className="font-mono">{selected.soi.toFixed(2)}° {t.eoiModal.unitBtdc.replace("° ", "")}</span>
                  <span style={{ color: muted }}> · {t.eoiModal.durationValue} </span>
                  <span className="font-mono">{selected.duration.toFixed(2)}°</span>
                  <span style={{ color: muted }}> ({selected.durationSource})</span>
                  <span style={{ color: muted }}> · {t.eoiModal.eoi} </span>
                  <span className="font-mono font-medium">{fmt(selected.eoiAtdc)}{unitLabel}</span>
                  <span style={{ color: muted }}> · {statusLabel(selected.status)}</span>
                </>
              )}
            </div>

            {/* Légende et formule */}
            <div className="mt-2 flex flex-wrap items-center justify-between gap-x-4 gap-y-1 text-xs" style={{ color: muted }}>
              <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
                {STATUS_ORDER.map((s) => (
                  <span key={s} className="inline-flex items-center gap-1.5">
                    <span className="inline-block w-3 h-3 rounded-sm" style={{ background: statusTint(s), border: `1px solid ${border}` }} />
                    {statusLabel(s)}
                  </span>
                ))}
              </div>
              <span>{t.eoiModal.formula}</span>
            </div>
          </>
        ) : null}
      </div>
    </div>
  );
}
