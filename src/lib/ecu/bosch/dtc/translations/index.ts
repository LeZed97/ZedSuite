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
import { dtcTranslationsES } from './es';
import { dtcTranslationsIT } from './it';
import { dtcTranslationsDE } from './de';

export type DTCLanguageCode = 'EN' | 'FR' | 'ES' | 'IT' | 'DE';

export const dtcTranslations: Record<DTCLanguageCode, Record<string, string>> = {
  EN: dtcTranslationsEN,
  FR: dtcTranslationsFR,
  ES: dtcTranslationsES,
  IT: dtcTranslationsIT,
  DE: dtcTranslationsDE,
};

/**
 * Get DTC description in specified language
 * Falls back to English, then French, if the translation is missing
 */
export function getDTCDescription(code: string, language: DTCLanguageCode = 'FR'): string {
  const translation = dtcTranslations[language]?.[code];
  if (translation) return translation;

  // Fallback: English (the reference file), then French
  const englishTranslation = dtcTranslations['EN']?.[code];
  if (englishTranslation) return englishTranslation;
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

export { dtcTranslationsFR, dtcTranslationsEN, dtcTranslationsES, dtcTranslationsIT, dtcTranslationsDE };
