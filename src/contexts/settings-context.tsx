"use client";

import { createContext, useContext, useState, useEffect, ReactNode, useCallback } from "react";

export interface UserSettings {
  easyViewDefault: boolean;
  disableTableColors: boolean;
  disableGraphColors: boolean;
  autoSave: boolean;
  autoSaveInterval: string;
  language: string;
  /** Dashboard wallpaper: 'auto' follows the theme, or force one.
   *  'custom' = image choisie par l'utilisateur (lib/custom-wallpaper),
   *  utilisable avec TOUS les thèmes. */
  dashboardWallpaper: "auto" | "lines" | "lines-light" | "editor" | "white" | "black" | "custom";
  /** Editor wallpaper: same options and rules as the dashboard */
  editorWallpaper: "auto" | "lines" | "lines-light" | "editor" | "white" | "black" | "custom";
  /** Nom de l'entreprise de reprogrammation — affiché sur les PDF exportés */
  companyName: string;
}

// Detect browser language
const detectBrowserLanguage = (): string => {
  if (typeof window === "undefined") return "EN";
  const browserLang = navigator.language || (navigator as any).userLanguage || "en";
  const lang = browserLang.toLowerCase();
  if (lang.startsWith("fr")) return "FR";
  if (lang.startsWith("es")) return "ES";
  if (lang.startsWith("it")) return "IT";
  if (lang.startsWith("de")) return "DE";
  return "EN";
};

const defaultSettings: UserSettings = {
  easyViewDefault: false,
  disableTableColors: false,
  disableGraphColors: false,
  autoSave: false,
  autoSaveInterval: "15min",
  language: "EN", // Will be overridden by browser detection on first visit
  dashboardWallpaper: "auto",
  editorWallpaper: "auto",
  companyName: "",
};

export interface PlatformSettings {
  maxFileSizeMB: number;
  defaultCredits: number;
  siteName: string;
}

const defaultPlatformSettings: PlatformSettings = {
  maxFileSizeMB: 10,
  defaultCredits: 3,
  siteName: "ZedSuite",
};

interface SettingsContextType {
  settings: UserSettings;
  platform: PlatformSettings;
  updateSettings: (newSettings: Partial<UserSettings>) => void;
  saveSettings: (settingsToSave?: UserSettings) => Promise<void>;
  isLoading: boolean;
}

const SettingsContext = createContext<SettingsContextType | undefined>(undefined);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<UserSettings>(defaultSettings);
  const [platform, setPlatform] = useState<PlatformSettings>(defaultPlatformSettings);
  const [isLoading, setIsLoading] = useState(true);
  const [isInitialized, setIsInitialized] = useState(false);

  // Load settings on mount - localStorage first for instant response, then sync with API
  useEffect(() => {
    const loadSettings = async () => {
      // Detect browser language for first-time visitors
      const detectedLanguage = detectBrowserLanguage();

      // STEP 1: Load from localStorage FIRST for immediate response
      const savedSettings = localStorage.getItem("userSettings");
      let localSettings: UserSettings | null = null;

      if (savedSettings) {
        try {
          const parsed = JSON.parse(savedSettings);
          localSettings = {
            easyViewDefault: parsed.easyViewDefault ?? defaultSettings.easyViewDefault,
            disableTableColors: parsed.disableTableColors ?? defaultSettings.disableTableColors,
            disableGraphColors: parsed.disableGraphColors ?? defaultSettings.disableGraphColors,
            autoSave: parsed.autoSave ?? defaultSettings.autoSave,
            autoSaveInterval: parsed.autoSaveInterval ?? defaultSettings.autoSaveInterval,
            language: parsed.language ?? detectedLanguage,
            dashboardWallpaper: parsed.dashboardWallpaper ?? defaultSettings.dashboardWallpaper,
            editorWallpaper: parsed.editorWallpaper ?? defaultSettings.editorWallpaper,
            companyName: parsed.companyName ?? defaultSettings.companyName,
          };
          // Apply localStorage settings immediately
          setSettings(localSettings);
        } catch {
          // Invalid JSON in localStorage
          localSettings = null;
        }
      }

      // STEP 2: Then try to load from API (for first-time users or sync)
      try {
        const response = await fetch("/api/user/settings");
        if (response.ok) {
          // Check if response is JSON before parsing
          const contentType = response.headers.get("content-type");
          if (contentType && contentType.includes("application/json")) {
            const data = await response.json();
            const hasApiLanguage = data.language !== undefined && data.language !== null;

            // Only use API data if:
            // 1. No local settings exist (first time user)
            // 2. Or API has explicit language setting (user saved via another device/session)
            if (!localSettings) {
              setSettings({
                easyViewDefault: data.easyViewDefault ?? defaultSettings.easyViewDefault,
                disableTableColors: data.disableTableColors ?? defaultSettings.disableTableColors,
                disableGraphColors: data.disableGraphColors ?? defaultSettings.disableGraphColors,
                autoSave: data.autoSave ?? defaultSettings.autoSave,
                autoSaveInterval: data.autoSaveInterval ?? defaultSettings.autoSaveInterval,
                language: hasApiLanguage ? data.language : detectedLanguage,
                dashboardWallpaper: data.dashboardWallpaper ?? defaultSettings.dashboardWallpaper,
                editorWallpaper: data.editorWallpaper ?? defaultSettings.editorWallpaper,
                companyName: data.companyName ?? defaultSettings.companyName,
              });
            }
            // If local settings exist, keep them (they are more recent from user's last save)
          }
        }
      } catch (error) {
        // Silently fail - localStorage settings are already applied
        // Keep localStorage settings or use defaults
        if (!localSettings) {
          setSettings({ ...defaultSettings, language: detectedLanguage });
        }
      }

      setIsLoading(false);
      setIsInitialized(true);
    };

    const loadPlatformSettings = async () => {
      try {
        const res = await fetch("/api/settings/public");
        if (res.ok) {
          const data = await res.json();
          setPlatform({
            maxFileSizeMB: data.maxFileSizeMB ?? 10,
            defaultCredits: data.defaultCredits ?? 3,
            siteName: data.siteName ?? "ZedSuite",
          });
        }
      } catch {
        // Use defaults
      }
    };

    loadSettings();
    loadPlatformSettings();
  }, []);

  // Update settings locally
  const updateSettings = useCallback((newSettings: Partial<UserSettings>) => {
    setSettings(prev => ({ ...prev, ...newSettings }));
  }, []);

  // Save settings to database
  const saveSettings = useCallback(async (settingsToSave?: UserSettings) => {
    const dataToSave = settingsToSave || settings;
    try {
      // Save to localStorage first for immediate persistence
      localStorage.setItem("userSettings", JSON.stringify(dataToSave));

      // Then save to database
      await fetch("/api/user/settings", {
        method: "PUT",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(dataToSave),
      });
    } catch (error) {
      console.error("Failed to save settings:", error);
      // Still save to localStorage even if DB save fails
      localStorage.setItem("userSettings", JSON.stringify(dataToSave));
    }
  }, [settings]);

  // Auto-save to localStorage when settings change (after initial load)
  useEffect(() => {
    if (isInitialized) {
      localStorage.setItem("userSettings", JSON.stringify(settings));
    }
  }, [settings, isInitialized]);

  return (
    <SettingsContext.Provider value={{ settings, platform, updateSettings, saveSettings, isLoading }}>
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettings() {
  const context = useContext(SettingsContext);
  if (context === undefined) {
    throw new Error("useSettings must be used within a SettingsProvider");
  }
  return context;
}
