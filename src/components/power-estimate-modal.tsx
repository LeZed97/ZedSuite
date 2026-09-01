"use client";

// Virtual dyno — estimates the full-load power/torque curves of a project
// version from its injection maps (see lib/power-estimation.ts for the
// physical model: driver wish → limiters → boost/air → injector duration).
// Each codeblock (EDC15) or driver-wish map (EDC16) is a selectable curve;
// the sheet can be exported as a PDF dyno report with the project info.

import { useEffect, useMemo, useState } from "react";
import {
  ResponsiveContainer,
  ComposedChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
} from "recharts";
import { X, Gauge, FileDown } from "lucide-react";
import { MODAL_GLASS, MODAL_GLASS_LIGHT } from "@/lib/modal-glass";
import { useThemeOptional } from "@/contexts/theme-context";
import { StyledSelect } from "@/components/styled-select";
import { useI18n } from "@/contexts/i18n-context";
import { useSettings } from "@/contexts/settings-context";
import * as store from "@/lib/local/store";
import type { FileRecord, Version } from "@/lib/types";
import {
  computePowerCurves,
  guessEnginePreset,
  ENGINE_PRESETS,
  NOZZLE_PRESETS,
  DEFAULT_EFFICIENCY,
  type SourceCurveResult,
} from "@/lib/power-estimation";
import { exportPowerPdf, type PdfCurve, type PdfTheme } from "@/lib/power-pdf";

interface PowerEstimateModalProps {
  file: FileRecord;
  onClose: () => void;
}

// Une teinte par codeblock/carte — mêmes familles sur l'écran et le PDF
const SOURCE_COLORS = [
  { dark: "#f87171", light: "#dc2626" },
  { dark: "#60a5fa", light: "#2563eb" },
  { dark: "#34d399", light: "#059669" },
  { dark: "#fbbf24", light: "#d97706" },
  { dark: "#c084fc", light: "#9333ea" },
];

