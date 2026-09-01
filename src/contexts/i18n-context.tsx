"use client";

import { createContext, useContext, ReactNode, useMemo } from "react";
import { useSettings } from "./settings-context";
import { translations } from "@/i18n/translations";

type Language = "EN" | "FR";
type TranslationType = (typeof translations)[Language];

interface I18nContextType {
  t: TranslationType;
  language: Language;
}

const I18nContext = createContext<I18nContextType | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const { settings } = useSettings();

  const language: Language = settings.language === "FR" ? "FR" : "EN";

  // Use language as the dependency, not t, because translations[language] always returns the same object reference
  const value = useMemo(() => ({
    t: translations[language],
    language
  }), [language]);

  return (
    <I18nContext.Provider value={value}>
      {children}
    </I18nContext.Provider>
  );
}

export function useI18n(): I18nContextType {
  const context = useContext(I18nContext);
  if (!context) {
    // Fallback if not in provider
    return { t: translations.EN, language: "EN" };
  }
  return context;
}

// Helper hook for getting translations
export function useTranslations() {
  const { t } = useI18n();
  return t;
}
