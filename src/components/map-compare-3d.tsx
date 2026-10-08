"use client";

import { useMemo, useRef, useCallback, useEffect } from "react";
import dynamic from "next/dynamic";
import { buildPlot3DTicks, transformColorForTheme } from "@/components/map-viewer";
import { cellsDiffer, percentChange, type MapCompareDetail } from "@/lib/map-compare";
import type { MapAxisLabels } from "@/lib/map-cell-layout";

const Plot = dynamic(() => import("react-plotly.js"), {
  ssr: false,
  loading: () => null,
});

type Camera = {
  eye: { x: number; y: number; z: number };
  center: { x: number; y: number; z: number };
  up: { x: number; y: number; z: number };
};

interface MapCompare3DProps {
  detail: MapCompareDetail;
  mapAddress: number;
  axisLabels: MapAxisLabels | null;
  leftName: string;
  rightName: string;
  theme: string;
  hairline: string;
}

/**
 * Deux surfaces 3D côte à côte (gauche / droite) dont la caméra est
 * synchronisée : tourner ou zoomer l'une applique le même point de vue à
 * l'autre, en direct pendant le glisser. Même échelle Z et mêmes couleurs des
 * deux côtés pour que les hauteurs se comparent à l'œil ; les points modifiés
 * sont marqués sur la vue de droite (rouge = hausse, bleu = baisse).
 */
