/**
 * Module Solutions — corrections applicables au binaire.
 *
 * Seule solution disponible : l'activation du Launch Control sur EDC15P et
 * EDC15VM. La cartographie 25×14 existe déjà dans le fichier d'origine mais
 * son axe de vitesse véhicule est neutralisé ; la solution le réécrit, ce qui
 * rend la carte utilisable (le détecteur Rust ne l'expose que dans ce cas).
 *
 * Le repérage suit exactement la version web : recherche d'une signature avec
 * jokers, puis écriture des paliers 0, 20, 40 … 260 km/h.
 */

export interface BinaryPatch {
  address: number; // Adresse absolue dans le fichier
  data: number[]; // Octets à écrire
  description?: string;
  /** Carte rendue disponible par le patch, pour l'ajouter à la liste. */
  createsMap?: {
    name: string;
    address: number;
    size: number;
    rows: number;
    cols: number;
    category?: string;
    subcategory?: string;
    correction_factor?: number;
    x_axis_correction?: number;
    y_axis_correction?: number;
    x_axis_address?: number;
    y_axis_address?: number;
    x_label?: string;
    y_label?: string;
    unit?: string;
    description?: string;
    y_axis_inverted?: boolean;
  };
}

export interface SolutionImplementation {
  id: string;
  name: string;
  description: string;
  applyBinaryPatches: (fileData: Uint8Array) => BinaryPatch[];
}

export interface Solution {
  id: string;
  name: string;
  description: string;
  category: string;
  icon: string;
  credits: number;
}

export interface SolutionCategory {
  id: string;
  name: string;
  description: string;
}

export interface ECUSolutionsConfig {
  ecuType: string;
  manufacturer: string;
  solutions: Solution[];
}

/**
 * Recherche une séquence d'octets avec masque (1 = doit correspondre,
 * 0 = joker). Renvoie l'offset trouvé, ou -1.
 */
export function findSequence(
  fileData: Uint8Array,
  offset: number,
  sequence: number[],
  mask: number[]
): number {
  let i = 0;
  let position = offset;

  while (position < fileData.length) {
    const data = fileData[position++];

    if (data === sequence[i] || mask[i] === 0) {
      i++;
    } else {
      position -= i;
      i = 0;
    }

    if (i === sequence.length) {
      return position - sequence.length;
    }
  }

  return -1;
}

/**
 * Octets de l'axe Y du Launch Control : en-tête (14 valeurs) puis les paliers
 * de vitesse véhicule en 16 bits petit-boutiste.
 *
 * Les paliers écrits sont les valeurs BRUTES 0, 20 … 260 ; l'axe se lit avec
 * le facteur 0,15625 km/h par bit, soit 0 à 40,6 km/h à l'écran — la plage
 * d'un launch control, et la même que celle d'EDCSuite sur ces fichiers
 * (40,6 km/h et 5361 tr/min en bout d'axes, vérifié le 09/09/2026). Ne pas
 * « corriger » ces paliers en les multipliant par 6,4 : la voiture réagit
 * comme prévu avec ceux-ci.
 */
export function generateLaunchControlYAxisBytes(): number[] {
  // Compteur d'axe en 16 bits petit-boutiste, comme le reste du fichier
  // EDC15 (l'axe neutralisé d'origine commence par 02 00 = 2 valeurs) :
  // 14 valeurs = 0E 00. Écrit 00 0E, le compteur valait 3584 et le launch
  // control restait inactif sur la voiture (issue #3, 038906019HJ).
  const bytes: number[] = [0x0e, 0x00];
  const values = [0, 20, 40, 60, 80, 100, 120, 140, 160, 180, 200, 220, 240, 260];

  for (const value of values) {
    bytes.push(value & 0xff);
    bytes.push((value >> 8) & 0xff);
  }

  return bytes;
}

/**
 * L'axe de vitesse du Launch Control est-il réellement écrit à cette adresse ?
 *
 * Même contrôle que le détecteur Rust (`launch_control.rs`) : l'en-tête de
 * 14 valeurs, dans l'un des deux encodages rencontrés, puis les 14 paliers
 * exacts. Sert à ne pas afficher une carte de launch control héritée d'une
 * liste enregistrée alors que les octets, eux, sont revenus à l'origine :
 * ses axes seraient lus dans le descripteur neutralisé et afficheraient
 * n'importe quoi (issue #23).
 */
export function isLaunchControlActive(
  data: Uint8Array | number[],
  yAxisAddress: number,
): boolean {
  const at = (i: number): number | undefined => (data as any)[i];
  const header = yAxisAddress - 2;
  if (header < 0 || yAxisAddress + 28 > (data as any).length) return false;
  const h0 = at(header);
  const h1 = at(header + 1);
  const headerOk = (h0 === 0x00 && h1 === 0x0e) || (h0 === 0x0e && h1 === 0x00);
  if (!headerOk) return false;
  const steps = [0, 20, 40, 60, 80, 100, 120, 140, 160, 180, 200, 220, 240, 260];
  return steps.every((expected, i) => {
    const lo = at(yAxisAddress + 2 * i);
    const hi = at(yAxisAddress + 2 * i + 1);
    return lo !== undefined && hi !== undefined && (lo | (hi << 8)) === expected;
  });
}

