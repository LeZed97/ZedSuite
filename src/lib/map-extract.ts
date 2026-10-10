/**
 * Lecture d'une map telle que l'éditeur l'AFFICHE : valeurs (facteur/offset
 * appliqués), libellés d'axes, orientation (transpositions, inversions de
 * lignes/colonnes).
 *
 * Extrait du MapViewer pour être partagé avec la comparaison de maps
 * (compare-modal) : les deux vues DOIVENT montrer la même grille, sinon une
 * différence pointée dans la comparaison ne se retrouve pas dans l'éditeur.
 * Fonction pure — le cache reste côté MapViewer.
 */
import { isBigEndianEcu, hasUnsignedAxes } from "@/lib/ecu-endianness";
import { resolveMapCellLayout, resolveAxisSources, shouldSwapAxes, type MapAxisSourceInput } from "@/lib/map-cell-layout";

export interface ExtractMapInput extends MapAxisSourceInput {
  name: string;
  address: number;
  size: number;
  description?: string;
  correction_factor?: number;
  offset?: number;
  y_axis_inverted?: boolean;
  is_little_endian?: boolean;
  data_type?: string;
  x_axis_values?: number[] | null;
  y_axis_values?: number[] | null;
}

interface AxisDisplaySettings {
  factor?: number;
  offset?: number;
  divisor?: number;
  precision?: number;
  /** Axe affiché en miroir (fenêtre Propriétés) */
  mirror?: boolean;
}

export interface ExtractMapDisplaySettings {
  xAxis?: AxisDisplaySettings;
  yAxis?: AxisDisplaySettings;
  map?: AxisDisplaySettings & {
    /** Bouton d'inversion du header : affichage transposé */
    invertDisplay?: boolean;
  };
}

export interface ExtractedMapGrid {
  mapValues: number[][];
  xAxisLabels: string[];
  yAxisLabels: string[];
  axesSwapped: boolean;
  rowsReversed: boolean;
  colsReversed: boolean;
  xAxisIsIndex: boolean;
  yAxisIsIndex: boolean;
  apiRows: number;
  apiCols: number;
}