export function PowerEstimateModal({ file, onClose }: PowerEstimateModalProps) {
  const { t, language } = useI18n();
  const { settings } = useSettings();
  // Suit le thème de l'écran hôte (dashboard)
  const themeCtx = useThemeOptional();
  const L = (themeCtx?.theme ?? "default") === "light";

  const [versions, setVersions] = useState<Version[]>([]);
  const [versionId, setVersionId] = useState<string>("");
  const [preset, setPreset] = useState<string>(() =>
    guessEnginePreset(file.engine_type, file.ecu_type)
  );
  const [efficiency, setEfficiency] = useState<number>(DEFAULT_EFFICIENCY);
  const [nozzle, setNozzle] = useState<string>("stock");
  const [results, setResults] = useState<SourceCurveResult[]>([]);
  const [selected, setSelected] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Pré-export : renommage du projet et des courbes affichées sur le PDF
  const [exportOpen, setExportOpen] = useState(false);
  const [pdfProject, setPdfProject] = useState("");
  const [pdfNames, setPdfNames] = useState<Record<string, string>>({});
  const [pdfTheme, setPdfTheme] = useState<PdfTheme>("dark");

  // Load versions once
  useEffect(() => {
    store
      .listVersions(file.id)
      .then((v) => {
        setVersions(v);
        const current = v.find((x) => x.is_current) || v[0];
        if (current) setVersionId(current.id);
      })
      .catch(() => setError("versions"));
  }, [file.id]);

  // Recompute whenever version / engine / efficiency / nozzles change
  useEffect(() => {
    if (!versionId) return;
    let cancelled = false;
    setLoading(true);
    setError(null);

    (async () => {
      try {
        const binary = await store.readBinary(file.id);
        if (!binary) throw new Error("no_binary");

        // Reconstruct the selected version: original + binary patches;
        // map-cell edits are overlaid in display units by the estimator.
        // The untouched original anchors the injector-flow model.
        const version = versions.find((v) => v.id === versionId);
        let bytes = binary;
        let edits: Array<{ map_address: number; payload?: any }> = [];
        if (version && version.name !== "Ori") {
          edits = await store.listMapEdits(versionId);
          bytes = new Uint8Array(binary);
          for (const edit of edits) {
            if (edit.map_address === -1 && edit.payload?.type === "binary") {
              for (const c of edit.payload.changes || []) {
                if (c.address >= 0 && c.address < bytes.length) {
                  bytes[c.address] = c.newValue & 0xff;
                }
              }
            }
          }
        }

        const detection =
          typeof file.detection_data === "string"
            ? JSON.parse(file.detection_data)
            : file.detection_data;
        const maps = detection?.maps || [];

        const engine =
          ENGINE_PRESETS.find((p) => p.id === preset) ?? ENGINE_PRESETS[1];
        const noz =
          NOZZLE_PRESETS.find((n) => n.id === nozzle) ?? NOZZLE_PRESETS[0];
        const curves = computePowerCurves(
          bytes,
          binary,
          maps,
          file.ecu_type || "",
          {
            cylinders: engine.cylinders,
            displacement: engine.displacement,
            efficiency,
            nozzleFactor: noz.factor,
            nozzleCeilingMg: noz.ceilingMg,
          },
          edits
        );
        if (!cancelled) {
          if (curves.length === 0) setError("no_map");
          setResults(curves);
          setSelected((prev) => {
            const valid = prev.filter((id) =>
              curves.some((c) => c.source.id === id)
            );
            return valid.length > 0 ? valid : curves.map((c) => c.source.id);
          });
        }
      } catch {
        if (!cancelled) setError("load");
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [versionId, preset, efficiency, nozzle, versions, file]);

  const shown = useMemo(
    () => results.filter((r) => selected.includes(r.source.id)),
    [results, selected]
  );
  const anyIqBased = results.some((r) => !r.torqueBased);
  // EDC15VM/V (pompe rotative VP37/VP44) : pas d'injecteurs-pompe, le choix
  // des nez n'a pas de sens — le sélecteur est masqué et le calcul reste
  // sur « stock » (facteur 1, pas de plafond).
  const ecuUpper = (file.ecu_type || "").toUpperCase();
  const isVmPump = ecuUpper.includes("EDC15VM") || ecuUpper.includes("EDC15V");

  const colorOf = (sourceId: string) => {
    const idx = results.findIndex((r) => r.source.id === sourceId);
    const c = SOURCE_COLORS[Math.max(0, idx) % SOURCE_COLORS.length];
    return L ? c.light : c.dark;
  };
  // Couleurs du PDF : plus saturées que l'affichage écran pour rester
  // bien distinctes sur papier, selon le thème choisi à l'export
  const PDF_CURVE_COLORS = [
    { dark: "#ef4444", light: "#dc2626" },
    { dark: "#3b82f6", light: "#2563eb" },
    { dark: "#10b981", light: "#059669" },
    { dark: "#f59e0b", light: "#d97706" },
    { dark: "#a855f7", light: "#9333ea" },
  ];
  const pdfColorOf = (sourceId: string) => {
    const idx = results.findIndex((r) => r.source.id === sourceId);
    const c = PDF_CURVE_COLORS[Math.max(0, idx) % PDF_CURVE_COLORS.length];
    return pdfTheme === "light" ? c.light : c.dark;
  };

  // Merge the curves on a common RPM key for recharts
  const chartData = useMemo(() => {
    const byRpm = new Map<number, Record<string, number>>();
    for (const r of shown) {
      for (const p of r.points) {
        const row = byRpm.get(p.rpm) ?? { rpm: p.rpm };
        row[`hp_${r.source.id}`] = Math.round(p.powerHp);
        row[`nm_${r.source.id}`] = Math.round(p.torqueNm);
        byRpm.set(p.rpm, row);
      }
    }
    return [...byRpm.values()].sort((a, b) => a.rpm - b.rpm);
  }, [shown]);

  const unrealistic = shown.find(
    (r) => !r.torqueBased && !r.hasBoostData && r.peakPower.fuel > 80
  );

  const toggleSource = (id: string) => {
    setSelected((prev) =>
      prev.includes(id)
        ? prev.length > 1
          ? prev.filter((x) => x !== id)
          : prev
        : [...prev, id]
    );
  };

  const openExportDialog = () => {
    if (shown.length === 0) return;
    setPdfProject(file.project_name || file.original_name || "");
    setPdfNames(
      Object.fromEntries(shown.map((r) => [r.source.id, r.source.label]))
    );
    setExportOpen(true);
  };

  const handleExportPdf = async () => {
    if (shown.length === 0 || exporting) return;
    setExporting(true);
    try {
      const engine =
        ENGINE_PRESETS.find((p) => p.id === preset) ?? ENGINE_PRESETS[1];
      const versionName =
        versions.find((v) => v.id === versionId)?.name ?? "";
      const d = t.dashboard;
      // Le nom du fichier binaire ne figure jamais sur le PDF client.
      // Ordre imposé de la carte : Projet seul, puis ECU+Version, puis HW+SW,
      // puis les infos véhicule quand elles sont renseignées.
      const infoPairs: Array<[string, string]> = [
        [d.pdfProject, pdfProject.trim() || file.project_name || ""],
        [d.pdfEcu, file.ecu_type || ""],
        [d.pdfVersion, versionName],
        ["HW", file.hardware_version || ""],
        ["SW", file.software_version || ""],
        [d.pdfVehicle, [file.vehicle_brand, file.vehicle_model].filter(Boolean).join(" ")],
        [d.pdfEngine, file.engine_type || ""],
        [d.pdfYear, file.year || ""],
        [d.pdfStage, file.stage || ""],
      ].filter(([, v]) => v !== "") as Array<[string, string]>;

      const curves: PdfCurve[] = shown.map((r) => ({
        label: pdfNames[r.source.id]?.trim() || r.source.label,
        color: pdfColorOf(r.source.id),
        points: r.points.map((p) => ({
          rpm: p.rpm,
          hp: p.powerHp,
          nm: p.torqueNm,
        })),
        peakHp: r.peakPower.powerHp,
        peakHpRpm: r.peakPower.rpm,
        peakNm: r.peakTorque.torqueNm,
        peakNmRpm: r.peakTorque.rpm,
        boostMax: r.hasBoostData
          ? Math.max(...r.points.map((p) => p.boostMbar ?? 0))
          : null,
      }));

      const paramsParts = [
        engine.label,
        `${d.powerEfficiency} ${Math.round(efficiency * 100)} %`,
      ];

      const safeName = (pdfProject.trim() || file.project_name || "projet")
        .replace(/[\\/:*?"<>|]/g, "_")
        .slice(0, 60);
      const ok = await exportPowerPdf(
        {
          title: d.powerTitle,
          clientLabel: d.pdfClient,
          clientName: file.customer || "",
          infoPairs,
          curves,
          rpmLabel: d.unitRpm,
          hpLabel: d.unitPower,
          nmLabel: "Nm",
          locale: language === "FR" ? "fr-FR" : "en-GB",
          peaksTitle: d.pdfPeaks,
          peaksHeader: [d.pdfCurve, d.pdfPower, d.pdfTorque, d.pdfBoostMax],
          paramsLabel: d.pdfParams,
          paramsLine: paramsParts.join(" — "),
          disclaimer: d.powerDisclaimer,
          footer: d.pdfFooter,
          dateLabel: d.pdfDate,
          theme: pdfTheme,
          companyName: settings.companyName,
          tunerLabel: d.pdfTuner,
        },
        `${safeName} - dyno.pdf`
      );
      if (ok) setExportOpen(false);
    } finally {
      setExporting(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-[80] flex items-center justify-center backdrop-blur-sm"
      style={{ backgroundColor: "#000000a2", animation: "backdropFadeIn 0.2s ease-out forwards" }}
      onClick={onClose}
    >
      <div
        className="relative w-full max-w-5xl mx-4 border rounded-lg p-6 max-h-[92vh] overflow-y-auto"
        style={{ ...(L ? MODAL_GLASS_LIGHT : MODAL_GLASS), animation: "modalExpand 0.2s ease-out forwards" }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-start justify-between mb-4">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-red-600 via-red-500 to-orange-500 flex items-center justify-center shadow-lg shadow-red-500/25">
              <Gauge className="w-5 h-5 text-white" />
            </div>
            <div>
              <h2 className={`text-lg font-semibold leading-tight ${L ? "text-slate-900" : "text-white"}`}>
                {t.dashboard.powerTitle}
              </h2>
              <p className="text-xs text-slate-400 truncate max-w-md">
                {file.project_name || file.original_name}
                {file.customer ? ` — ${file.customer}` : ""}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            {!loading && !error && shown.length > 0 && (
              <button
                onClick={openExportDialog}
                disabled={exporting}
                className={`flex items-center gap-2 px-3 py-2 rounded-lg text-sm font-medium transition-colors border ${
                  L
                    ? "border-blue-600/30 bg-blue-600/10 text-blue-700 hover:bg-blue-600/20"
                    : "border-blue-500/30 bg-blue-500/10 text-blue-300 hover:bg-blue-500/20"
                } ${exporting ? "opacity-60 cursor-wait" : ""}`}
              >
                <FileDown className="w-4 h-4" />
                {t.dashboard.powerExportPdf}
              </button>
            )}
            <button
              onClick={onClose}
              className={`p-2 rounded-lg transition-colors ${L ? "hover:bg-black/5" : "hover:bg-white/5"}`}
              style={{ color: L ? "rgba(0,0,0,0.5)" : "rgba(255,255,255,0.6)" }}
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Controls */}
        <div className="flex flex-wrap items-center gap-4 mb-3">
          <div className="flex items-center gap-2">
            <span className="text-sm text-slate-400">{t.dashboard.powerVersion}:</span>
            <StyledSelect
              appearance="auto"
              value={versionId}
              onChange={setVersionId}
              minWidth={120}
              options={versions.map((v) => ({ value: v.id, label: v.name }))}
            />
          </div>
          <div className="flex items-center gap-2">
            <span className="text-sm text-slate-400">{t.dashboard.powerEngine}:</span>
            <StyledSelect
              appearance="auto"
              value={preset}
              onChange={setPreset}
              minWidth={170}
              options={ENGINE_PRESETS.map((p) => ({ value: p.id, label: p.label }))}
            />
          </div>
          <div className="flex items-center gap-2">
            <span className="text-sm text-slate-400">{t.dashboard.powerEfficiency}:</span>
            <StyledSelect
              appearance="auto"
              value={String(efficiency)}
              onChange={(v) => setEfficiency(Number(v))}
              minWidth={150}
              options={[
                { value: "0.36", label: t.dashboard.powerEffWorn },
                { value: "0.38", label: t.dashboard.powerEffCautious },
                { value: "0.4", label: t.dashboard.powerEffNominal },
                { value: "0.42", label: t.dashboard.powerEffOptimistic },
              ]}
            />
          </div>
          {anyIqBased && !isVmPump && (
            <div className="flex items-center gap-2">
              <span className="text-sm text-slate-400">{t.dashboard.powerNozzle}:</span>
              <StyledSelect
                appearance="auto"
                value={nozzle}
                onChange={setNozzle}
                minWidth={110}
                options={NOZZLE_PRESETS.map((n) => ({ value: n.id, label: n.label }))}
              />
            </div>
          )}
        </div>

        {/* Sélection des codeblocks / cartes */}
        {results.length > 1 && (
          <div className="flex flex-wrap items-center gap-2 mb-4">
            <span className="text-sm text-slate-400">{t.dashboard.powerSources}:</span>
            {results.map((r) => {
              const active = selected.includes(r.source.id);
              const color = colorOf(r.source.id);
              return (
                <button
                  key={r.source.id}
                  onClick={() => toggleSource(r.source.id)}
                  className={`flex items-center gap-2 px-3 py-1.5 rounded-full border text-xs font-medium transition-all ${
                    active
                      ? L
                        ? "border-black/20 bg-black/[0.06] text-slate-900"
                        : "border-white/25 bg-white/[0.08] text-white"
                      : L
                        ? "border-black/[0.08] text-slate-400 hover:border-black/15"
                        : "border-white/[0.08] text-slate-500 hover:border-white/15"
                  }`}
                >
                  <span
                    className="w-2.5 h-2.5 rounded-full"
                    style={{ backgroundColor: color, opacity: active ? 1 : 0.35 }}
                  />
                  {r.source.label}
                </button>
              );
            })}
          </div>
        )}

        {/* Body */}
        {loading ? (
          <div className="h-72 flex items-center justify-center">
            <div className={`loader loader-lg${L ? " loader-light" : ""}`} />
          </div>
        ) : error || shown.length === 0 ? (
          <div className="h-72 flex items-center justify-center text-center px-8">
            <p className="text-slate-400">{t.dashboard.powerNoData}</p>
          </div>
        ) : (
          <>
            {/* Peaks */}
            {shown.length === 1 ? (
              <div className="grid grid-cols-2 gap-4 mb-4">
                <div className={`rounded-xl border px-4 py-3 ${L ? "border-black/[0.08] bg-black/[0.03]" : "border-white/[0.08] bg-white/[0.03]"}`}>
                  <p className="text-xs text-slate-400 mb-0.5">{t.dashboard.powerPeak}</p>
                  <p className={`text-2xl font-bold tabular-nums ${L ? "text-slate-900" : "text-white"}`}>
                    ≈ {Math.round(shown[0].peakPower.powerHp)} <span className="text-base font-medium">ch</span>
                    <span className="text-sm font-normal text-slate-500 ml-2">
                      ({Math.round(shown[0].peakPower.powerKw)} kW) @ {shown[0].peakPower.rpm} tr/min
                    </span>
                  </p>
                </div>
                <div className={`rounded-xl border px-4 py-3 ${L ? "border-black/[0.08] bg-black/[0.03]" : "border-white/[0.08] bg-white/[0.03]"}`}>
                  <p className="text-xs text-slate-400 mb-0.5">{t.dashboard.torquePeak}</p>
                  <p className={`text-2xl font-bold tabular-nums ${L ? "text-slate-900" : "text-white"}`}>
                    ≈ {Math.round(shown[0].peakTorque.torqueNm)} <span className="text-base font-medium">Nm</span>
                    <span className="text-sm font-normal text-slate-500 ml-2">
                      @ {shown[0].peakTorque.rpm} tr/min
                    </span>
                  </p>
                </div>
              </div>
            ) : (
              <div className={`flex flex-nowrap items-center justify-between gap-x-4 overflow-x-auto rounded-xl border px-4 py-2.5 mb-4 ${L ? "border-black/[0.08] bg-black/[0.03]" : "border-white/[0.08] bg-white/[0.03]"}`}>
                {shown.map((r) => (
                  <div key={r.source.id} className="flex items-center gap-2 text-xs whitespace-nowrap">
                    <span className="w-2.5 h-2.5 rounded-full shrink-0" style={{ backgroundColor: colorOf(r.source.id) }} />
                    <span className={`font-medium ${L ? "text-slate-700" : "text-slate-300"}`}>
                      {r.source.label}
                    </span>
                    <span className={`tabular-nums font-bold ${L ? "text-slate-900" : "text-white"}`}>
                      ≈ {Math.round(r.peakPower.powerHp)} ch
                      <span className="font-normal text-slate-500">@{r.peakPower.rpm}</span>
                      {" · "}{Math.round(r.peakTorque.torqueNm)} Nm
                    </span>
                  </div>
                ))}
              </div>
            )}

            {/* Curve */}
            <div className={`h-80 rounded-xl border pt-3 pr-2 ${L ? "border-black/[0.06] bg-white/40" : "border-white/[0.06] bg-black/20"}`}>
              <ResponsiveContainer width="100%" height="100%">
                <ComposedChart data={chartData} margin={{ top: 5, right: 10, bottom: 5, left: 0 }}>
                  <CartesianGrid stroke={L ? "rgba(0,0,0,0.08)" : "rgba(255,255,255,0.06)"} strokeDasharray="3 3" />
                  <XAxis
                    dataKey="rpm"
                    type="number"
                    domain={["dataMin", "dataMax"]}
                    tick={{ fill: "#94a3b8", fontSize: 11 }}
                    stroke="rgba(255,255,255,0.15)"
                    tickCount={8}
                  />
                  <YAxis
                    yAxisId="hp"
                    tick={{ fill: L ? "#b91c1c" : "#f87171", fontSize: 11 }}
                    stroke="rgba(255,255,255,0.15)"
                    width={40}
                  />
                  <YAxis
                    yAxisId="nm"
                    orientation="right"
                    tick={{ fill: L ? "#1d4ed8" : "#60a5fa", fontSize: 11 }}
                    stroke="rgba(255,255,255,0.15)"
                    width={40}
                  />
                  <Tooltip
                    contentStyle={{
                      backgroundColor: L ? "rgba(255,255,255,0.97)" : "rgba(22,25,34,0.97)",
                      border: L ? "1px solid rgba(0,0,0,0.12)" : "1px solid rgba(255,255,255,0.1)",
                      borderRadius: 8,
                      color: L ? "#0f172a" : "#fff",
                      fontSize: 12,
                    }}
                    labelFormatter={(rpm) => `${rpm} tr/min`}
                  />
                  <Legend wrapperStyle={{ fontSize: 12, color: L ? "#475569" : "#94a3b8" }} />
                  {shown.map((r) => (
                    <Line
                      key={`hp-${r.source.id}`}
                      yAxisId="hp"
                      type="monotone"
                      dataKey={`hp_${r.source.id}`}
                      name={shown.length > 1 ? `${r.source.label.replace("Codeblock ", "CB")} (ch)` : "ch"}
                      stroke={colorOf(r.source.id)}
                      strokeWidth={2.5}
                      dot={false}
                      connectNulls
                      activeDot={{ r: 4 }}
                    />
                  ))}
                  {shown.map((r) => (
                    <Line
                      key={`nm-${r.source.id}`}
                      yAxisId="nm"
                      type="monotone"
                      dataKey={`nm_${r.source.id}`}
                      name={shown.length > 1 ? `${r.source.label.replace("Codeblock ", "CB")} (Nm)` : "Nm"}
                      stroke={colorOf(r.source.id)}
                      strokeWidth={1.5}
                      strokeDasharray="6 4"
                      dot={false}
                      connectNulls
                      legendType="plainline"
                    />
                  ))}
                </ComposedChart>
              </ResponsiveContainer>
            </div>

            {/* Fichier déplafonné sans données de boost : l'estimation ne peut
                refléter que ce que le fichier demande */}
            {unrealistic && (
              <div className={`mt-3 px-3 py-2 rounded-lg border ${L ? "border-amber-600/40 bg-amber-500/15" : "border-amber-500/30 bg-amber-500/10"}`}>
                <p className={`text-[12px] leading-relaxed ${L ? "text-amber-700" : "text-amber-400"}`}>
                  ⚠ {t.dashboard.powerUnrealistic.replace("{iq}", String(Math.round(unrealistic.peakPower.fuel)))}
                </p>
              </div>
            )}

            {/* Disclaimer */}
            <p className="text-[11px] text-slate-500 mt-2 leading-relaxed">
              {t.dashboard.powerDisclaimer}
            </p>
          </>
        )}
      </div>

      {/* Boîte de pré-export : renommer le projet et les courbes du PDF */}
      {exportOpen && (
        <div
          className="fixed inset-0 z-[95] flex items-center justify-center backdrop-blur-sm"
          style={{ backgroundColor: "#000000a2" }}
          onClick={(e) => e.stopPropagation()}
        >
          <div
            className="w-full max-w-md mx-4 border rounded-lg p-5"
            style={{ ...(L ? MODAL_GLASS_LIGHT : MODAL_GLASS), animation: "modalExpand 0.15s ease-out forwards" }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center gap-2.5 mb-4">
              <FileDown className={`w-5 h-5 ${L ? "text-blue-700" : "text-blue-400"}`} />
              <h3 className={`text-base font-semibold ${L ? "text-slate-900" : "text-white"}`}>
                {t.dashboard.powerExportPdf}
              </h3>
            </div>

            <label className="block text-xs text-slate-400 mb-1.5">
              {t.dashboard.pdfDialogProject}
            </label>
            <input
              type="text"
              value={pdfProject}
              onChange={(e) => setPdfProject(e.target.value)}
              className={`w-full px-3 py-2 rounded-lg border text-sm outline-none transition-colors mb-4 ${
                L
                  ? "bg-black/[0.04] border-black/10 text-slate-900 focus:border-blue-600/50"
                  : "bg-white/[0.05] border-white/10 text-white focus:border-blue-500/50"
              }`}
            />

            <label className="block text-xs text-slate-400 mb-1.5">
              {t.dashboard.pdfDialogCurves}
            </label>
            <div className="space-y-2 mb-5">
              {shown.map((r) => (
                <div key={r.source.id} className="flex items-center gap-2.5">
                  <span
                    className="w-2.5 h-2.5 rounded-full shrink-0"
                    style={{ backgroundColor: colorOf(r.source.id) }}
                  />
                  <input
                    type="text"
                    value={pdfNames[r.source.id] ?? r.source.label}
                    onChange={(e) =>
                      setPdfNames((prev) => ({ ...prev, [r.source.id]: e.target.value }))
                    }
                    className={`flex-1 px-3 py-1.5 rounded-lg border text-sm outline-none transition-colors ${
                      L
                        ? "bg-black/[0.04] border-black/10 text-slate-900 focus:border-blue-600/50"
                        : "bg-white/[0.05] border-white/10 text-white focus:border-blue-500/50"
                    }`}
                  />
                </div>
              ))}
            </div>

            <label className="block text-xs text-slate-400 mb-1.5">
              {t.dashboard.pdfThemeLabel}
            </label>
            <div className="flex gap-2 mb-5">
              {(["dark", "light"] as const).map((th) => (
                <button
                  key={th}
                  onClick={() => setPdfTheme(th)}
                  className={`flex-1 px-3 py-2 rounded-lg border text-sm font-medium transition-colors ${
                    pdfTheme === th
                      ? L
                        ? "border-blue-600/40 bg-blue-600/10 text-blue-700"
                        : "border-blue-500/40 bg-blue-500/10 text-blue-300"
                      : L
                        ? "border-black/10 text-slate-500 hover:border-black/20"
                        : "border-white/10 text-slate-400 hover:border-white/20"
                  }`}
                >
                  {th === "dark" ? t.dashboard.pdfThemeDark : t.dashboard.pdfThemeLight}
                </button>
              ))}
            </div>

            <div className="flex justify-end gap-2">
              <button
                onClick={() => setExportOpen(false)}
                disabled={exporting}
                className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
                  L ? "text-slate-600 hover:bg-black/5" : "text-slate-300 hover:bg-white/5"
                }`}
              >
                {t.dashboard.cancel}
              </button>
              <button
                onClick={handleExportPdf}
                disabled={exporting}
                className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium border transition-colors ${
                  L
                    ? "border-blue-600/30 bg-blue-600/10 text-blue-700 hover:bg-blue-600/20"
                    : "border-blue-500/30 bg-blue-500/10 text-blue-300 hover:bg-blue-500/20"
                } ${exporting ? "opacity-60 cursor-wait" : ""}`}
              >
                <FileDown className="w-4 h-4" />
                {t.dashboard.powerExportPdf}
              </button>
            </div>
          </div>
        </div>
      )}

      <style jsx>{`
        @keyframes backdropFadeIn {
          from { opacity: 0; }
          to { opacity: 1; }
        }
        @keyframes modalExpand {
          from { opacity: 0; transform: scale(0.95); }
          to { opacity: 1; transform: scale(1); }
        }
      `}</style>
    </div>
  );
}