// Structure de la zone Launch Control, relative à la signature
const Y_AXIS_SIZE = 2 + 14 * 2; // 30 octets
const X_AXIS_SIZE = 2 + 25 * 2; // 52 octets
const MAP_OFFSET = 2 + Y_AXIS_SIZE + X_AXIS_SIZE + 2; // 86 octets

/** Signature de la zone Launch Control (les deux premiers octets sont des jokers). */
const LC_SEQUENCE = [
  0xff, 0xff, 0x02, 0x00, 0x80, 0x00, 0x00, 0x0a,
  0xff, 0xff, 0x02, 0x00, 0x00, 0x00, 0x70, 0x17,
];
const LC_MASK = [0, 0, 1, 1, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1];

export const launchControl: SolutionImplementation = {
  id: 'launch_control',
  name: 'Launch Control',
  description:
    "Active le Launch Control en écrivant l'axe de vitesse véhicule (km/h) de la cartographie",

  applyBinaryPatches: (fileData: Uint8Array): BinaryPatch[] => {
    const patches: BinaryPatch[] = [];
    let offset = 0;

    while (offset < fileData.length) {
      const signatureAddress = findSequence(fileData, offset, LC_SEQUENCE, LC_MASK);
      if (signatureAddress === -1) break;

      const mapDataAddress = signatureAddress + MAP_OFFSET;
      // Axe Y après les 2 octets joker + l'en-tête ; axe X décalé de 2 octets
      // pour aligner les étiquettes sur les données (convention de la web app).
      const yAxisAddress = signatureAddress + 4;
      const xAxisAddress = signatureAddress + 36;

      patches.push({
        address: signatureAddress + 2,
        data: generateLaunchControlYAxisBytes(),
        description: `Launch Control Y-axis @ 0x${(signatureAddress + 2)
          .toString(16)
          .toUpperCase()}`,
        createsMap: {
          name: 'Launch control map',
          address: mapDataAddress,
          size: 700,
          rows: 14,
          cols: 25,
          category: 'Launch control',
          subcategory: 'Launch control',
          correction_factor: 0.01,
          x_axis_correction: 1.0,
          y_axis_correction: 0.15625,
          x_axis_address: xAxisAddress,
          y_axis_address: yAxisAddress,
          x_label: 'Engine speed (rpm)',
          y_label: 'Vehicle speed (km/h)',
          unit: 'mg/st',
          description: 'IQ limit | X: Engine speed (rpm) | Y: Vehicle speed (km/h)',
          y_axis_inverted: true,
        },
      });

      offset = signatureAddress + 1;
    }

    return patches;
  },
};

const SOLUTION_CATEGORIES: SolutionCategory[] = [
  { id: 'performance', name: 'Performance', description: 'Solutions de performance' },
];

const EDC15_SOLUTIONS: Solution[] = [
  {
    id: 'launch_control',
    name: 'Launch Control',
    description: "Active le Launch Control en écrivant l'axe de vitesse véhicule",
    category: 'performance',
    icon: 'zap',
    credits: 0,
  },
];

const SOLUTION_IMPLEMENTATIONS: Record<string, SolutionImplementation> = {
  launch_control: launchControl,
};

/** Familles supportées : EDC15P et EDC15VM uniquement. */
export function getSolutionsForECU(ecuType: string | undefined): ECUSolutionsConfig | null {
  if (!ecuType) return null;
  const upper = ecuType.toUpperCase();
  if (!upper.includes('EDC15')) return null;

  return {
    ecuType: upper.includes('VM') || upper.includes('EDC15V') ? 'EDC15VM' : 'EDC15P',
    manufacturer: 'Bosch',
    solutions: EDC15_SOLUTIONS,
  };
}

export function getSolutionCategories(): SolutionCategory[] {
  return SOLUTION_CATEGORIES;
}

export function getSolutionImplementation(solutionId: string): SolutionImplementation | null {
  return SOLUTION_IMPLEMENTATIONS[solutionId] || null;
}

export function areSolutionsSupported(ecuType: string | undefined): boolean {
  return getSolutionsForECU(ecuType) !== null;
}

/**
 * Noms de cartes créées par chaque solution : sert à savoir si elle est déjà
 * active dans le fichier (le détecteur n'expose la carte que dans ce cas).
 */
export const solutionMapPatterns: Record<string, string[]> = {
  launch_control: ['launch control', 'launch-control', 'launchcontrol'],
};
