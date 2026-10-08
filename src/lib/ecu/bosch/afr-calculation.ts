/**
 * Module AFR (Air-Fuel Ratio) & Lambda Calculation for Smoke Limiters
 * Bosch EDC15 (and extensible for future EDC16, EDC17, etc.)
 */

export type AfrMapType = 'maf' | 'map' | 'lambda';
export type AfrUnit = 'afr' | 'lambda';
export type AfrCellStatus = 'rich' | 'limit' | 'clean' | 'lean' | 'invalid';

export interface DetectedMapLike {
  name: string;
  address: number;
  size?: number;
  description?: string;
  category?: string;
  subcategory?: string;
  x_label?: string;
  y_label?: string;
  unit?: string;
  dimensions?: {
    TwoDimensional?: {
      rows: number;
      cols: number;
    };
    OneDimensional?: {
      length: number;
    };
  };
}

export interface AfrMapInfo {
  isSmokeMap: boolean;
  mapType: AfrMapType;
  /** Which axis represents Engine Speed (RPM) */
  rpmAxis: 'x' | 'y';
  /** Which axis represents Air Mass (mg/st) or Boost (mbar) */
  airAxis: 'x' | 'y';
  description?: string;
}

export interface AfrEnginePreset {
  id: string;
  label: string;
  cylinders: number;
  displacementCc: number;
}

export const AFR_ENGINE_PRESETS: AfrEnginePreset[] = [
  { id: '1.4tdi', label: '1.4 TDI — 3 cyl. (1422 cc)', cylinders: 3, displacementCc: 1422 },
  { id: '1.9tdi', label: '1.9 TDI — 4 cyl. (1896 cc)', cylinders: 4, displacementCc: 1896 },
  { id: '2.0tdi', label: '2.0 TDI — 4 cyl. (1968 cc)', cylinders: 4, displacementCc: 1968 },
  { id: '2.5tdi', label: '2.5 TDI V6 — 6 cyl. (2496 cc)', cylinders: 6, displacementCc: 2496 },
];

export const DEFAULT_DIESEL_STOICH = 14.5;
export const DEFAULT_SMOKE_LIMIT_AFR = 17.0;

export interface AfrCalculationOptions {
  unit?: AfrUnit;
  enginePresetId?: string;
  displacementCc?: number;
  cylinders?: number;
  manifoldTempK?: number;
  smokeLimitAfr?: number;
  stoichRatio?: number;
}

export interface AfrMatrixResult {
  /** Matrix of values according to selected unit (AFR or Lambda) */
  values: number[][];
  /** Raw AFR values matrix (always AFR ratio) */
  rawAfr: number[][];
  minAfr: number;
  maxAfr: number;
  minLambda: number;
  maxLambda: number;
  smokeRiskCount: number;
  unit: AfrUnit;
  mapType: AfrMapType;
}

export interface EcuAfrStrategy {
  id: string;
  name: string;
  supports(ecuType?: string): boolean;
  identifySmokeMap(map: DetectedMapLike, ecuType?: string): AfrMapInfo | null;
  calculate(
    map: DetectedMapLike,
    displayValues: number[][],
    displayXAxis: number[],
    displayYAxis: number[],
    options: AfrCalculationOptions,
    ecuType?: string
  ): AfrMatrixResult | null;
}

// ── Physical Constants & VE Curve ─────────────────────────────────────
const R_AIR = 287; // J/(kg·K)
const DEFAULT_T_MANIFOLD = 330; // K (after intercooler under full boost)

/**
 * 8-valve TDI (EDC15) Volumetric Efficiency curve vs RPM
 * Calibrated: Peak VE is 89% (around 2000-2500 rpm) and 79% at 4000 rpm.
 */
const EDC15_8V_VE_CURVE: Array<[number, number]> = [
  [1000, 0.78],
  [1500, 0.84],
  [2000, 0.89],
  [2500, 0.89],
  [3000, 0.86],
  [3500, 0.83],
  [4000, 0.79],
  [4500, 0.70],
  [5000, 0.58],
  [5500, 0.48],
];