export function MapCompare3D({ detail, mapAddress, axisLabels, leftName, rightName, theme, hairline }: MapCompare3DProps) {
  const light = theme === "light";
  const graphs = useRef<{ left: HTMLElement | null; right: HTMLElement | null }>({ left: null, right: null });
  // Vue dont on est en train d'appliquer la caméra : son propre événement
  // relayout ne doit pas être renvoyé à la vue d'origine (boucle).
  const syncingSide = useRef<"left" | "right" | null>(null);
  const plotlyRef = useRef<typeof import("plotly.js/dist/plotly").default | null>(null);

  const prepared = useMemo(() => {
    const { leftXLabels, leftYLabels } = detail;
    // Même règle que l'éditeur : Plotly trace Y croissant, on retourne les
    // lignes quand l'axe affiché est décroissant
    const needsYReverse =
      leftYLabels.length > 1 && parseFloat(leftYLabels[0]) > parseFloat(leftYLabels[leftYLabels.length - 1]);
    const order = (rows: number[][]) => (needsYReverse ? [...rows].reverse() : rows);
    const zL = order(detail.left);
    const zR = order(detail.right);
    const yLabels = needsYReverse ? [...leftYLabels].reverse() : leftYLabels;
    const xIdx = leftXLabels.map((_, i) => i);
    const yIdx = yLabels.map((_, i) => i);

    let zMin = Infinity;
    let zMax = -Infinity;
    for (const grid of [zL, zR]) {
      for (const row of grid) {
        for (const v of row) {
          if (v < zMin) zMin = v;
          if (v > zMax) zMax = v;
        }
      }
    }
    if (!Number.isFinite(zMin)) { zMin = 0; zMax = 1; }
    if (zMin === zMax) { zMin -= 1; zMax += 1; }

    const ticks = buildPlot3DTicks(leftXLabels, leftYLabels);
    return { zL, zR, yLabels, xLabels: leftXLabels, xIdx, yIdx, zMin, zMax, ticks };
  }, [detail]);

  // Point de vue par défaut de l'éditeur : coin bas-gauche du tableau face à la caméra
  const defaultCamera = useMemo<Camera>(() => {
    const xs = detail.leftXLabels.map((l) => parseFloat(l));
    const ys = detail.leftYLabels.map((l) => parseFloat(l));
    const xAscending = xs.length < 2 || xs[0] <= xs[xs.length - 1];
    const bottomIsMin = ys.length < 2 || ys[ys.length - 1] <= ys[0];
    return {
      eye: { x: xAscending ? -1.05 : 1.05, y: bottomIsMin ? -1.05 : 1.05, z: 0.6 },
      center: { x: 0, y: 0, z: -0.15 },
      up: { x: 0, y: 0, z: 1 },
    };
  }, [detail]);

  const colorscale = useMemo(() => {
    const th = (theme === "light" || theme === "oled" ? theme : "default") as "default" | "light" | "oled";
    return [
      [0, transformColorForTheme(0, 55, 240, th)],
      [0.25, transformColorForTheme(0, 185, 0, th)],
      [0.5, transformColorForTheme(200, 165, 0, th)],
      [0.75, transformColorForTheme(220, 120, 0, th)],
      [1, transformColorForTheme(250, 0, 0, th)],
    ];
  }, [theme]);

  const buildTraces = useCallback((side: "left" | "right") => {
    const { zL, zR, xLabels, yLabels, xIdx, yIdx, zMin, zMax } = prepared;
    const z = side === "left" ? zL : zR;
    const zOffset = (zMax - zMin) * 0.001;
    const xName = axisLabels?.xLabel || "X";
    const yName = axisLabels?.yLabel || "Y";
    const hover = z.map((row, yi) =>
      row.map((_, xi) => {
        const a = zL[yi][xi];
        const b = zR[yi]?.[xi] ?? a;
        const base = `${xName}: ${xLabels[xi]}<br>${yName}: ${yLabels[yi]}<br>${leftName}: ${a.toFixed(detail.decimals)}<br>${rightName}: ${b.toFixed(detail.decimals)}`;
        if (!cellsDiffer(a, b)) return base;
        const pct = percentChange(a, b);
        const d = b - a;
        return `${base}<br>Δ ${d > 0 ? "+" : ""}${d.toFixed(detail.decimals)}${pct !== null ? ` (${pct > 0 ? "+" : ""}${pct.toFixed(1)}%)` : ""}`;
      }),
    );
    const traces: any[] = [
      {
        type: "surface",
        z,
        x: xIdx,
        y: yIdx,
        text: hover,
        hovertemplate: "%{text}<extra></extra>",
        colorscale,
        cmin: zMin,
        cmax: zMax,
        showscale: false,
        contours: { z: { show: false } },
      },
      // Maillage blanc, une ligne par cellule (comme l'éditeur)
      ...yIdx.map((yi) => ({
        type: "scatter3d", mode: "lines", x: xIdx, y: new Array(xIdx.length).fill(yi),
        z: z[yi].map((v) => v + zOffset), line: { color: "#ffffff", width: 1.3 },
        showlegend: false, hoverinfo: "skip", connectgaps: true,
      })),
      ...xIdx.map((xi) => ({
        type: "scatter3d", mode: "lines", x: new Array(yIdx.length).fill(xi), y: yIdx,
        z: z.map((row) => row[xi] + zOffset), line: { color: "#ffffff", width: 1.3 },
        showlegend: false, hoverinfo: "skip", connectgaps: true,
      })),
    ];
    if (side === "right") {
      // Points modifiés : rouge = hausse, bleu = baisse
      const up = { x: [] as number[], y: [] as number[], z: [] as number[] };
      const down = { x: [] as number[], y: [] as number[], z: [] as number[] };
      zR.forEach((row, yi) => row.forEach((b, xi) => {
        const a = zL[yi][xi];
        if (!cellsDiffer(a, b)) return;
        const target = b > a ? up : down;
        target.x.push(xi);
        target.y.push(yi);
        target.z.push(b + zOffset * 4);
      }));
      for (const [pts, color] of [[up, "#ff3b3b"], [down, "#3b8bff"]] as const) {
        if (pts.x.length === 0) continue;
        traces.push({
          type: "scatter3d", mode: "markers", ...pts,
          marker: { size: 4, color, line: { color: "#ffffff", width: 1 } },
          showlegend: false, hoverinfo: "skip",
        });
      }
    }
    return traces;
  }, [prepared, axisLabels, colorscale, leftName, rightName, detail.decimals]);

  const leftTraces = useMemo(() => buildTraces("left"), [buildTraces]);
  const rightTraces = useMemo(() => buildTraces("right"), [buildTraces]);

  const layout = useMemo(() => {
    const axisStyle = {
      backgroundcolor: "transparent",
      gridcolor: light ? "#d1d5db" : "#374151",
      showbackground: true,
      color: light ? "#4b5563" : "#9ca3af",
      tickmode: "array" as const,
    };
    return {
      paper_bgcolor: "transparent",
      plot_bgcolor: "transparent",
      scene: {
        xaxis: { ...axisStyle, title: axisLabels?.xUnit || axisLabels?.xLabel || "X", tickvals: prepared.ticks.xTickVals, ticktext: prepared.ticks.xTickText },
        yaxis: { ...axisStyle, title: axisLabels?.yUnit || axisLabels?.yLabel || "Y", tickvals: prepared.ticks.yTickVals, ticktext: prepared.ticks.yTickText },
        // Même plage Z des deux côtés : les hauteurs se comparent directement
        zaxis: { ...axisStyle, tickmode: "auto" as const, title: "", range: [prepared.zMin, prepared.zMax] },
        camera: defaultCamera,
        aspectmode: "manual",
        aspectratio: { x: 1, y: 1, z: 0.7 },
      },
      margin: { t: 10, r: 0, b: 0, l: 0 },
      autosize: true,
      // Garde le point de vue de l'utilisateur tant que la map ne change pas
      uirevision: mapAddress,
    };
  }, [light, axisLabels, prepared, defaultCamera, mapAddress]);

  // Applique la caméra de la vue manipulée à l'autre vue
  const syncCamera = useCallback((from: "left" | "right", event: any) => {
    if (syncingSide.current === from) return;
    const camera = event?.["scene.camera"];
    const to = from === "left" ? "right" : "left";
    const target = graphs.current[to];
    if (!camera || !target) return;
    const apply = (Plotly: NonNullable<typeof plotlyRef.current>) => {
      syncingSide.current = to;
      Promise.resolve(Plotly.relayout(target, { "scene.camera": camera }))
        .catch(() => {})
        .finally(() => { if (syncingSide.current === to) syncingSide.current = null; });
    };
    if (plotlyRef.current) apply(plotlyRef.current);
  }, []);

  // Plotly chargé dès l'ouverture de la vue : la synchro suit dès la
  // première image du glisser. Même build que react-plotly.js (module déjà
  // chargé, pas de second Plotly dans le bundle).
  useEffect(() => {
    let cancelled = false;
    import("plotly.js/dist/plotly").then((mod) => {
      if (!cancelled) plotlyRef.current = mod.default;
    });
    return () => { cancelled = true; };
  }, []);

  // Écoute de la caméra branchée directement sur le graphe, et rebranchée à
  // chaque (ré)initialisation : un Plotly.react peut purger les écouteurs du
  // graphe alors que react-plotly croit toujours les siens attachés
  // (onRelayout / onRelayouting ne se déclenchaient plus).
  const listeners = useRef<Partial<Record<"left" | "right", (e: unknown) => void>>>({});
  const wheelBound = useRef(new WeakSet<HTMLElement>());
  const bindGraph = useCallback((side: "left" | "right", gd: HTMLElement) => {
    graphs.current[side] = gd;
    let fn = listeners.current[side];
    if (!fn) {
      fn = (e: unknown) => syncCamera(side, e);
      listeners.current[side] = fn;
    }
    const g = gd as unknown as {
      on?: (evt: string, f: (e: unknown) => void) => void;
      removeListener?: (evt: string, f: (e: unknown) => void) => void;
    };
    for (const evt of ["plotly_relayouting", "plotly_relayout"]) {
      g.removeListener?.(evt, fn);
      g.on?.(evt, fn);
    }
    // Zoom molette : Plotly émet son relayout AVANT d'appliquer le zoom (la
    // caméra transmise est l'ancienne). On relit la caméra réelle de la
    // scène juste après.
    if (!wheelBound.current.has(gd)) {
      wheelBound.current.add(gd);
      gd.addEventListener("wheel", () => {
        window.setTimeout(() => {
          const scene = (gd as unknown as { _fullLayout?: { scene?: { _scene?: { getCamera?: () => Camera } } } })
            ._fullLayout?.scene?._scene;
          const camera = scene?.getCamera?.();
          if (camera) syncCamera(side, { "scene.camera": camera });
        }, 50);
      }, { passive: true });
    }
  }, [syncCamera]);

  // Identité stable : un nouvel objet à chaque rendu relancerait Plotly.react
  const plotConfig = useMemo(() => ({ displayModeBar: false, displaylogo: false }), []);
  const plotStyle = useMemo(() => ({ width: "100%", height: "100%" }), []);

  const panel = (side: "left" | "right") => {
    const name = side === "left" ? leftName : rightName;
    return (
      <div className="flex-1 min-w-0 flex flex-col" style={side === "left" ? { borderRight: `1px solid ${hairline}` } : undefined}>
        <div
          className="text-[10px] font-semibold text-center py-0.5 truncate"
          style={side === "left"
            ? { color: light ? "#991b1b" : "#fca5a5" }
            : { color: light ? "#166534" : "#86efac" }}
        >
          {name}
        </div>
        <div className="flex-1 relative min-h-0">
          <div className="absolute inset-0">
            {/* @ts-ignore */}
            <Plot
              key={`cmp3d-${side}-${mapAddress}`}
              data={side === "left" ? leftTraces : rightTraces}
              layout={layout}
              config={plotConfig}
              style={plotStyle}
              useResizeHandler={true}
              onInitialized={(_fig: unknown, gd: HTMLElement) => bindGraph(side, gd)}
              onUpdate={(_fig: unknown, gd: HTMLElement) => bindGraph(side, gd)}
            />
          </div>
        </div>
      </div>
    );
  };

  return (
    <div className="flex-1 flex min-h-0">
      {panel("left")}
      {panel("right")}
    </div>
  );
}
