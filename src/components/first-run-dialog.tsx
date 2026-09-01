"use client";

// First-launch setup dialog — shown once (zedsuiteFirstRunDone flag).
// Asks for the tuner/company name (optional, printed on exported PDFs),
// the language and the app theme (applied to BOTH the dashboard and the
// editor scopes). Completing it triggers a background update check.

import { useState } from "react";
import { Check, Rocket } from "lucide-react";
import { Button } from "@/components/ui/button";
import { MODAL_GLASS } from "@/lib/modal-glass";
import { useI18n } from "@/contexts/i18n-context";
import { useSettings } from "@/contexts/settings-context";
import { setStoredTheme, type Theme } from "@/contexts/theme-context";

interface FirstRunDialogProps {
  onDone: () => void;
}

export function FirstRunDialog({ onDone }: FirstRunDialogProps) {
  const { t } = useI18n();
  const { settings, updateSettings, saveSettings } = useSettings();
  const [tunerName, setTunerName] = useState(settings.companyName);
  const [theme, setTheme] = useState<Theme>("default");

  const themeOptions = [
    { id: "default" as Theme, label: t.settingsPage.themeDefault, preview: "#0a0b0f", ring: "#312e81" },
    { id: "light" as Theme, label: t.settingsPage.themeLight, preview: "#eef1f6", ring: "#94a3b8" },
    { id: "oled" as Theme, label: t.settingsPage.themeOled, preview: "#000000", ring: "#334155" },
  ];

  // Language switches live so the dialog itself changes language
  const setLanguage = (lang: "EN" | "FR") => {
    updateSettings({ language: lang });
  };

  const finish = async () => {
    const next = { ...settings, companyName: tunerName.trim() };
    updateSettings({ companyName: next.companyName });
    await saveSettings(next);
    // One theme choice applies to both scopes; each stays adjustable later
    setStoredTheme("dashboard", theme);
    setStoredTheme("editor", theme);
    onDone();
  };

  return (
    <div
      className="fixed inset-0 z-[95] flex items-center justify-center backdrop-blur-md"
      style={{ backgroundColor: "#000000c8" }}
    >
      <div
        className="relative w-full max-w-xl mx-4"
        style={{ animation: "modalExpand 0.25s ease-out forwards" }}
      >
        <div className="border rounded-lg p-8" style={MODAL_GLASS}>
          <div className="flex items-center gap-3 mb-2">
            <div className="p-2 rounded-full bg-red-500/20">
              <Rocket className="w-6 h-6 text-red-400" />
            </div>
            <h2 className="text-2xl font-bold text-white">{t.firstRun.title}</h2>
          </div>
          <p className="text-sm text-slate-400 mb-6">{t.firstRun.subtitle}</p>

          {/* Nom du tuner (optionnel) */}
          <div className="mb-5">
            <div className="font-medium text-white mb-1">{t.firstRun.tunerName}</div>
            <div className="text-sm text-slate-400 mb-2">{t.firstRun.tunerNameDesc}</div>
            <input
              type="text"
              value={tunerName}
              onChange={(e) => setTunerName(e.target.value)}
              placeholder={t.settingsPage.companyNamePlaceholder}
              spellCheck={false}
              className="w-full px-4 py-2 rounded-lg bg-white/[0.05] border border-white/[0.10] text-white placeholder:text-slate-500 focus:outline-none focus:ring-2 focus:ring-red-500"
            />
          </div>

          {/* Langue */}
          <div className="mb-5">
            <div className="font-medium text-white mb-2">{t.firstRun.language}</div>
            <div className="grid grid-cols-2 gap-3">
              {([
                { id: "EN" as const, label: "English" },
                { id: "FR" as const, label: "Français" },
              ]).map((opt) => {
                const selected = (settings.language === "FR" ? "FR" : "EN") === opt.id;
                return (
                  <button
                    key={opt.id}
                    onClick={() => setLanguage(opt.id)}
                    className={`rounded-xl border px-4 py-3 transition-all text-center ${
                      selected
                        ? "border-red-500/60 ring-1 ring-red-500/40"
                        : "border-white/10 hover:border-white/25"
                    }`}
                    style={{ backgroundColor: "rgba(142,142,142,0.13)" }}
                  >
                    <div className="flex items-center justify-center gap-2">
                      {selected && <Check className="w-4 h-4 text-red-400" />}
                      <span className="text-sm font-medium text-white">{opt.label}</span>
                    </div>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Thème */}
          <div className="mb-7">
            <div className="font-medium text-white mb-2">{t.firstRun.theme}</div>
            <div className="grid grid-cols-3 gap-3">
              {themeOptions.map((opt) => (
                <button
                  key={opt.id}
                  onClick={() => setTheme(opt.id)}
                  className={`relative rounded-xl border p-3 transition-all ${
                    theme === opt.id
                      ? "border-red-500/60 ring-1 ring-red-500/40"
                      : "border-white/10 hover:border-white/25"
                  }`}
                  style={{ backgroundColor: "rgba(142,142,142,0.13)" }}
                >
                  <div
                    className="h-10 rounded-lg mb-2 border"
                    style={{ backgroundColor: opt.preview, borderColor: opt.ring }}
                  >
                    <div
                      className="h-1.5 w-1/2 rounded-full mt-2.5 ml-2.5"
                      style={{ backgroundColor: opt.id === "light" ? "#334155" : "#e2e8f0", opacity: 0.7 }}
                    />
                  </div>
                  <div className="flex items-center justify-center gap-2">
                    {theme === opt.id && <Check className="w-4 h-4 text-red-400" />}
                    <span className="text-sm font-medium text-white">{opt.label}</span>
                  </div>
                </button>
              ))}
            </div>
          </div>

          <div className="flex justify-end">
            <Button
              size="sm"
              className="h-10 px-8 text-white bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400 shadow-lg shadow-red-500/25"
              onClick={finish}
            >
              {t.firstRun.start}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
