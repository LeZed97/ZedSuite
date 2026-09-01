// PDF export of the virtual dyno sheet (A4 portrait, vector drawing),
// styled after the app's dark glassmorphism theme: dark page, glass cards,
// blue/indigo accent, red power / blue torque axes.
//
// The chart is drawn directly with jsPDF primitives — no DOM capture, so
// the output is crisp at any zoom. Saved through the native Tauri dialog
// (saveBytesToFile), browser downloads being inert in the webview.
//
// ⚠ Standard PDF fonts are WinAnsi/Latin-1 only: one character outside that
// range (≈, →, emojis…) silently corrupts the WHOLE string in jsPDF. Every
// string goes through sanitize(), and long values are ellipsized to their
// column width instead of overflowing the neighbours.

import { jsPDF } from "jspdf";
import { saveBytesToFile } from "./local/save-file";

export interface PdfCurve {
  label: string;
  color: string; // hex "#rrggbb"
  points: Array<{ rpm: number; hp: number; nm: number }>;
  peakHp: number;
  peakHpRpm: number;
  peakNm: number;
  peakNmRpm: number;
  boostMax: number | null; // mbar absolute
}

export interface PowerPdfData {
  title: string;
  clientLabel: string;
  clientName: string; // may be empty — the label is printed anyway
  infoPairs: Array<[string, string]>;
  curves: PdfCurve[];
  rpmLabel: string;
  hpLabel: string;
  nmLabel: string;
  peaksTitle: string;
  peaksHeader: [string, string, string, string];
  paramsLabel: string;
  paramsLine: string;
  disclaimer: string;
  footer: string;
  dateLabel: string;
  locale?: string; // "fr-FR" / "en-GB" — suit la langue de l'application
  theme?: PdfTheme; // choisi à l'export (défaut : sombre, style de l'app)
  companyName?: string; // entreprise de reprogrammation (réglages)
  tunerLabel: string; // « Tuner » / « Mappeur » — affiché même sans nom
}

const PAGE_W = 210;
const PAGE_H = 297;
const MARGIN = 15;
const CONTENT_W = PAGE_W - MARGIN * 2;

export type PdfTheme = "dark" | "light";

type Rgb = [number, number, number];
interface PdfPalette {
  BG: Rgb;
  PANEL: Rgb;
  PANEL_EDGE: Rgb;
  CHART_BG: Rgb;
  INK: Rgb;
  MUTED: Rgb;
  GRID: Rgb;
  TABLE_HEAD: Rgb;
  FOOT: Rgb;
}

// Sombre = thème de l'app ; clair = version imprimable sur fond blanc
const PALETTES: Record<PdfTheme, PdfPalette> = {
  dark: {
    BG: [11, 14, 20],
    PANEL: [22, 25, 34],
    PANEL_EDGE: [46, 51, 62],
    CHART_BG: [13, 16, 24],
    INK: [241, 245, 249],
    MUTED: [148, 163, 184],
    GRID: [40, 45, 56],
    TABLE_HEAD: [31, 37, 50],
    FOOT: [107, 118, 138],
  },
  light: {
    BG: [255, 255, 255],
    PANEL: [248, 250, 252],
    PANEL_EDGE: [226, 232, 240],
    CHART_BG: [253, 253, 255],
    INK: [15, 23, 42],
    MUTED: [100, 116, 139],
    GRID: [226, 232, 240],
    TABLE_HEAD: [241, 245, 249],
    FOOT: [130, 140, 155],
  },
};

// Remplacements typographiques → équivalents Latin-1 sûrs
const CHAR_MAP: Record<string, string> = {
  "≈": "~",
  "—": "-",
  "–": "-",
  "’": "'",
  "‘": "'",
  "“": '"',
  "”": '"',
  "…": "...",
  "→": "->",
  " ": " ",
};

function sanitize(s: string): string {
  let out = "";
  for (const ch of String(s)) {
    if (ch.charCodeAt(0) <= 0xff) out += ch;
    else out += CHAR_MAP[ch] ?? "";
  }
  return out;
}

function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

/** Round an axis maximum up to a "nice" bound (50/100 steps). */
function niceMax(v: number): number {
  if (v <= 0) return 100;
  const step = v > 400 ? 100 : 50;
  return Math.ceil((v * 1.08) / step) * step;
}


