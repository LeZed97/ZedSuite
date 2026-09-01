/**
 * DTC Internationalization Helper
 * Provides bilingual access to DTC descriptions and system names
 *
 * Uses translations from i18n/translations.ts for system names
 * Uses translations/ folder for DTC descriptions
 */

import {
  getBilingualDTCInfoByPCode,
  getDTCInfoByVagCode,
  DTC_SYSTEM_KEYS,
} from './dtc-database';
import { translations } from '@/i18n/translations';
import { getDTCDescription } from './translations';

// Type for supported languages
export type DTCLanguage = 'EN' | 'FR';

// Interface for translated DTC info
export interface TranslatedDTCInfo {
  code: string;
  description: string;
  system: string;
  systemKey: string;
}

// Use translations from i18n/translations.ts (single source of truth)
const getSystemNames = (language: DTCLanguage): Record<string, string> => {
  return translations[language].dtcSystems;
};

// French system name to key mapping (for reverse lookup from existing data)
const SYSTEM_FR_TO_KEY: Record<string, string> = {
  Carburant: 'FUEL',
  Admission: 'INTAKE',
  Echappement: 'EXHAUST',
  Échappement: 'EXHAUST',
  Moteur: 'ENGINE',
  Refroidissement: 'COOLING',
  Allumage: 'IGNITION',
  Emission: 'EMISSION',
  Émission: 'EMISSION',
  Turbo: 'TURBO',
  EGR: 'EGR',
  'FAP/DPF': 'DPF',
  Injecteur: 'INJECTOR',
  'Sonde Lambda': 'LAMBDA',
  Papillon: 'THROTTLE',
  Transmission: 'TRANSMISSION',
  Electrique: 'ELECTRICAL',
  Électrique: 'ELECTRICAL',
  Communication: 'CAN',
  Capteur: 'SENSOR',
  Regulateur: 'CRUISE',
  Régulateur: 'CRUISE',
  Climatisation: 'CLIMATE',
  'Bougie Prechauffage': 'GLOW_PLUG',
  'Bougie Préchauffage': 'GLOW_PLUG',
  Catalyseur: 'CATALYST',
  Canister: 'EVAP',
  Autre: 'OTHER',
};

/**
 * Get the system key from a French system name
 */
function getSystemKeyFromFrench(frenchSystem: string): string {
  return SYSTEM_FR_TO_KEY[frenchSystem] || 'OTHER';
}

/**
 * Get the translated system name for a given key and language
 */
export function getSystemName(systemKey: string, language: DTCLanguage = 'FR'): string {
  const names = getSystemNames(language);
  return names[systemKey as keyof typeof names] || systemKey;
}

/**
 * Get the translated system name from a French system name
 */
export function translateSystemName(frenchSystem: string, language: DTCLanguage = 'FR'): string {
  if (language === 'FR') return frenchSystem;
  const key = getSystemKeyFromFrench(frenchSystem);
  const names = getSystemNames('EN');
  return names[key as keyof typeof names] || frenchSystem;
}

/**
 * Get DTC info by P-code with language-specific description
 */
export function getDTCInfoByPCodeTranslated(
  pCode: string,
  language: DTCLanguage = 'FR'
): TranslatedDTCInfo | null {
  const bilingualInfo = getBilingualDTCInfoByPCode(pCode);
  if (!bilingualInfo) return null;

  return {
    code: bilingualInfo.code,
    description: language === 'EN' ? bilingualInfo.descriptionEN : bilingualInfo.descriptionFR,
    system: getSystemName(bilingualInfo.systemKey, language),
    systemKey: bilingualInfo.systemKey,
  };
}

/**
 * Get DTC info by VAG code with language-specific description
 */
export function getDTCInfoByVagCodeTranslated(
  vagCode: number,
  language: DTCLanguage = 'FR'
): TranslatedDTCInfo | null {
  const baseInfo = getDTCInfoByVagCode(vagCode);
  if (!baseInfo) return null;

  // Get the bilingual info using the P-code
  const bilingualInfo = getBilingualDTCInfoByPCode(baseInfo.code);
  if (!bilingualInfo) {
    // Fallback to base info
    const systemKey = getSystemKeyFromFrench(baseInfo.system);
    return {
      code: baseInfo.code,
      description: baseInfo.description,
      system: getSystemName(systemKey, language),
      systemKey,
    };
  }

  return {
    code: bilingualInfo.code,
    description: language === 'EN' ? bilingualInfo.descriptionEN : bilingualInfo.descriptionFR,
    system: getSystemName(bilingualInfo.systemKey, language),
    systemKey: bilingualInfo.systemKey,
  };
}

/**
 * Translate a DTC description based on P-code and language
 * Uses the new translations system from translations/ folder
 */
export function translateDTCDescription(
  pCode: string,
  frenchDescription: string,
  language: DTCLanguage = 'FR'
): string {
  // Use the new translations system
  const translation = getDTCDescription(pCode, language);

  // If we got a translation (not just the code back), use it
  if (translation !== pCode) {
    return translation;
  }

  // Fallback to French description if no translation found
  return frenchDescription;
}

// Re-export for convenience
export { DTC_SYSTEM_KEYS };
