"use client";

import { useRef } from "react";
import { X } from "lucide-react";
import { StyledSelect } from "@/components/styled-select";
import {
  clearCustomWallpaper,
  getCustomWallpaper,
  setCustomWallpaperFromFile,
} from "@/lib/custom-wallpaper";
import { useTheme } from "@/contexts/theme-context";
import { useSettings, UserSettings } from "@/contexts/settings-context";
import { useI18n } from "@/contexts/i18n-context";
import { MODAL_GLASS, MODAL_GLASS_LIGHT } from "@/lib/modal-glass";

interface SettingsMenuProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess?: (message: string) => void;
  isClosing?: boolean;
}

type Theme = "default" | "light" | "oled";
type Wallpaper = UserSettings["editorWallpaper"];

/**
 * Menu paramètres de l'éditeur — chaque réglage s'applique IMMÉDIATEMENT
 * (comme la page paramètres du dashboard) : pas de Save/Cancel, la croix
 * ou le clic sur le fond ferment simplement le menu.
 */
export function SettingsMenu({ isOpen, onClose, isClosing = false }: SettingsMenuProps) {
  const { theme, setTheme } = useTheme();
  const { settings, updateSettings, saveSettings } = useSettings();
  const { t } = useI18n();

  // Application immédiate + persistance d'un réglage
  const apply = async (patch: Partial<UserSettings>) => {
    updateSettings(patch);
    await saveSettings({ ...settings, ...patch });
  };

  // ── Fond d'écran : mêmes règles que le dashboard ──────────────────
  // « Image personnalisée » est disponible avec TOUS les thèmes.
  const wallpaperChoices: { value: Wallpaper; label: string }[] =
    theme === "light"
      ? [
          { value: "auto", label: t.settingsPage.wallpaperAuto },
          { value: "white", label: t.settingsPage.wallpaperWhite },
          { value: "lines-light", label: t.settingsPage.wallpaperLinesLight },
          { value: "custom", label: t.settingsPage.wallpaperCustom },
        ]
      : [
          { value: "auto", label: t.settingsPage.wallpaperAuto },
          { value: "lines", label: t.settingsPage.wallpaperLines },
          { value: "editor", label: t.settingsPage.wallpaperEditor },
          { value: "black", label: t.settingsPage.wallpaperBlack },
          { value: "custom", label: t.settingsPage.wallpaperCustom },
        ];

  // Sélecteur de fichier pour l'image personnalisée (fond de l'éditeur)
  const wallpaperFileRef = useRef<HTMLInputElement>(null);
  const handleWallpaperChange = (value: Wallpaper) => {
    // Première sélection de « Image personnalisée » sans image enregistrée :
    // ouvrir le sélecteur ; le réglage n'est appliqué qu'après un choix.
    if (value === "custom" && !getCustomWallpaper("editor")) {
      wallpaperFileRef.current?.click();
      return;
    }
    void apply({ editorWallpaper: value });
  };
  const handleWallpaperFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = "";
    if (!file) return;
    try {
      await setCustomWallpaperFromFile("editor", file);
      void apply({ editorWallpaper: "custom" });
    } catch {
      // image illisible ou stockage plein : on ne change rien
    }
  };

  // Changement de thème : appliqué direct ; si le fond mémorisé devient
  // invalide pour ce thème, repli immédiat sur Automatique
  const handleThemeChange = (th: Theme) => {
    setTheme(th);
    const valid =
      th === "light"
        ? ["auto", "white", "lines-light", "custom"]
        : ["auto", "lines", "editor", "black", "custom"];
    if (!valid.includes(settings.editorWallpaper)) {
      void apply({ editorWallpaper: "auto" });
    }
  };

  // ── Le menu suit le thème appliqué ────────────────────────────────
  const menuLight = theme === "light";
  const surface = menuLight ? MODAL_GLASS_LIGHT : MODAL_GLASS;
  const textMain = menuLight ? "text-slate-900" : "text-white";
  const textSub = menuLight ? "text-slate-500" : "text-slate-400";
  const divider = menuLight ? "border-black/10" : "border-white/10";
  const cardBg = menuLight
    ? { backgroundColor: "rgba(0, 0, 0, 0.05)" }
    : { backgroundColor: "rgba(142, 142, 142, 0.13)" };
  const toggleOff = menuLight ? "rgba(0, 0, 0, 0.12)" : "rgba(255, 255, 255, 0.1)";
  const closeColor = menuLight ? "rgba(0, 0, 0, 0.5)" : "rgba(255, 255, 255, 0.6)";

  // Petit interrupteur réutilisable (application immédiate)
  const Toggle = ({
    id,
    checked,
    onColor,
    onChange,
  }: {
    id: string;
    checked: boolean;
    onColor: string;
    onChange: (v: boolean) => void;
  }) => (
    <div className="relative inline-flex items-center cursor-pointer">
      <input
        type="checkbox"
        id={id}
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="sr-only peer"
      />
      <div
        className="w-11 h-6 rounded-full relative transition-colors cursor-pointer"
        style={{ background: checked ? onColor : toggleOff }}
        onClick={() => onChange(!checked)}
      >
        <div
          className="absolute top-[2px] left-[2px] rounded-full h-5 w-5 transition-transform bg-white border border-white/30"
          style={{ transform: checked ? "translateX(20px)" : "translateX(0)" }}
        />
      </div>
    </div>
  );

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center backdrop-blur-sm"
      style={{
        backgroundColor: '#000000a2',
        animation: isClosing ? 'backdropFadeOut 0.2s ease-out forwards' : 'backdropFadeIn 0.2s ease-out forwards'
      }}
      onClick={onClose}
    >
      <div
        className="relative w-full max-w-4xl max-h-[95vh] overflow-y-auto upload-scroll"
        style={{
          animation: isClosing ? 'modalCollapse 0.2s ease-out forwards' : 'modalExpand 0.2s ease-out forwards'
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Close button */}
        <button
          onClick={onClose}
          className={`absolute top-4 right-4 z-10 p-2 rounded-lg transition-colors ${menuLight ? 'hover:bg-black/5' : 'hover:bg-white/5'}`}
          style={{ color: closeColor }}
        >
          <X className="w-5 h-5" />
        </button>

        {/* Settings Form */}
        <div className="max-w-4xl mx-auto">
          <div className="border rounded-lg p-8" style={surface}>
            <h2 className={`text-2xl font-bold mb-6 ${textMain}`}>{t.settings.title}</h2>

            <div className="space-y-6">
              {/* Display Settings Section */}
              <div className={`border-t ${divider} pt-6`}>
                <h3 className={`text-lg font-semibold mb-4 ${textMain}`}>{t.settings.mapDisplay}</h3>
                <div className="grid md:grid-cols-2 gap-4">
                  {/* Theme */}
                  <div className="p-4 rounded-lg" style={cardBg}>
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <label htmlFor="theme" className={`font-medium cursor-pointer text-sm ${textMain}`}>
                          {t.settings.theme}
                        </label>
                        <p className={`text-xs mt-1 ${textSub}`}>
                          {t.settings.themeDescription}
                        </p>
                      </div>
                      <div className="ml-4">
                        <StyledSelect
                          appearance="auto"
                          id="theme"
                          value={theme}
                          onChange={(value) => handleThemeChange(value as Theme)}
                          minWidth={140}
                          options={[
                            { value: "default", label: t.settings.themeDefault },
                            { value: "light", label: t.settings.themeLight },
                            { value: "oled", label: t.settings.themeOled },
                          ]}
                        />
                      </div>
                    </div>
                  </div>

                  {/* Fond d'écran de l'éditeur */}
                  <div className="p-4 rounded-lg" style={cardBg}>
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <label htmlFor="editor-wallpaper" className={`font-medium cursor-pointer text-sm ${textMain}`}>
                          {t.settingsPage.wallpaperSection}
                        </label>
                        <p className={`text-xs mt-1 ${textSub}`}>
                          {t.settingsPage.wallpaperDesc}
                        </p>
                      </div>
                      <div className="ml-4">
                        <StyledSelect
                          appearance="auto"
                          id="editor-wallpaper"
                          value={settings.editorWallpaper}
                          onChange={(value) => handleWallpaperChange(value as Wallpaper)}
                          minWidth={170}
                          options={wallpaperChoices.map((c) => ({ value: c.value, label: c.label }))}
                        />
                      </div>
                    </div>
                    {/* Image personnalisée active : changer / retirer */}
                    {settings.editorWallpaper === "custom" && (
                      <div className="flex gap-2 mt-3">
                        <button
                          className={`text-xs px-3 py-1.5 rounded-md border transition-colors ${menuLight ? "border-black/15 text-slate-700 hover:bg-black/5" : "border-white/15 text-slate-300 hover:bg-white/10"}`}
                          onClick={() => wallpaperFileRef.current?.click()}
                        >
                          {t.settingsPage.wallpaperCustomChange}
                        </button>
                        <button
                          className={`text-xs px-3 py-1.5 rounded-md border transition-colors ${menuLight ? "border-black/15 text-slate-700 hover:bg-black/5" : "border-white/15 text-slate-300 hover:bg-white/10"}`}
                          onClick={() => {
                            clearCustomWallpaper("editor");
                            void apply({ editorWallpaper: "auto" });
                          }}
                        >
                          {t.settingsPage.wallpaperCustomRemove}
                        </button>
                      </div>
                    )}
                    <input
                      ref={wallpaperFileRef}
                      type="file"
                      accept="image/*"
                      className="hidden"
                      onChange={(e) => void handleWallpaperFile(e)}
                    />
                  </div>

                  {/* EasyView Default */}
                  <div className="p-4 rounded-lg" style={cardBg}>
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <label htmlFor="easyview-default" className={`font-medium cursor-pointer text-sm ${textMain}`}>
                          {t.settings.easyViewDefault}
                        </label>
                        <p className={`text-xs mt-1 ${textSub}`}>
                          {t.settings.easyViewDescription}
                        </p>
                      </div>
                      <div className="ml-4">
                        <Toggle
                          id="easyview-default"
                          checked={settings.easyViewDefault}
                          onColor="rgba(34, 197, 94, 0.5)"
                          onChange={(v) => void apply({ easyViewDefault: v })}
                        />
                      </div>
                    </div>
                  </div>

                  {/* Disable Table Colors */}
                  <div className="p-4 rounded-lg" style={cardBg}>
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <label htmlFor="disable-table-colors" className={`font-medium cursor-pointer text-sm ${textMain}`}>
                          {t.settings.disableTableColors}
                        </label>
                        <p className={`text-xs mt-1 ${textSub}`}>
                          {t.settings.disableTableColorsDescription}
                        </p>
                      </div>
                      <div className="ml-4">
                        <Toggle
                          id="disable-table-colors"
                          checked={settings.disableTableColors}
                          onColor="rgba(239, 68, 68, 0.5)"
                          onChange={(v) => void apply({ disableTableColors: v })}
                        />
                      </div>
                    </div>
                  </div>

                  {/* Disable 3D Colors */}
                  <div className="p-4 rounded-lg" style={cardBg}>
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <label htmlFor="disable-3d-colors" className={`font-medium cursor-pointer text-sm ${textMain}`}>
                          {t.settings.disable3DColors}
                        </label>
                        <p className={`text-xs mt-1 ${textSub}`}>
                          {t.settings.disable3DColorsDescription}
                        </p>
                      </div>
                      <div className="ml-4">
                        <Toggle
                          id="disable-3d-colors"
                          checked={settings.disableGraphColors}
                          onColor="rgba(239, 68, 68, 0.5)"
                          onChange={(v) => void apply({ disableGraphColors: v })}
                        />
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              {/* Auto-Save Section */}
              <div className={`border-t ${divider} pt-6`}>
                <h3 className={`text-lg font-semibold mb-4 ${textMain}`}>{t.settings.autoSave}</h3>
                <div className="grid md:grid-cols-2 gap-4">
                  {/* Enable Auto-Save */}
                  <div className="p-4 rounded-lg" style={cardBg}>
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <label htmlFor="auto-save" className={`font-medium cursor-pointer text-sm ${textMain}`}>
                          {t.settings.autoSaveEnable}
                        </label>
                        <p className={`text-xs mt-1 ${textSub}`}>
                          {t.settings.autoSaveDescription}
                        </p>
                      </div>
                      <div className="ml-4">
                        <Toggle
                          id="auto-save"
                          checked={settings.autoSave}
                          onColor="rgba(59, 130, 246, 0.5)"
                          onChange={(v) => void apply({ autoSave: v })}
                        />
                      </div>
                    </div>
                  </div>

                  {/* Auto-Save Interval */}
                  <div className="p-4 rounded-lg" style={cardBg}>
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <label htmlFor="auto-save-interval" className={`font-medium cursor-pointer text-sm ${textMain}`}>
                          {t.settings.autoSaveInterval}
                        </label>
                      </div>
                      <div className="ml-4">
                        <StyledSelect
                          appearance="auto"
                          id="auto-save-interval"
                          value={settings.autoSaveInterval}
                          onChange={(value) => void apply({ autoSaveInterval: value })}
                          disabled={!settings.autoSave}
                          minWidth={140}
                          options={[
                            { value: "2min", label: "2 minutes" },
                            { value: "5min", label: "5 minutes" },
                            { value: "15min", label: "15 minutes" },
                            { value: "30min", label: "30 minutes" },
                          ]}
                        />
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              {/* Language Section */}
              <div className={`border-t ${divider} pt-6`}>
                <h3 className={`text-lg font-semibold mb-4 ${textMain}`}>{t.settings.language}</h3>
                <div className="grid md:grid-cols-2 gap-4">
                  {/* Language Selection */}
                  <div className="p-4 rounded-lg" style={cardBg}>
                    <div className="flex items-center justify-between">
                      <div className="flex-1">
                        <label htmlFor="language" className={`font-medium cursor-pointer text-sm ${textMain}`}>
                          {t.settings.displayLanguage}
                        </label>
                        <p className={`text-xs mt-1 ${textSub}`}>
                          {t.settings.displayLanguageDescription}
                        </p>
                      </div>
                      <div className="ml-4">
                        <StyledSelect
                          appearance="auto"
                          id="language"
                          value={settings.language}
                          onChange={(value) => void apply({ language: value })}
                          minWidth={140}
                          options={[
                            { value: "EN", label: "English" },
                            { value: "FR", label: "Français" },
                            { value: "ES", label: "Español" },
                            { value: "IT", label: "Italiano" },
                            { value: "DE", label: "Deutsch" },
                          ]}
                        />
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