export async function exportPowerPdf(
  data: PowerPdfData,
  fileName: string
): Promise<boolean> {
  const doc = new jsPDF({ unit: "mm", format: "a4", compress: true });
  const { BG, PANEL, PANEL_EDGE, CHART_BG, INK, MUTED, GRID, TABLE_HEAD, FOOT } =
    PALETTES[data.theme ?? "dark"];

  // Texte assaini + tronqué à une largeur donnée (ellipse "...")
  const T = (
    s: string,
    x: number,
    y: number,
    opts?: { align?: "left" | "right" | "center"; maxW?: number }
  ) => {
    let txt = sanitize(s);
    if (opts?.maxW) {
      const full = txt;
      while (txt.length > 1 && doc.getTextWidth(txt) > opts.maxW) {
        txt = txt.slice(0, -1);
      }
      if (txt !== full) txt = txt.slice(0, -2) + "...";
    }
    doc.text(txt, x, y, opts?.align ? { align: opts.align } : undefined);
  };

  const panel = (x: number, y: number, w: number, h: number) => {
    doc.setFillColor(...PANEL);
    doc.setDrawColor(...PANEL_EDGE);
    doc.setLineWidth(0.25);
    doc.roundedRect(x, y, w, h, 2, 2, "FD");
  };

  // ── Fond de page uni (sombre ou clair selon le thème choisi) ──
  doc.setFillColor(...BG);
  doc.rect(0, 0, PAGE_W, PAGE_H, "F");

  let y = 16;

  // ── En-tête : logo « ZedSuite » comme dans l'éditeur (« Zed » en
  // dégradé red-600 → red-500 → orange-500, « Suite » en blanc/encre) —
  // le nom de l'entreprise s'affiche plus bas, sur la ligne client
  doc.setFont("helvetica", "bold");
  doc.setFontSize(21);
  const LOGO_STOPS: Array<[number, number, number]> = [
    [220, 38, 38], // red-600
    [239, 68, 68], // red-500
    [249, 115, 22], // orange-500
  ];
  let lx0 = MARGIN;
  "Zed".split("").forEach((ch, i) => {
    doc.setTextColor(...LOGO_STOPS[Math.min(i, LOGO_STOPS.length - 1)]);
    doc.text(ch, lx0, y + 1.5);
    lx0 += doc.getTextWidth(ch);
  });
  doc.setTextColor(...INK);
  doc.text("Suite", lx0, y + 1.5);

  doc.setFontSize(13.5);
  T(data.title, PAGE_W - MARGIN, y - 1.5, { align: "right" });
  doc.setFont("helvetica", "normal");
  doc.setFontSize(9);
  doc.setTextColor(...MUTED);
  T(`${data.dateLabel} : ${new Date().toLocaleDateString(data.locale)}`, PAGE_W - MARGIN, y + 3.5, { align: "right" });
  y += 8.5;
  // filet accent : même dégradé que le logo (red-600 → red-500 → orange-500)
  doc.setLineWidth(0.8);
  const RULE_SEGS = 48;
  for (let i = 0; i < RULE_SEGS; i++) {
    const t0 = i / RULE_SEGS;
    // interpolation sur les 3 arrêts du dégradé
    const pos = t0 * (LOGO_STOPS.length - 1);
    const k = Math.min(Math.floor(pos), LOGO_STOPS.length - 2);
    const f = pos - k;
    const c0 = LOGO_STOPS[k], c1 = LOGO_STOPS[k + 1];
    doc.setDrawColor(
      Math.round(c0[0] + (c1[0] - c0[0]) * f),
      Math.round(c0[1] + (c1[1] - c0[1]) * f),
      Math.round(c0[2] + (c1[2] - c0[2]) * f)
    );
    const xa = MARGIN + (CONTENT_W / RULE_SEGS) * i;
    doc.line(xa, y, xa + CONTENT_W / RULE_SEGS + 0.2, y);
  }
  y += 9;

  // ── Ligne tuner / client : le tuner à gauche (libellé toujours affiché,
  // comme le client), le client à droite ──
  doc.setFontSize(12);
  const company = (data.companyName || "").trim();
  doc.setFont("helvetica", "normal");
  doc.setTextColor(...MUTED);
  T(`${data.tunerLabel} :`, MARGIN, y);
  doc.setFont("helvetica", "bold");
  doc.setTextColor(...INK);
  T(company, MARGIN + doc.getTextWidth(`${data.tunerLabel} : `), y, {
    maxW: CONTENT_W * 0.45,
  });
  const clientName = data.clientName || "";
  doc.setFont("helvetica", "bold");
  const nameW = doc.getTextWidth(sanitize(clientName));
  doc.setFont("helvetica", "normal");
  const labelW2 = doc.getTextWidth(`${data.clientLabel} : `);
  const clientX = PAGE_W - MARGIN - (labelW2 + nameW);
  doc.setTextColor(...MUTED);
  T(`${data.clientLabel} :`, clientX, y);
  doc.setFont("helvetica", "bold");
  doc.setTextColor(...INK);
  T(clientName, clientX + labelW2, y, { maxW: CONTENT_W * 0.45 });
  y += 6.5;

  // ── Carte infos projet ──
  // Ligne 1 : le projet seul, pleine largeur (les noms sont longs).
  // Lignes suivantes : les paires restantes deux par deux, EN LIGNE
  // (ECU+Version, puis HW+SW, puis véhicule…), valeurs tronquées.
  const [firstPair, ...restPairs] = data.infoPairs;
  const infoRows = 1 + Math.ceil(restPairs.length / 2);
  const cardH = infoRows * 5.6 + 6;
  panel(MARGIN, y, CONTENT_W, cardH);
  doc.setFontSize(9);
  const colW = CONTENT_W / 2;
  const labelW = 24;
  const drawPair = (label: string, value: string, px: number, py: number, valueMaxW: number) => {
    doc.setFont("helvetica", "normal");
    doc.setTextColor(...MUTED);
    T(`${label} :`, px, py, { maxW: labelW - 2 });
    doc.setFont("helvetica", "bold");
    doc.setTextColor(...INK);
    T(String(value || "-"), px + labelW, py, { maxW: valueMaxW });
  };
  if (firstPair) {
    drawPair(firstPair[0], firstPair[1], MARGIN + 5, y + 7, CONTENT_W - labelW - 14);
  }
  restPairs.forEach(([label, value], i) => {
    const col = i % 2;
    const row = 1 + Math.floor(i / 2);
    drawPair(label, value, MARGIN + 5 + col * colW, y + 7 + row * 5.6, colW - labelW - 9);
  });
  y += cardH + 12;

  // ── Graphique ──
  const chartX = MARGIN + 9;
  const chartW = CONTENT_W - 18;
  const chartH = 92;
  const chartY = y;

  const allPts = data.curves.flatMap((c) => c.points);
  if (allPts.length > 0) {
    const rpmMin = Math.min(...allPts.map((p) => p.rpm));
    const rpmMax = Math.max(...allPts.map((p) => p.rpm));
    const hpMax = niceMax(Math.max(...allPts.map((p) => p.hp)));
    const nmMax = niceMax(Math.max(...allPts.map((p) => p.nm)));
    const sx = (rpm: number) => chartX + ((rpm - rpmMin) / (rpmMax - rpmMin)) * chartW;
    const syHp = (hp: number) => chartY + chartH - (hp / hpMax) * chartH;
    const syNm = (nm: number) => chartY + chartH - (nm / nmMax) * chartH;

    // panneau du graphique (style carte)
    panel(chartX - 9, chartY - 10, CONTENT_W, chartH + 27);
    doc.setFillColor(...CHART_BG);
    doc.rect(chartX, chartY, chartW, chartH, "F");

    // grille + graduations
    doc.setFontSize(7.5);
    doc.setFont("helvetica", "normal");
    const gridLines = 5;
    for (let i = 0; i <= gridLines; i++) {
      const gyy = chartY + (chartH / gridLines) * i;
      doc.setDrawColor(...GRID);
      doc.setLineWidth(0.15);
      doc.line(chartX, gyy, chartX + chartW, gyy);
      doc.setTextColor(...INK);
      T(String(Math.round(hpMax - (hpMax / gridLines) * i)), chartX - 1.5, gyy + 1, { align: "right" });
      T(String(Math.round(nmMax - (nmMax / gridLines) * i)), chartX + chartW + 1.5, gyy + 1);
    }
    const rpmStep = rpmMax - rpmMin > 3000 ? 1000 : 500;
    doc.setTextColor(...MUTED);
    for (let r = Math.ceil(rpmMin / rpmStep) * rpmStep; r <= rpmMax; r += rpmStep) {
      const gxx = sx(r);
      doc.setDrawColor(...GRID);
      doc.line(gxx, chartY, gxx, chartY + chartH);
      T(String(r), gxx, chartY + chartH + 4, { align: "center" });
    }
    T(data.rpmLabel, chartX + chartW / 2, chartY + chartH + 8.5, { align: "center" });
    // Unités centrées au-dessus de leur colonne de graduations (l'alignement
    // au bord était optiquement décalé par la chasse des lettres)
    doc.setTextColor(...INK);
    const hpColW = doc.getTextWidth(String(hpMax));
    const nmColW = doc.getTextWidth(String(nmMax));
    T(data.hpLabel, chartX - 1.5 - hpColW / 2, chartY - 4.8, { align: "center" });
    T(data.nmLabel, chartX + chartW + 1.5 + nmColW / 2, chartY - 4.8, { align: "center" });

    // courbes : puissance pleine, couple pointillé.
    // Superposition : quand plusieurs courbes passent au même endroit
    // (± 0,55 mm sur les deux extrémités d'un segment), chacune ne trace
    // qu'un segment sur k, en alternance le long du tracé — les deux
    // couleurs restent visibles en « rayures » nettes, sans halo.
    const nCurves = data.curves.length;
    const EPS_OVERLAP = 0.55;
    const hpY = data.curves.map((c) => c.points.map((p) => syHp(p.hp)));
    const nmY = data.curves.map((c) => c.points.map((p) => syNm(p.nm)));
    const rpmIndex = data.curves.map((c) => {
      const m = new Map<number, number>();
      c.points.forEach((p, i) => m.set(p.rpm, i));
      return m;
    });
    const strokeInterleaved = (ci: number, ys: number[][]) => {
      const pts = data.curves[ci].points;
      for (let i = 1; i < pts.length; i++) {
        const overlapping: number[] = [];
        for (let cj = 0; cj < nCurves; cj++) {
          const j0 = rpmIndex[cj].get(pts[i - 1].rpm);
          const j1 = rpmIndex[cj].get(pts[i].rpm);
          if (j0 === undefined || j1 === undefined) continue;
          if (
            Math.abs(ys[cj][j0] - ys[ci][i - 1]) < EPS_OVERLAP &&
            Math.abs(ys[cj][j1] - ys[ci][i]) < EPS_OVERLAP
          ) {
            overlapping.push(cj);
          }
        }
        const k = overlapping.length;
        // phase alignée sur la grille rpm pour que l'alternance des
        // courbes s'imbrique proprement
        const phase = Math.round(pts[i].rpm / 100) % Math.max(1, k);
        if (k > 1 && overlapping.indexOf(ci) !== phase) continue;
        doc.line(sx(pts[i - 1].rpm), ys[ci][i - 1], sx(pts[i].rpm), ys[ci][i]);
      }
    };
    data.curves.forEach((curve, ci) => {
      const [cr, cg, cb] = hexToRgb(curve.color);
      doc.setDrawColor(cr, cg, cb);
      doc.setLineWidth(1.0);
      doc.setLineDashPattern([], 0);
      strokeInterleaved(ci, hpY);
      doc.setLineWidth(0.55);
      doc.setLineDashPattern([1.6, 1.2], 0);
      strokeInterleaved(ci, nmY);
      doc.setLineDashPattern([], 0);
      const pk = curve.points.reduce((m, p) => (p.hp > m.hp ? p : m));
      doc.setFillColor(cr, cg, cb);
      doc.circle(sx(pk.rpm), syHp(pk.hp), 1.0, "F");
    });

    doc.setDrawColor(...PANEL_EDGE);
    doc.setLineWidth(0.3);
    doc.rect(chartX, chartY, chartW, chartH);
    y = chartY + chartH + 12;

    // légende : une entrée par courbe (à l'épaisseur de son tracé) +
    // rappel plein/pointillé
    doc.setFontSize(8);
    let lx = chartX;
    for (const curve of data.curves) {
      const [cr, cg, cb] = hexToRgb(curve.color);
      doc.setDrawColor(cr, cg, cb);
      doc.setLineWidth(1.0);
      doc.line(lx, y - 1.2, lx + 6, y - 1.2);
      doc.setTextColor(...INK);
      doc.setFont("helvetica", "bold");
      T(curve.label, lx + 7.5, y, { maxW: 42 });
      lx += 7.5 + Math.min(42, doc.getTextWidth(sanitize(curve.label))) + 9;
    }
    // Rappel des styles : vrai trait plein pour la puissance, vrai
    // pointillé pour le couple (plus de tirets typographiques)
    doc.setFont("helvetica", "normal");
    doc.setTextColor(...MUTED);
    doc.setDrawColor(...MUTED);
    const seg = 8, segGap = 2, groupSp = 6;
    const wHp = doc.getTextWidth(`${data.hpLabel} =`);
    const wNm = doc.getTextWidth(`${data.nmLabel} =`);
    let hx = chartX + chartW - (wHp + segGap + seg + groupSp + wNm + segGap + seg);
    T(`${data.hpLabel} =`, hx, y);
    hx += wHp + segGap;
    doc.setLineWidth(0.9);
    doc.setLineDashPattern([], 0);
    doc.line(hx, y - 1.2, hx + seg, y - 1.2);
    hx += seg + groupSp;
    T(`${data.nmLabel} =`, hx, y);
    hx += wNm + segGap;
    doc.setLineWidth(0.5);
    doc.setLineDashPattern([1.6, 1.2], 0);
    doc.line(hx, y - 1.2, hx + seg, y - 1.2);
    doc.setLineDashPattern([], 0);
    y += 10;
  }

  // ── Tableau des pics ──
  doc.setFont("helvetica", "bold");
  doc.setFontSize(11.5);
  doc.setTextColor(...INK);
  T(data.peaksTitle, MARGIN, y);
  y += 5.5;
  const colX = [MARGIN, MARGIN + 50, MARGIN + 98, MARGIN + 146];
  const rowH = 7;
  const tableH = rowH * (data.curves.length + 1);
  panel(MARGIN, y, CONTENT_W, tableH + 1);
  // en-tête
  doc.setFillColor(...TABLE_HEAD);
  doc.roundedRect(MARGIN, y, CONTENT_W, rowH, 2, 2, "F");
  doc.rect(MARGIN, y + rowH - 2, CONTENT_W, 2, "F");
  doc.setFontSize(8.5);
  doc.setTextColor(...MUTED);
  data.peaksHeader.forEach((h, i) => {
    T(h, colX[i] + 3, y + 4.8, { maxW: (colX[i + 1] ?? MARGIN + CONTENT_W) - colX[i] - 5 });
  });
  y += rowH;
  // lignes
  data.curves.forEach((curve) => {
    const [cr, cg, cb] = hexToRgb(curve.color);
    doc.setFillColor(cr, cg, cb);
    doc.circle(colX[0] + 4.5, y + 3.4, 1.2, "F");
    doc.setTextColor(...INK);
    doc.setFont("helvetica", "normal");
    T(curve.label, colX[0] + 8, y + 4.8, { maxW: colX[1] - colX[0] - 10 });
    doc.setFont("helvetica", "bold");
    T(`${Math.round(curve.peakHp)} ${data.hpLabel} @ ${curve.peakHpRpm} ${data.rpmLabel}`, colX[1] + 3, y + 4.8, {
      maxW: colX[2] - colX[1] - 5,
    });
    T(`${Math.round(curve.peakNm)} Nm @ ${curve.peakNmRpm} ${data.rpmLabel}`, colX[2] + 3, y + 4.8, {
      maxW: colX[3] - colX[2] - 5,
    });
    doc.setFont("helvetica", "normal");
    doc.setTextColor(...MUTED);
    T(
      curve.boostMax != null ? `${(curve.boostMax / 1000).toFixed(2)} bar` : "-",
      colX[3] + 3,
      y + 4.8,
      { maxW: MARGIN + CONTENT_W - colX[3] - 5 }
    );
    y += rowH;
  });
  y += 8;

  // ── Paramètres + avertissement ──
  doc.setFontSize(8.5);
  doc.setTextColor(...MUTED);
  doc.setFont("helvetica", "bold");
  T(`${data.paramsLabel} :`, MARGIN, y);
  doc.setFont("helvetica", "normal");
  T(data.paramsLine, MARGIN + doc.getTextWidth(`${data.paramsLabel} : `) + 2, y, {
    maxW: CONTENT_W - 40,
  });
  y += 6;
  doc.setTextColor(...FOOT);
  const disclaimer = doc.splitTextToSize(sanitize(data.disclaimer), CONTENT_W);
  doc.text(disclaimer, MARGIN, y);

  // ── Pied de page ──
  doc.setFontSize(7.5);
  doc.setTextColor(...FOOT);
  T(data.footer, PAGE_W / 2, 289, { align: "center" });

  const bytes = new Uint8Array(doc.output("arraybuffer"));
  return saveBytesToFile(bytes, fileName);
}