export function getEdc15VolumetricEfficiency(rpm: number): number {
  if (rpm <= EDC15_8V_VE_CURVE[0][0]) return EDC15_8V_VE_CURVE[0][1];
  const last = EDC15_8V_VE_CURVE[EDC15_8V_VE_CURVE.length - 1];
  if (rpm >= last[0]) return last[1];

  for (let i = 0; i < EDC15_8V_VE_CURVE.length - 1; i++) {
    const [r0, ve0] = EDC15_8V_VE_CURVE[i];
    const [r1, ve1] = EDC15_8V_VE_CURVE[i + 1];
    if (rpm >= r0 && rpm <= r1) {
      const t = (rpm - r0) / (r1 - r0);
      return ve0 + t * (ve1 - ve0);
    }
  }
  return 0.85;
}

/**
 * Calculates estimated cylinder air mass in mg/stroke from manifold absolute pressure (mbar) and RPM.
 */
export function estimateAirMassFromMap(
  boostMbar: number,
  rpm: number,
  displacementCc: number = 1896,
  cylinders: number = 4,
  manifoldTempK: number = DEFAULT_T_MANIFOLD
): number {
  const pPa = Math.max(0, boostMbar) * 100; // mbar to Pa
  const rho = pPa / (R_AIR * manifoldTempK); // kg/m3
  const cylVolM3 = (displacementCc / cylinders) * 1e-6; // m3
  const ve = getEdc15VolumetricEfficiency(rpm);

  return cylVolM3 * rho * ve * 1e6; // mg/stroke
}

