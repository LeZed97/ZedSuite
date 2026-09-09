/**
 * Solution « EGR OFF » pour Bosch EDC16CP31 (Mercedes OM642 / OM646).
 *
 * ── Provenance ───────────────────────────────────────────────────────────
 * Rejeu à l'octet près d'une désactivation EGR réalisée par un préparateur
 * professionnel, vérifiée sur une PAIRE réelle : lecture d'origine du même
 * calculateur et relecture du calculateur après flash (OM642 3.0 CDI 165 kW,
 * CLK 320 CDI W209, SW Bosch 1037393817). 21 plages, 60 octets, tous dans le
 * bloc EGR. Aucune autre zone du fichier n'est touchée — ni suralimentation,
 * ni rail, ni avance, ni limiteurs.
 *
 * La lecture après flash a confirmé que le contenu de calibration flashé est
 * exactement celui analysé ici ; seule la queue de région (signature de
 * programmation + mot de checksum) diffère, et elle est régénérée à
 * l'écriture par l'outil de flash.
 *
 * ── Ce que ça fait ───────────────────────────────────────────────────────
 * Cœur fonctionnel :
 *   AirCtl_qLo_CUR         seuil de coupure EGR par quantité injectée → 0
 *                          partout (40/40/40/40/43/37/37 mm³ à l'origine),
 *                          donc EGR coupée sur toute la plage.
 *   EGRCD_swtEGROnVal_C    interrupteur EGR : 1 → 0.
 *   EGRCD_rCnv_MAP         1ʳᵉ ligne de la conversion en rapport cyclique :
 *                          95 % → 5 %, vanne verrouillée fermée.
 *   EGRCD_rInit_C          rapport cyclique appliqué au contact : 12 % → 5 %.
 *
 * Diagnostics :
 *   EGRCD_Deb*             antirebonds surtempérature, chute de charge et
 *   EGRCD_tiDia*           courts-circuits → 0x0000 ou 0xFFFF. 0xFFFF vaut
 *                          655 s : un défaut devrait persister onze minutes
 *                          d'affilée pour se poser.
 *   4 octets DSM_*         conditions de suppression et classe de défaut.
 *                          NOMMAGE INCERTAIN : l'alignement damos donne deux
 *                          candidats pour chacun de ces octets. Ils sont
 *                          rejoués parce qu'ils font partie de la
 *                          modification vérifiée, pas parce que leur rôle
 *                          exact est établi.
 *
 * ── Ce que ça NE fait PAS ────────────────────────────────────────────────
 * Les cartes de consigne d'air EGR (AirCtl_mDesBas, mDesValCor, mNSCor,
 * AirCtl_rEGR) restent INTACTES — c'est le choix du préparateur : on tue
 * l'actionneur plutôt que de réécrire les consignes. Le régulateur d'air
 * garde donc une cible qu'il ne peut plus atteindre. Le défaut d'écart de
 * régulation dépend de AirCtl_mMaxDvt / mMinDvt, qui ne font pas partie des
 * octets modifiés : rien ici ne garantit que ce chemin de défaut est couvert.
 *
 * ── Sécurité ─────────────────────────────────────────────────────────────
 * Corpus d'UN logiciel. Les adresses ne sont donc JAMAIS écrites en dur :
 * chaque groupe est ancré sur une signature d'octets vérifiée unique dans le
 * fichier, et chaque écriture porte la valeur d'origine attendue. Au moindre
 * écart — ancre introuvable, octets déjà différents, solution déjà appliquée
 * — la solution ne renvoie AUCUN patch plutôt que d'écrire à l'aveugle.
 *
 * Le checksum n'est pas corrigé ici : l'éditeur le recalcule (module EDC16,
 * région 0x190000..0x1FCFFB, somme cible 0xD01FE500).
 */

import type { BinaryPatch, SolutionImplementation } from './index';

interface PatchWrite {
  /** Décalage depuis la FIN de l'ancre. */
  at: number;
  /** Octets attendus à cet emplacement sur un fichier d'origine. */
  expect: number[];
  write: number[];
}

interface PatchGroup {
  id: string;
  /** Octets précédant immédiatement la zone modifiée, uniques dans le dump. */
  anchor: number[];
  writes: PatchWrite[];
}

