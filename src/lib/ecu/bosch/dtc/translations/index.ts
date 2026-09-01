/**
 * DTC Translations Index
 *
 * Provides multilingual DTC descriptions.
 * To add a new language:
 * 1. Create a new file (e.g., de.ts for German)
 * 2. Export dtcTranslationsDE with all translations
 * 3. Import and add to dtcTranslations object below
 */

import { dtcTranslationsFR } from './fr';
import { dtcTranslationsEN } from './en';

export type DTCLanguageCode = 'EN' | 'FR';

export const dtcTranslations: Record<DTCLanguageCode, Record<string, string>> = {
  EN: dtcTranslationsEN,
  FR: dtcTranslationsFR,
};

/**
 * Get DTC description in specified language
 * Falls back to French if translation not found
 */
export function getDTCDescription(code: string, language: DTCLanguageCode = 'FR'): string {
  const translation = dtcTranslations[language]?.[code];
  if (translation) return translation;

  // Fallback to French
  const frenchTranslation = dtcTranslations['FR']?.[code];
  if (frenchTranslation) return frenchTranslation;

  // Return code if no translation found
  return code;
}

/**
 * Check if a translation exists for a code
 */
export function hasDTCTranslation(code: string, language: DTCLanguageCode = 'FR'): boolean {
  return !!dtcTranslations[language]?.[code];
}

/**
 * Get all supported languages
 */
export function getSupportedDTCLanguages(): DTCLanguageCode[] {
  return Object.keys(dtcTranslations) as DTCLanguageCode[];
}

export { dtcTranslationsFR, dtcTranslationsEN };