// ── EDC15 Strategy ───────────────────────────────────────────────────
export const edc15AfrStrategy: EcuAfrStrategy = {
  id: 'edc15',
  name: 'Bosch EDC15 (EDC15P / EDC15VM / EDC15C)',

  supports(ecuType?: string): boolean {
    if (!ecuType) return true; // Default fallback to EDC15
    const type = ecuType.toLowerCase();
    return type.includes('edc15') || type.includes('edc15p') || type.includes('edc15vm') || type.includes('edc15c');
  },

  identifySmokeMap(map: DetectedMapLike, ecuType?: string): AfrMapInfo | null {
    if (!this.supports(ecuType)) return null;

    const name = (map.name || '').toLowerCase();
    const category = (map.category || '').toLowerCase();
    const subcategory = (map.subcategory || '').toLowerCase();

    // Ignore single-value switches, air intake linearizations, EGR
    if (name.includes('switch') || name.includes('linearisation') || name.includes('air intake')) {
      return null;
    }

    const isSmokeName =
      name.includes('smoke') ||
      name.includes('fumée') ||
      name.includes('fumee') ||
      name.includes('iq by ma') ||
      name.includes('limitation de débit') ||
      name.includes('limitation de debit');

    const isSmokeCategory =
      category.includes('smoke') ||
      subcategory.includes('smoke') ||
      category.includes('fumée') ||
      category.includes('fumee');

    if (!isSmokeName && !isSmokeCategory) {
      return null;
    }

    // Determine whether MAP or MAF based on labels and name
    const xLabel = (map.x_label || '').toLowerCase();
    const yLabel = (map.y_label || '').toLowerCase();

    const isExplicitMap =
      name.includes('by map') ||
      name.includes('par map') ||
      xLabel.includes('mbar') ||
      yLabel.includes('mbar') ||
      xLabel.includes('pressure') ||
      yLabel.includes('pressure') ||
      xLabel.includes('boost') ||
      yLabel.includes('boost');

    // Identify which axis is RPM and which is Air/Boost
    let rpmAxis: 'x' | 'y' = 'y';
    let airAxis: 'x' | 'y' = 'x';

    if (xLabel.includes('rpm') || xLabel.includes('tr/min') || xLabel.includes('speed')) {
      rpmAxis = 'x';
      airAxis = 'y';
    } else if (yLabel.includes('rpm') || yLabel.includes('tr/min') || yLabel.includes('speed')) {
      rpmAxis = 'y';
      airAxis = 'x';
    } else {
      // Default standard layout for EDC15 smoke maps is X = Airflow / Boost, Y = RPM
      airAxis = 'x';
      rpmAxis = 'y';
    }

    return {
      isSmokeMap: true,
      mapType: isExplicitMap ? 'map' : 'maf',
      rpmAxis,
      airAxis,
      description: isExplicitMap ? 'Smoke limiter (IQ by MAP)' : 'Smoke limiter (by MAF air mass)',
    };
  },

  calculate(
    map: DetectedMapLike,
    displayValues: number[][],
    displayXAxis: number[],
    displayYAxis: number[],
    options: AfrCalculationOptions,
    ecuType?: string
  ): AfrMatrixResult | null {
    const info = this.identifySmokeMap(map, ecuType);
    if (!info) return null;

    if (!displayValues.length || !displayValues[0]?.length) return null;

    const rows = displayValues.length;
    const cols = displayValues[0].length;

    // Detect if axis numbers suggest MAP (mbar values > 1600)
    let mapType = info.mapType;
    const airValues = info.airAxis === 'x' ? displayXAxis : displayYAxis;
    if (airValues.length > 0 && Math.max(...airValues) > 1600) {
      mapType = 'map';
    }

    // Engine displacement & cylinders selection
    let displacementCc = options.displacementCc ?? 1896;
    let cylinders = options.cylinders ?? 4;
    if (options.enginePresetId) {
      const preset = AFR_ENGINE_PRESETS.find((p) => p.id === options.enginePresetId);
      if (preset) {
        displacementCc = preset.displacementCc;
        cylinders = preset.cylinders;
      }
    }

    const manifoldTempK = options.manifoldTempK ?? DEFAULT_T_MANIFOLD;
    const stoich = options.stoichRatio ?? DEFAULT_DIESEL_STOICH;
    const smokeLimitAfr = options.smokeLimitAfr ?? DEFAULT_SMOKE_LIMIT_AFR;
    const unit = options.unit ?? 'afr';

    const values: number[][] = [];
    const rawAfr: number[][] = [];
    let minAfr = Infinity;
    let maxAfr = -Infinity;
    let smokeRiskCount = 0;

    for (let r = 0; r < rows; r++) {
      const rowVal: number[] = [];
      const rowRaw: number[] = [];
      for (let c = 0; c < cols; c++) {
        const iq = displayValues[r][c];

        // Fuel cut / zero fuel cell
        if (iq <= 0.5 || isNaN(iq)) {
          rowVal.push(NaN);
          rowRaw.push(NaN);
          continue;
        }

        const airVal = info.airAxis === 'x' ? displayXAxis[c] : displayYAxis[r];
        const rpmVal = info.rpmAxis === 'x' ? displayXAxis[c] : displayYAxis[r];

        let airMg = airVal;
        if (mapType === 'map') {
          airMg = estimateAirMassFromMap(airVal, rpmVal, displacementCc, cylinders, manifoldTempK);
        }

        if (airMg <= 0 || isNaN(airMg)) {
          rowVal.push(NaN);
          rowRaw.push(NaN);
          continue;
        }

        const afr = airMg / iq;
        const lambda = afr / stoich;

        rowRaw.push(afr);
        rowVal.push(unit === 'lambda' ? lambda : afr);

        if (afr < minAfr) minAfr = afr;
        if (afr > maxAfr) maxAfr = afr;
        if (afr < smokeLimitAfr) smokeRiskCount++;
      }
      values.push(rowVal);
      rawAfr.push(rowRaw);
    }

    const resolvedMinAfr = Number.isFinite(minAfr) ? minAfr : 0;
    const resolvedMaxAfr = Number.isFinite(maxAfr) ? maxAfr : 0;

    return {
      values,
      rawAfr,
      minAfr: resolvedMinAfr,
      maxAfr: resolvedMaxAfr,
      minLambda: resolvedMinAfr / stoich,
      maxLambda: resolvedMaxAfr / stoich,
      smokeRiskCount,
      unit,
      mapType,
    };
  },
};