// Table générée depuis la paire vérifiée (origine / relecture voiture).
// Les adresses en commentaire sont celles du SW 1037393817, à titre de
// repère uniquement : la localisation se fait par l'ancre.
const EGR_OFF_GROUPS: PatchGroup[] = [
  {
    id: 'AirCtl_qLo_CUR',
    // 14 octets à 0x195DBC : fin de l'axe X de la courbe (bloc Kl)
    anchor: [0x01, 0xf4, 0x03, 0x20, 0x03, 0xe8, 0x07, 0xd0, 0x09, 0xc4, 0x0a, 0x28, 0x0b, 0xb8],
    writes: [
      // 0x195DCA — les 7 valeurs Z de la courbe
      {
        at: 0,
        expect: [0x0f, 0xa0, 0x0f, 0xa0, 0x0f, 0xa0, 0x0f, 0xa0, 0x10, 0xcc, 0x0e, 0x74, 0x0e, 0x74],
        write: [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
      },
    ],
  },
  {
    id: 'DSM_suppression_1',
    // 12 octets à 0x1994C7
    anchor: [0x01, 0x01, 0x02, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    writes: [{ at: 0, expect: [0x07], write: [0x00] }], // 0x1994D3
  },
  {
    id: 'DSM_suppression_2',
    // 14 octets à 0x1995F1
    anchor: [0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    writes: [{ at: 0, expect: [0x07, 0x07], write: [0x00, 0x00] }], // 0x1995FF
  },
  {
    id: 'DSM_fault_class',
    // 8 octets à 0x19A6F7
    anchor: [0x00, 0x00, 0x05, 0x04, 0x04, 0x04, 0x00, 0x05],
    writes: [{ at: 0, expect: [0x03], write: [0x00] }], // 0x19A6FF
  },
  {
    id: 'EGRCD_debounce_and_duty',
    // 8 octets à 0x1A3288
    anchor: [0x00, 0x02, 0x01, 0x02, 0x00, 0x04, 0x13, 0x74],
    writes: [
      { at: 0, expect: [0x00, 0xc8], write: [0xff, 0xff] }, // 0x1A3290 antirebonds
      { at: 3, expect: [0x0a, 0x00, 0xc8], write: [0x00, 0xff, 0xff] }, // 0x1A3293
      { at: 7, expect: [0x0a, 0x00, 0xc8], write: [0x00, 0xff, 0xff] }, // 0x1A3297
      { at: 11, expect: [0x0a], write: [0x00] }, // 0x1A329B
      { at: 13, expect: [0xe6, 0xff, 0xff], write: [0x00, 0x00, 0x00] }, // 0x1A329D
      { at: 18, expect: [0x01, 0x0c, 0x01], write: [0x00, 0x00, 0x00] }, // 0x1A32A2
      { at: 24, expect: [0x01, 0xf4], write: [0x00, 0x00] }, // 0x1A32A8
      { at: 54, expect: [0x25, 0x1c], write: [0x01, 0xf4] }, // 0x1A32C6 rCnv 95 %→5 %
      { at: 70, expect: [0x25, 0x1c], write: [0x01, 0xf4] }, // 0x1A32D6
      { at: 86, expect: [0x25, 0x1c], write: [0x01, 0xf4] }, // 0x1A32E6
      { at: 102, expect: [0x25, 0x1c], write: [0x01, 0xf4] }, // 0x1A32F6
    ],
  },
  {
    id: 'EGRCD_switch_and_diag',
    // 8 octets à 0x1A34EA
    anchor: [0x13, 0x88, 0x13, 0x88, 0x01, 0xf4, 0x01, 0xf4],
    writes: [
      { at: 0, expect: [0x04, 0xb0, 0x25, 0x1c, 0x01, 0xf4], write: [0x01, 0xf4, 0x00, 0x00, 0x00, 0x00] }, // 0x1A34F2 rInit
      { at: 22, expect: [0x01], write: [0x00] }, // 0x1A3508 swtEGROnVal
      { at: 33, expect: [0x04, 0x00, 0x1e, 0xff, 0xff], write: [0x00, 0xff, 0xff, 0x00, 0x00] }, // 0x1A3513
      { at: 42, expect: [0x00, 0x28], write: [0xff, 0xff] }, // 0x1A351C
      { at: 46, expect: [0x00, 0xdc], write: [0xff, 0xff] }, // 0x1A3520
      { at: 49, expect: [0x32], write: [0x00] }, // 0x1A3523
    ],
  },
];

/** Cherche une séquence exacte et exige qu'elle soit UNIQUE. */
function findUnique(fileData: Uint8Array, seq: number[]): number {
  let found = -1;
  outer: for (let p = 0; p + seq.length <= fileData.length; p++) {
    for (let i = 0; i < seq.length; i++) {
      if (fileData[p + i] !== seq[i]) continue outer;
    }
    if (found !== -1) return -1; // deux occurrences : ancre non fiable
    found = p;
  }
  return found;
}

export const edc16cp31EgrOff: SolutionImplementation = {
  id: 'edc16cp31_egr_off',
  name: 'EGR OFF',
  description:
    "Désactive la vanne EGR et neutralise ses défauts (rejeu d'une modification vérifiée sur OM642 SW 1037393817)",

  applyBinaryPatches: (fileData: Uint8Array): BinaryPatch[] => {
    const patches: BinaryPatch[] = [];

    for (const group of EGR_OFF_GROUPS) {
      const anchorAt = findUnique(fileData, group.anchor);
      if (anchorAt === -1) return []; // ancre absente ou ambiguë → on n'écrit rien
      const base = anchorAt + group.anchor.length;

      for (const w of group.writes) {
        const at = base + w.at;
        if (at + w.expect.length > fileData.length) return [];
        for (let i = 0; i < w.expect.length; i++) {
          // Octets déjà différents : fichier d'un autre logiciel, déjà
          // modifié, ou solution déjà appliquée. Dans les trois cas on
          // s'arrête — un patch partiel serait pire que pas de patch.
          if (fileData[at + i] !== w.expect[i]) return [];
        }
        patches.push({
          address: at,
          data: [...w.write],
          description: `EGR OFF — ${group.id} @ 0x${at.toString(16).toUpperCase()}`,
        });
      }
    }

    return patches;
  },
};