export function extractMapGrid(
  mapData: ExtractMapInput,
  fileData: number[],
  ecuType: string | undefined,
  displaySettings?: ExtractMapDisplaySettings,
): ExtractedMapGrid {
  const mapNameLower = mapData.external_source ? "" : (mapData.name || "").toLowerCase();
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
  // Dimensions d'affichage et index fichier de chaque cellule : règles
  // partagées avec l'éditeur (écriture des modifications) dans
  // lib/map-cell-layout — les deux DOIVENT lire/écrire la même cellule.
  const cellLayout = resolveMapCellLayout(mapData);
  const egrDimsSwapped = isEgrMap && apiRows < apiCols;
  const rows = cellLayout.rows;  // Display rows (vertical axis)
  const cols = cellLayout.cols;  // Display cols (horizontal axis)


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
  const isStartIQ = mapNameLower.includes("start iq");
  
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

  // Adresse, facteur et offset des axes AFFICHÉS : règle partagée avec
  // l'éditeur (écriture des libellés édités) dans lib/map-cell-layout — les
  // deux DOIVENT lire/écrire le même axe au même facteur. Sur les maps dont
  // la vue transpose (durations 01-05, Drivers wish MJD6, torque limiter,
  // N75 13x16), l'axe du haut vient de l'adresse Y du détecteur.
  const axisSources = resolveAxisSources(mapData);
  let xAxisAddr = axisSources.x.address;
  let yAxisAddr = axisSources.y.address;

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
  
  // Corrections d'affichage (même source que les adresses, voir plus haut) ;
  // les réglages de la fenêtre Propriétés s'appliquent ensuite.
  let xAxisCorrection = axisSources.x.correction;
  let xAxisOffset = axisSources.x.offset;
  let yAxisCorrection = axisSources.y.correction;
  let yAxisOffset = axisSources.y.offset;

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
  // Axe écrit en clair dans le fichier de définitions importé (balises
  // LABEL d'un XDF) : il n'a pas d'adresse dans le binaire, ses points
  // sont dans la définition. Sans lui, l'axe serait numéroté 1..N.
  const fixedAxisLabels = (values: number[] | null | undefined, count: number): string[] | null => {
    if (!Array.isArray(values) || values.length !== count || count === 0) return null;
    const step = values.length > 1 ? Math.abs(values[1] - values[0]) : Math.abs(values[0]);
    const decimals = step === 0 ? 0 : step < 0.1 ? 4 : step < 1 ? 2 : 0;
    return values.map((v) => v.toFixed(decimals));
  };
  let xAxisIsIndex = false;
  let yAxisIsIndex = false;
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
      // MAP linearisation EDC15 : EDCSuite présente ses deux points à
      // l'envers (axe « backwards » : 913 puis 44 sur un capteur 4 bars),
      // on garde le même ordre pour que les écrans se comparent.
      const xDescendingLikeEdcsuite = mapNameLower === 'map linearisation' && !mapData.external_source && !isBigEndianEcu(ecuType);
      if (xDescendingLikeEdcsuite ? first < last : first > last) {
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
    const fixedX = fixedAxisLabels(mapData.x_axis_values, cols);
    if (fixedX) {
      xLabels.push(...fixedX);
    } else {
      // Sans axe dans le fichier (sélecteurs, courbes 1×N…) : simple index
      // 1..N — l'ancien « 0, 5, 10… » ressemblait à de vraies valeurs et
      // rendait les sélecteurs SOI illisibles.
      for (let i = 0; i < cols; i++) {
        xLabels.push(String(i + 1));
      }
      xAxisIsIndex = true;
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
    const isTorqueLimiter = mapNameLower.includes("torque limiter");
    const isIQByMap = mapNameLower.includes("iq by map");
    const isIQByMAF = mapNameLower.includes("iq by maf");
    const isStartIQ = mapNameLower.includes("start iq");
    
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
    const fixedY = fixedAxisLabels(mapData.y_axis_values, rows);
    if (fixedY) {
      yLabels.push(...fixedY);
    } else {
      // Sans axe dans le fichier : simple index 1..N (voir l'axe X)
      for (let i = 0; i < rows; i++) {
        yLabels.push(String(i + 1));
      }
      yAxisIsIndex = true;
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
  if (process.env.NODE_ENV !== 'production' && cellLayout.axesSwapped !== needsAxisSwap) {
    console.warn('[MapViewer] cell layout swap mismatch for', mapData.name, cellLayout.axesSwapped, needsAxisSwap);
  }
  for (let row = 0; row < rows; row++) {
    const rowValues: number[] = [];
    for (let col = 0; col < cols; col++) {
      // Offset fichier de la cellule (colonne-major pour le torque limiter et
      // les IQ by MAF/MAP transposés, transposition standard sinon)
      const offset = startAddress + cellLayout.cellIndex(row, col) * cellBytes;
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
          const isSOISelector = mapNameLower.includes('soi selector');
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
    const isTorqueLimiter = mapNameLower.includes("torque limiter");
    
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
      const isStartIQ = mapNameLower.includes("start iq");
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
    const isIQByMap = mapNameLower.includes("iq by map");
    const isIQByMAF = mapNameLower.includes("iq by maf");
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
    const isStartIQ = mapNameLower.includes("start iq");
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

  // NOTE : l'ancien « garde-fou » des maps « Duration NN » (EDC16) comparait
  // le libellé du haut à tempYLabels[0]… alors que tempYLabels venait d'être
  // retourné EN PLACE par yLabels.push(...tempYLabels.reverse()). Il
  // concluait toujours à un désalignement et re-retournait les valeurs :
  // toutes les durées EDC16 à axe croissant s'affichaient en miroir (coin
  // rouge en bas à droite). Les blocs réellement stockés à l'envers portent
  // maintenant rows_reversed, lu par cellLayout — plus rien à corriger ici.

  // Net row reversal: each reverse() inverts the order, so an odd count means
  // display row 0 corresponds to file row N-1.
  const rowsReversed = (rowsReversedCount % 2) === 1;
  const colsReversed = xLabelsWereReversed;
  return {
    mapValues: values,
    xAxisLabels: xLabels,
    yAxisLabels: yLabels,
    axesSwapped: needsAxisSwap,
    rowsReversed,
    colsReversed,
    xAxisIsIndex,
    yAxisIsIndex,
    apiRows,
    apiCols,
  };
}

/**
 * Décimales d'affichage des cellules : précision de la fenêtre Propriétés si
 * l'utilisateur l'a fixée, sinon autant de décimales que le pas de la map en
 * demande (WinOLS/EDCSuite), plafonné à 3. La pompe N146 en volts
 * (0.001221/bit) s'affiche 1.455 et non 1.4 ; un pas ≥ 0.01 garde
 * l'affichage habituel à 1 décimale.
 */
export function getMapCellDecimals(correctionFactor: number | undefined, precisionOverride?: number): number {
  if (typeof precisionOverride === 'number' && isFinite(precisionOverride) && precisionOverride >= 0) {
    return Math.min(6, Math.trunc(precisionOverride));
  }
  const cellFactorAbs = Math.abs(correctionFactor ?? 1);
  return cellFactorAbs > 0 && cellFactorAbs < 0.01
    ? Math.min(3, Math.ceil(-Math.log10(cellFactorAbs)))
    : 1;
}

export interface DisplayOrientationSettings {
  xMirror?: boolean;
  yMirror?: boolean;
  /** Bouton d'inversion du header : transposition de l'affichage */
  invert?: boolean;
}

/** Passage d'une grille lue (mapValues) à la grille AFFICHÉE */
export interface DisplayOrientation {
  /** Lignes d'affichage inversées par rapport à mapValues (après transposition) */
  rowsFlipped: boolean;
  colsFlipped: boolean;
  /** display(row, col) lit mapValues(col, row) */
  transposed: boolean;
}

export interface OrientedMapGrid extends DisplayOrientation {
  values: number[][];
  xLabels: string[];
  yLabels: string[];
  xIsIndex: boolean;
  yIsIndex: boolean;
}

const transposeMatrix = (matrix: number[][]): number[][] => {
  if (!matrix.length) return [];
  return matrix[0].map((_, colIndex) => matrix.map((row) => row[colIndex]));
};

const ensureXAsc = (labels: string[], values: number[][]) => {
  if (labels.length > 1) {
    const first = parseFloat(labels[0]);
    const last = parseFloat(labels[labels.length - 1]);
    if (!Number.isNaN(first) && !Number.isNaN(last) && first > last) {
      return { labels: [...labels].reverse(), values: values.map((row) => [...row].reverse()), flipped: true };
    }
  }
  return { labels, values, flipped: false };
};

const ensureYDesc = (labels: string[], values: number[][]) => {
  if (labels.length > 1) {
    const first = parseFloat(labels[0]);
    const last = parseFloat(labels[labels.length - 1]);
    if (!Number.isNaN(first) && !Number.isNaN(last) && first < last) {
      return { labels: [...labels].reverse(), values: [...values].reverse(), flipped: true };
    }
  }
  return { labels, values, flipped: false };
};

/**
 * Orientation d'AFFICHAGE d'une map lue par extractMapGrid : ordre des axes
 * (X croissant de gauche à droite, Y décroissant du haut vers le bas, sauf
 * sélecteur / durations d'injection), miroirs X/Y de la fenêtre Propriétés et
 * transposition du bouton d'inversion. Extrait du MapViewer, partagé avec la
 * comparaison de maps : une cellule pointée dans la comparaison doit être à
 * la même ligne / colonne que dans la fenêtre de la map.
 */
export function orientMapForDisplay(
  map: { name?: string; external_source?: string | null },
  mapValues: number[][],
  xAxisLabels: string[],
  yAxisLabels: string[],
  axisIsIndex: { x: boolean; y: boolean },
  settings: DisplayOrientationSettings = {},
  ecuType?: string,
): OrientedMapGrid {
  // Maps importées (OLS/XDF/JSON) : aucune règle d'affichage par le nom
  const mapNameLowerDisplay = map.external_source ? "" : (map.name || "").toLowerCase();
  const isSelectorInjector = mapNameLowerDisplay.includes("selector for injector duration");
  // Treat EDC16U34's "Duration NN" names as the same family as "Injector Duration NN"
  // when applying display-layer ordering decisions.
  const isInjectorDurationDisplay =
    mapNameLowerDisplay.includes("injector duration") || /^duration \d+$/.test(mapNameLowerDisplay);

  let displayMapValues = mapValues;
  let displayXAxisLabels = xAxisLabels;
  let displayYAxisLabels = yAxisLabels;
  // Track whether the display→mapValues mapping needs row/col mirroring.
  let displayRowsFlippedCount = 0;
  let displayColsFlippedCount = 0;

  // MAP linearisation EDC15 : ses deux points restent dans l'ordre d'EDCSuite
  // (décroissant, 913 puis 44 sur un capteur 4 bars), ici et dans
  // l'ordonnancement final plus bas. Les EDC16 gardent l'axe croissant.
  const keepXDescending = mapNameLowerDisplay === 'map linearisation' && !isBigEndianEcu(ecuType);
  if (!keepXDescending) {
    const xAdjusted = ensureXAsc(displayXAxisLabels, displayMapValues);
    displayXAxisLabels = xAdjusted.labels;
    displayMapValues = xAdjusted.values;
    if (xAdjusted.flipped) displayColsFlippedCount++;
  }

  // Y ordering:
  // - selector ascending
  // - others (injector durations, EGR, everything else): descending
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
  } else {
    const yAdjusted = ensureYDesc(displayYAxisLabels, displayMapValues);
    displayYAxisLabels = yAdjusted.labels;
    displayMapValues = yAdjusted.values;
    if (yAdjusted.flipped) displayRowsFlippedCount++;
  }

  // Appliquer les settings de miroir depuis displaySettings (prioritaire sur la logique par défaut)
  if (settings.xMirror) {
    displayXAxisLabels = [...displayXAxisLabels].reverse();
    displayMapValues = displayMapValues.map((row) => [...row].reverse());
    displayColsFlippedCount++;
  }
  if (settings.yMirror) {
    displayYAxisLabels = [...displayYAxisLabels].reverse();
    displayMapValues = [...displayMapValues].reverse();
    displayRowsFlippedCount++;
  }

  // Flip state (rows/cols mirrored vs mapValues) figé AVANT la transposition
  const preRowsFlipped = (displayRowsFlippedCount % 2) === 1;
  const preColsFlipped = (displayColsFlippedCount % 2) === 1;

  // Bouton d'inversion du header : on transpose le RÉSULTAT final (comme le
  // mirror), sans toucher la lecture. display[i][j] = pre[j][i].
  const displayTransposed = settings.invert === true;
  if (displayTransposed && displayMapValues.length > 0) {
    displayMapValues = transposeMatrix(displayMapValues);
    const tmp = displayXAxisLabels;
    displayXAxisLabels = displayYAxisLabels;
    displayYAxisLabels = tmp;
  }
  // Axe « index » (aucune valeur dans le fichier) : en-tête affiché « . »
  const xIsIndex = displayTransposed ? axisIsIndex.y : axisIsIndex.x;
  const yIsIndex = displayTransposed ? axisIsIndex.x : axisIsIndex.y;

  let netRowsFlipped = displayTransposed ? preColsFlipped : preRowsFlipped;
  let netColsFlipped = displayTransposed ? preRowsFlipped : preColsFlipped;

  // Ordonnancement FINAL (X croissant, Y décroissant), sauf familles à Y
  // ascendant et axes que l'utilisateur a mis en miroir (son choix prime).
  if (!settings.xMirror && !settings.yMirror) {
    if (!keepXDescending) {
      const xFinal = ensureXAsc(displayXAxisLabels, displayMapValues);
      displayXAxisLabels = xFinal.labels;
      displayMapValues = xFinal.values;
      if (xFinal.flipped) netColsFlipped = !netColsFlipped;
    }

    const keepYAscending = isSelectorInjector || isInjectorDurationDisplay;
    if (!keepYAscending) {
      const yFinal = ensureYDesc(displayYAxisLabels, displayMapValues);
      displayYAxisLabels = yFinal.labels;
      displayMapValues = yFinal.values;
      if (yFinal.flipped) netRowsFlipped = !netRowsFlipped;
    }
  }

  return {
    values: displayMapValues,
    xLabels: displayXAxisLabels,
    yLabels: displayYAxisLabels,
    xIsIndex,
    yIsIndex,
    rowsFlipped: netRowsFlipped,
    colsFlipped: netColsFlipped,
    transposed: displayTransposed,
  };
}

/**
 * Applique une orientation déjà calculée à une autre grille de même forme
 * (l'autre version d'une map comparée) : les deux côtés suivent exactement la
 * même disposition, même si leurs valeurs feraient choisir autrement.
 */
export function applyDisplayOrientation(
  mapValues: number[][],
  xAxisLabels: string[],
  yAxisLabels: string[],
  o: DisplayOrientation,
): { values: number[][]; xLabels: string[]; yLabels: string[] } {
  const mapRows = mapValues.length;
  const mapCols = mapValues[0]?.length ?? 0;
  const rows = o.transposed ? mapCols : mapRows;
  const cols = o.transposed ? mapRows : mapCols;
  const values: number[][] = [];
  for (let r = 0; r < rows; r++) {
    const rr = o.rowsFlipped ? rows - 1 - r : r;
    const row: number[] = [];
    for (let c = 0; c < cols; c++) {
      const cc = o.colsFlipped ? cols - 1 - c : c;
      row.push((o.transposed ? mapValues[cc]?.[rr] : mapValues[rr]?.[cc]) ?? 0);
    }
    values.push(row);
  }
  // Axe du haut : colonnes de mapValues (X), ou ses lignes (Y) si transposé
  const topSource = o.transposed ? yAxisLabels : xAxisLabels;
  const leftSource = o.transposed ? xAxisLabels : yAxisLabels;
  const xLabels = Array.from({ length: cols }, (_, c) => topSource[o.colsFlipped ? cols - 1 - c : c] ?? "");
  const yLabels = Array.from({ length: rows }, (_, r) => leftSource[o.rowsFlipped ? rows - 1 - r : r] ?? "");
  return { values, xLabels, yLabels };
}
