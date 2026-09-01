"use client";

import { createContext, useContext, useState, useEffect, ReactNode } from "react";

export type Theme = "default" | "light" | "oled";

const VALID: Theme[] = ["default", "light", "oled"];
const THEME_EVENT = "zedsuite-theme-changed";

/** Read a scope's stored theme (falls back to the legacy single key). */
export function getStoredTheme(scope: string): Theme {
  if (typeof window === "undefined") return "default";
  const v = (localStorage.getItem(`userTheme_${scope}`) ||
    localStorage.getItem("userTheme")) as Theme | null;
  return v && VALID.includes(v) ? v : "default";
}

/** Persist a scope's theme and notify the mounted provider of that scope. */
export function setStoredTheme(scope: string, theme: Theme): void {
  if (typeof window === "undefined") return;
  localStorage.setItem(`userTheme_${scope}`, theme);
  window.dispatchEvent(new CustomEvent(THEME_EVENT, { detail: { scope, theme } }));
}

interface ThemeContextType {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  saveTheme: () => Promise<void>;
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

/**
 * Scoped theme provider — the dashboard and the editor keep INDEPENDENT
 * themes (scope "dashboard" / scope "editor"), stored as userTheme_<scope>
 * in localStorage. The settings page can change any scope through
 * setStoredTheme(); a mounted provider picks the change up live.
 */
export function ThemeProvider({
  children,
  scope = "app",
}: {
  children: ReactNode;
  scope?: string;
}) {
  const [theme, setThemeState] = useState<Theme>("default");

  useEffect(() => {
    setThemeState(getStoredTheme(scope));
    const onChange = (e: Event) => {
      const d = (e as CustomEvent).detail;
      if (d?.scope === scope && VALID.includes(d.theme)) {
        setThemeState(d.theme);
      }
    };
    window.addEventListener(THEME_EVENT, onChange);
    return () => window.removeEventListener(THEME_EVENT, onChange);
  }, [scope]);

  const setTheme = (t: Theme) => {
    setThemeState(t);
    setStoredTheme(scope, t);
  };

  // No-op kept for API compatibility: persistence already happens inside
  // setTheme. (Writing here would use a stale closure of `theme` and undo
  // the change the caller just applied — the editor settings menu does
  // setTheme(newTheme) then saveTheme().)
  const saveTheme = async () => {};

  return (
    <ThemeContext.Provider value={{ theme, setTheme, saveTheme }}>
      {children}
    </ThemeContext.Provider>
  );
}

export function useTheme() {
  const context = useContext(ThemeContext);
  if (context === undefined) {
    throw new Error("useTheme must be used within a ThemeProvider");
  }
  return context;
}

/** Like useTheme but returns undefined outside a provider (used by
 *  components rendered both inside and outside themed pages). */
export function useThemeOptional() {
  return useContext(ThemeContext);
}