// ── Pluggable Registry ────────────────────────────────────────────────
const STRATEGIES: EcuAfrStrategy[] = [edc15AfrStrategy];

/**
 * Register a new ECU strategy (e.g. for EDC16, EDC17, Siemens, etc.)
 */
export function registerAfrStrategy(strategy: EcuAfrStrategy): void {
  if (!STRATEGIES.some((s) => s.id === strategy.id)) {
    STRATEGIES.push(strategy);
  }
}

export function getAfrStrategyForEcu(ecuType?: string): EcuAfrStrategy | null {
  for (const strategy of STRATEGIES) {
    if (strategy.supports(ecuType)) {
      return strategy;
    }
  }
  return null;
}

export function identifySmokeMap(map: DetectedMapLike, ecuType?: string): AfrMapInfo | null {
  const strategy = getAfrStrategyForEcu(ecuType);
  if (!strategy) return null;
  return strategy.identifySmokeMap(map, ecuType);
}

export function computeAfrMatrix(
  map: DetectedMapLike,
  displayValues: number[][],
  displayXAxis: number[],
  displayYAxis: number[],
  options: AfrCalculationOptions = {},
  ecuType?: string
): AfrMatrixResult | null {
  const strategy = getAfrStrategyForEcu(ecuType);
  if (!strategy) return null;
  return strategy.calculate(map, displayValues, displayXAxis, displayYAxis, options, ecuType);
}

// ── Color Grading & Formatting ───────────────────────────────────────

/**
 * Returns color representation for AFR / Lambda values with configurable smoke thresholds.
 */
export function getAfrCellColor(
  value: number,
  unit: AfrUnit = 'afr',
  smokeLimitAfr: number = DEFAULT_SMOKE_LIMIT_AFR,
  theme: 'default' | 'light' | 'oled' = 'default'
): string {
  if (isNaN(value) || !Number.isFinite(value) || value <= 0) {
    return 'transparent';
  }

  // Convert to equivalent AFR if value is Lambda
  const afr = unit === 'lambda' ? value * DEFAULT_DIESEL_STOICH : value;
  const isLight = theme === 'light';

  // Deep Rich / Unburnt Smoke (< 14.5)
  if (afr < 14.5) {
    return isLight ? 'rgba(239, 68, 68, 0.45)' : 'rgba(220, 38, 38, 0.45)';
  }

  // Rich / Visible Smoke Zone (14.5 .. smokeLimitAfr - 1.0)
  if (afr < smokeLimitAfr - 1.0) {
    return isLight ? 'rgba(249, 115, 22, 0.40)' : 'rgba(234, 88, 12, 0.40)';
  }

  // Limit / Performance Zone (around smoke limit)
  if (afr < smokeLimitAfr + 0.5) {
    return isLight ? 'rgba(234, 179, 8, 0.35)' : 'rgba(202, 138, 4, 0.35)';
  }

  // Clean / Optimal Burn Zone (smokeLimit + 0.5 .. 22.0)
  if (afr < 22.0) {
    return isLight ? 'rgba(34, 197, 94, 0.35)' : 'rgba(22, 163, 74, 0.35)';
  }

  // Very Lean / Cruising / Off-Boost (> 22.0)
  return isLight ? 'rgba(14, 165, 233, 0.30)' : 'rgba(2, 132, 199, 0.30)';
}

/**
 * Format AFR or Lambda value for table cell display.
 */
export function formatAfrValue(value: number, unit: AfrUnit = 'afr'): string {
  if (isNaN(value) || !Number.isFinite(value) || value <= 0) {
    return '—';
  }
  return unit === 'lambda' ? value.toFixed(2) : value.toFixed(1);
}
