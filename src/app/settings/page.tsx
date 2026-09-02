"use client";

import { useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/button";
import { ArrowLeft, Bell, LayoutDashboard, Check, RefreshCw } from "lucide-react";
import { useI18n } from "@/contexts/i18n-context";
import { useSettings, type UserSettings } from "@/contexts/settings-context";
import { ThemeProvider, useTheme, type Theme } from "@/contexts/theme-context";
import { WindowControls } from "@/components/window-controls";
import { StyledSelect } from "@/components/styled-select";
import ZedGradientDefs from "@/components/zed-gradient-defs";
import {
  checkForUpdate,
  isTauri,
  markUpdateCheckDone,
  UPDATE_AVAILABLE_EVENT,
} from "@/lib/update";
import { setAppZoom, setAppMinWidth } from "@/lib/webview-zoom";
import {
  clearCustomWallpaper,
  getCustomWallpaper,
  setCustomWallpaperFromFile,
} from "@/lib/custom-wallpaper";

type Wallpaper = UserSettings["dashboardWallpaper"];

export default function SettingsPage() {
  return (
    // La page Paramètres s'affiche dans le thème du dashboard
    <ThemeProvider scope="dashboard">
      <SettingsContent />
    </ThemeProvider>
  );
}

function SettingsContent() {
  const router = useRouter();
  const { t, language } = useI18n();
  const { settings, updateSettings, saveSettings } = useSettings();
  // Thème du dashboard : provider monté (mise à jour immédiate de la page)
  const { theme: dashboardTheme, setTheme: setDashboardTheme } = useTheme();

  // ── Application : version + vérification de mise à jour ──
  const [appVersion, setAppVersion] = useState("—");
  const [updateStatus, setUpdateStatus] = useState<
    "idle" | "checking" | "uptodate" | "available" | "error"
  >("idle");
  const [latestVersion, setLatestVersion] = useState("");

  // Même échelle que le dashboard : page réduite à 90 % (zoom natif webview),
  // rendue à 100 % en quittant l'écran. Sans ça, revenir du dashboard
  // affichait les Paramètres à 100 % — plus gros et avec du scroll.
  useEffect(() => {
    setAppZoom(0.9);
    setAppMinWidth(1100, 0.9);
    return () => {
      setAppZoom(1);
    };
  }, []);

  useEffect(() => {
    if (!isTauri()) {
      setAppVersion("dev");
      return;
    }
    void import("@tauri-apps/api/app").then(({ getVersion }) =>
      getVersion().then(setAppVersion).catch(() => setAppVersion("—"))
    );
  }, []);

  const handleCheckUpdates = async () => {
    if (!isTauri() || updateStatus === "checking") return;
    setUpdateStatus("checking");
    try {
      const info = await checkForUpdate();
      markUpdateCheckDone();
      if (info.update_available) {
        setLatestVersion(info.latest_version);
        setUpdateStatus("available");
        // Ouvre la fenêtre de mise à jour (AppBootstrap l'affiche), même si
        // cette version avait été passée — la demande est explicite ici
        window.dispatchEvent(new CustomEvent(UPDATE_AVAILABLE_EVENT, { detail: info }));
      } else {
        setUpdateStatus("uptodate");
      }
    } catch {
      setUpdateStatus("error");
    }
  };

  const isLight = dashboardTheme === "light";
  const pageBg = dashboardTheme === "oled" ? "#000000" : isLight ? "#eef1f6" : "#0a0b0f";
  const panelStyle = isLight
    ? { backgroundColor: "rgba(255,255,255,0.75)" }
    : { backgroundColor: "rgba(22,25,34,0.55)" };
  const panelBorder = isLight ? "border-black/[0.08]" : "border-white/[0.08]";
  const titleColor = isLight ? "text-slate-900" : "text-white";
  const labelColor = isLight ? "text-slate-900" : "text-white";
  const descColor = isLight ? "text-slate-500" : "text-slate-400";
  const cardBg = isLight
    ? { backgroundColor: "rgba(0,0,0,0.05)" }
    : { backgroundColor: "rgba(142,142,142,0.13)" };

  const themeOptions = [
    { id: "default" as Theme, label: t.settingsPage.themeDefault, preview: "#0a0b0f", ring: "#312e81" },
    { id: "light" as Theme, label: t.settingsPage.themeLight, preview: "#eef1f6", ring: "#94a3b8" },
    { id: "oled" as Theme, label: t.settingsPage.themeOled, preview: "#000000", ring: "#334155" },
  ];

  const wallpaperOptions: { id: Wallpaper; label: string }[] = [
    { id: "auto", label: t.settingsPage.wallpaperAuto },
    { id: "lines", label: t.settingsPage.wallpaperLines },
    { id: "lines-light", label: t.settingsPage.wallpaperLinesLight },
    { id: "editor", label: t.settingsPage.wallpaperEditor },
    { id: "white", label: t.settingsPage.wallpaperWhite },
    { id: "black", label: t.settingsPage.wallpaperBlack },
    { id: "custom", label: t.settingsPage.wallpaperCustom },
  ];

  // Règles thème/fond (identiques dashboard et éditeur) : le blanc n'est
  // disponible QUE sur le thème clair ; sur clair, seuls Auto et Blanc.
  // L'image personnalisée est disponible avec TOUS les thèmes.
  const isWallpaperDisabled = (scopeTheme: Theme, id: Wallpaper) =>
    id === "custom"
      ? false
      : id === "white" || id === "lines-light"
        ? scopeTheme !== "light"
        : scopeTheme === "light" && id !== "auto";

  // Sélecteur de fichier pour l'image personnalisée du dashboard
  const wallpaperFileRef = useRef<HTMLInputElement>(null);
  const handleCustomWallpaperFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = "";
    if (!file) return;
    try {
      await setCustomWallpaperFromFile("dashboard", file);
      void setWallpaper("dashboardWallpaper", "custom");
    } catch {
      // image illisible ou stockage plein : on ne change rien
    }
  };

  // Indicateur de sélection : pastille dégradé rouge posée sur le coin
  // supérieur droit du bouton — jamais dans la zone de texte, donc aucun
  // chevauchement ni décalage du libellé centré.
  const SelectedBadge = () => (
    <span
      className="absolute -top-1.5 -right-1.5 w-4 h-4 rounded-full flex items-center justify-center shadow"
      style={{ background: "linear-gradient(135deg, #dc2626, #ef4444 50%, #f97316)" }}
    >
      <Check className="w-2.5 h-2.5 text-white" strokeWidth={3.5} />
    </span>
  );

  // La partie entre parenthèses d'un libellé de fond (« (selon le thème) »)
  // est rendue insécable : elle passe entière sur sa propre ligne au lieu de
  // se couper au milieu quand le bouton est étroit.
  const renderWallpaperLabel = (label: string) => {
    const idx = label.indexOf(" (");
    if (idx === -1) return label;
    return (
      <>
        {label.slice(0, idx)}{" "}
        <span className="whitespace-nowrap">{label.slice(idx + 1)}</span>
      </>
    );
  };

  const setWallpaper = async (key: "dashboardWallpaper" | "editorWallpaper", w: Wallpaper) => {
    updateSettings({ [key]: w });
    await saveSettings({ ...settings, [key]: w });
  };

  // Changement de thème : si le fond mémorisé devient invalide pour le
  // nouveau thème (ex. blanc ou traits clairs hors thème clair), la
  // sélection revient automatiquement sur Automatique
  const handleDashboardTheme = (th: Theme) => {
    setDashboardTheme(th);
    const valid =
      th === "light"
        ? ["auto", "white", "lines-light", "custom"]
        : ["auto", "lines", "editor", "black", "custom"];
    if (!valid.includes(settings.dashboardWallpaper)) {
      void setWallpaper("dashboardWallpaper", "auto");
    }
  };

  // Un groupe complet Thème + Fond d'écran (réutilisé pour les deux portées)
  const renderGroup = (
    icon: React.ReactNode,
    title: string,
    scopeTheme: Theme,
    onTheme: (th: Theme) => void,
    wallpaperKey: "dashboardWallpaper" | "editorWallpaper"
  ) => (
    <div className={`backdrop-blur-xl border ${panelBorder} rounded-2xl p-5`} style={{ ...panelStyle, animation: 'slideInFromBottom 0.6s ease-out backwards' }}>
      <div className="flex items-center gap-4 mb-4">
        {icon}
        <h2 className={`text-xl font-bold ${titleColor}`}>{title}</h2>
      </div>

      {/* Thème */}
      <p className={`text-sm ${descColor} mb-2`}>{t.settingsPage.themeSection}</p>
      <div className="grid grid-cols-3 gap-3 mb-4">
        {themeOptions.map((opt) => (
          <button
            key={opt.id}
            onClick={() => onTheme(opt.id)}
            className={`relative rounded-xl border p-3 transition-all ${
              scopeTheme === opt.id
                ? "border-red-500/60 ring-1 ring-red-500/40"
                : isLight ? "border-black/10 hover:border-black/25" : "border-white/10 hover:border-white/25"
            }`}
            style={cardBg}
          >
            <div
              className="h-10 rounded-lg mb-2 border"
              style={{ backgroundColor: opt.preview, borderColor: opt.ring }}
            >
              <div className="h-1.5 w-1/2 rounded-full mt-2.5 ml-3" style={{ backgroundColor: opt.id === "light" ? "#334155" : "#e2e8f0", opacity: 0.7 }} />
              <div className="h-1.5 w-1/3 rounded-full mt-1.5 ml-3" style={{ backgroundColor: opt.id === "light" ? "#64748b" : "#94a3b8", opacity: 0.5 }} />
            </div>
            {scopeTheme === opt.id && <SelectedBadge />}
            <div className="flex items-center justify-center">
              <span className={`text-sm font-medium ${labelColor}`}>{opt.label}</span>
            </div>
          </button>
        ))}
      </div>

      {/* Fond d'écran */}
      <p className={`text-sm ${descColor} mb-2`}>{t.settingsPage.wallpaperSection}</p>
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
        {wallpaperOptions.map((opt) => {
          const disabled = isWallpaperDisabled(scopeTheme, opt.id);
          const selected = settings[wallpaperKey] === opt.id;
          return (
            <button
              key={opt.id}
              onClick={() => {
                if (disabled) return;
                // « Image personnalisée » sans image enregistrée : ouvrir le
                // sélecteur ; le réglage n'est appliqué qu'après un choix.
                if (opt.id === "custom" && !getCustomWallpaper("dashboard")) {
                  wallpaperFileRef.current?.click();
                  return;
                }
                void setWallpaper(wallpaperKey, opt.id);
              }}
              disabled={disabled}
              className={`relative rounded-xl border px-3 py-2.5 transition-all text-center ${
                disabled
                  ? "opacity-35 cursor-not-allowed " + (isLight ? "border-black/10" : "border-white/10")
                  : selected
                    ? "border-red-500/60 ring-1 ring-red-500/40"
                    : isLight ? "border-black/10 hover:border-black/25" : "border-white/10 hover:border-white/25"
              }`}
              style={cardBg}
            >
              {selected && !disabled && <SelectedBadge />}
              <div className="flex items-center justify-center">
                <span className={`text-sm font-medium ${labelColor}`}>{renderWallpaperLabel(opt.label)}</span>
              </div>
            </button>
          );
        })}
      </div>
      {/* Image personnalisée active : changer / retirer */}
      {settings[wallpaperKey] === "custom" && (
        <div className="flex gap-2 mt-3">
          <button
            className={`text-xs px-3 py-1.5 rounded-md border transition-colors ${isLight ? "border-black/15 text-slate-700 hover:bg-black/5" : "border-white/15 text-slate-300 hover:bg-white/10"}`}
            onClick={() => wallpaperFileRef.current?.click()}
          >
            {t.settingsPage.wallpaperCustomChange}
          </button>
          <button
            className={`text-xs px-3 py-1.5 rounded-md border transition-colors ${isLight ? "border-black/15 text-slate-700 hover:bg-black/5" : "border-white/15 text-slate-300 hover:bg-white/10"}`}
            onClick={() => {
              clearCustomWallpaper("dashboard");
              void setWallpaper(wallpaperKey, "auto");
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
        onChange={(e) => void handleCustomWallpaperFile(e)}
      />
    </div>
  );

  return (
    <div className="min-h-screen relative overflow-hidden" style={{ backgroundColor: pageBg }}>
      {/* Dégradé de marque pour les icônes de section (stroke url(#zedIconGradient)) */}
      <ZedGradientDefs />
      {/* Décor signature de l'app derrière les panneaux de verre (thème
          sombre uniquement) : halos flous + grain, version atténuée du fond
          de l'éditeur. La base reste noire/blanche selon le thème ; OLED
          reste noir pur, le thème clair reste épuré comme le dashboard. */}
      {dashboardTheme === "default" && (
        <>
          <div aria-hidden className="fixed inset-0 z-0 pointer-events-none overflow-hidden" style={{ opacity: 0.4 }}>
            <div className="absolute rounded-full" style={{ width: 520, height: 520, left: -120, top: -140, filter: 'blur(90px)', background: 'radial-gradient(circle, #ef444488, transparent 70%)' }} />
            <div className="absolute rounded-full" style={{ width: 620, height: 620, right: -160, top: 120, filter: 'blur(90px)', background: 'radial-gradient(circle, #2563eb77, transparent 70%)' }} />
            <div className="absolute rounded-full" style={{ width: 480, height: 480, left: '32%', bottom: -200, filter: 'blur(90px)', background: 'radial-gradient(circle, #7c3aed66, transparent 70%)' }} />
          </div>
          <div aria-hidden className="fixed inset-0 z-0 pointer-events-none" style={{
            opacity: 0.04,
            backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='140' height='140'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2'/%3E%3C/filter%3E%3Crect width='140' height='140' filter='url(%23n)' opacity='0.55'/%3E%3C/svg%3E")`
          }} />
        </>
      )}
      {/* Header — frameless-window title bar (drag region) */}
      <header data-tauri-drag-region className="relative z-10">
        <div data-tauri-drag-region className="pl-4 pr-2 py-2.5">
          <div data-tauri-drag-region className="flex items-center justify-between">
            <div data-tauri-drag-region className="flex items-center gap-4 min-w-0 overflow-hidden">
              <Button
                variant="ghost"
                size="icon"
                onClick={() => router.push("/dashboard")}
                className={isLight ? "text-slate-500 hover:text-black hover:bg-black/10" : "text-slate-400 hover:text-white hover:bg-white/10"}
              >
                <ArrowLeft className="w-5 h-5" />
              </Button>
              <h1 data-tauri-drag-region className={`text-xl font-bold select-none ${titleColor}`}>{t.settingsPage.title}</h1>
            </div>
            <WindowControls />
          </div>
        </div>
      </header>

      {/* Main Content */}
      {/* max-w-5xl : assez large pour que « Automatique (selon le thème) »
          tienne sur deux lignes propres dans la rangée de 6 fonds */}
      <main className="relative z-10 container mx-auto px-4 py-5 max-w-5xl">
        <div className="space-y-4">
          {/* Dashboard : thème + fond. Le thème/fond de l'ÉDITEUR se règle
              dans le menu paramètres de l'éditeur lui-même. */}
          {renderGroup(
            <LayoutDashboard className="w-6 h-6" style={{ stroke: "url(#zedIconGradient)" }} />,
            t.settingsPage.dashboardSection,
            dashboardTheme,
            handleDashboardTheme,
            "dashboardWallpaper"
          )}

          {/* Preferences Section */}
          <div className={`backdrop-blur-xl border ${panelBorder} rounded-2xl p-5`} style={{ ...panelStyle, animation: 'slideInFromBottom 0.6s ease-out backwards' }}>
            <div className="flex items-center gap-4 mb-4">
              <Bell className="w-6 h-6" style={{ stroke: "url(#zedIconGradient)" }} />
              <h2 className={`text-xl font-bold ${titleColor}`}>{t.settingsPage.preferences}</h2>
            </div>

            <div className="space-y-3">
              <div className={`flex items-center justify-between p-3 rounded-lg border ${isLight ? 'border-black/[0.06]' : 'border-white/[0.06]'}`} style={cardBg}>
                <div>
                  <div className={`font-medium ${labelColor}`}>{t.settingsPage.language}</div>
                  <div className={`text-sm ${descColor}`}>{t.settingsPage.languageDesc}</div>
                </div>
                <StyledSelect
                  appearance="auto"
                  id="language"
                  value={language}
                  minWidth={140}
                  onChange={(value) => {
                    const newLang = value as "EN" | "FR" | "ES" | "IT" | "DE";
                    updateSettings({ language: newLang });
                    void saveSettings({ ...settings, language: newLang });
                  }}
                  options={[
                    { value: "EN", label: "English" },
                    { value: "FR", label: "Français" },
                    { value: "ES", label: "Español" },
                    { value: "IT", label: "Italiano" },
                    { value: "DE", label: "Deutsch" },
                  ]}
                />
              </div>

              {/* Nom de l'entreprise de reprogrammation — repris sur les PDF */}
              <div className={`flex items-center justify-between gap-6 p-3 rounded-lg border ${isLight ? 'border-black/[0.06]' : 'border-white/[0.06]'}`} style={cardBg}>
                <div>
                  <div className={`font-medium ${labelColor}`}>{t.settingsPage.companyName}</div>
                  <div className={`text-sm ${descColor}`}>{t.settingsPage.companyNameDesc}</div>
                </div>
                <input
                  type="text"
                  value={settings.companyName}
                  onChange={(e) => updateSettings({ companyName: e.target.value })}
                  onBlur={async () => {
                    await saveSettings({ ...settings });
                  }}
                  placeholder={t.settingsPage.companyNamePlaceholder}
                  spellCheck={false}
                  className={`w-64 px-4 py-2 rounded-lg focus:outline-none focus:ring-2 focus:ring-red-500/60 ${
                    isLight
                      ? "bg-black/[0.05] border border-black/[0.10] text-slate-900 placeholder:text-slate-400"
                      : "bg-white/[0.05] border border-white/[0.10] text-white placeholder:text-slate-500"
                  }`}
                />
              </div>
            </div>
          </div>

          {/* Application : version + mises à jour */}
          <div className={`backdrop-blur-xl border ${panelBorder} rounded-2xl p-5`} style={{ ...panelStyle, animation: 'slideInFromBottom 0.6s ease-out backwards' }}>
            <div className="flex items-center gap-4 mb-4">
              <RefreshCw className="w-6 h-6" style={{ stroke: "url(#zedIconGradient)" }} />
              <h2 className={`text-xl font-bold ${titleColor}`}>{t.settingsPage.appSection}</h2>
            </div>

            <div className="space-y-3">
              <div className={`flex items-center justify-between p-3 rounded-lg border ${isLight ? 'border-black/[0.06]' : 'border-white/[0.06]'}`} style={cardBg}>
                <div>
                  <div className={`font-medium ${labelColor}`}>{t.settingsPage.version}</div>
                  <div className={`text-sm ${descColor}`}>{t.settingsPage.versionDesc}</div>
                </div>
                <span className={`px-4 py-2 rounded-lg font-mono text-sm ${isLight ? "bg-black/[0.05] text-slate-900" : "bg-white/[0.05] text-white"}`}>
                  v{appVersion}
                </span>
              </div>

              <div className={`flex items-center justify-between gap-6 p-3 rounded-lg border ${isLight ? 'border-black/[0.06]' : 'border-white/[0.06]'}`} style={cardBg}>
                <div>
                  <div className={`font-medium ${labelColor}`}>{t.settingsPage.updates}</div>
                  <div className={`text-sm ${descColor}`}>
                    {updateStatus === "uptodate"
                      ? t.settingsPage.upToDate
                      : updateStatus === "available"
                        ? `${t.settingsPage.updateAvailable} ${latestVersion}`
                        : updateStatus === "error"
                          ? t.settingsPage.updateCheckError
                          : t.settingsPage.updatesDesc}
                  </div>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-9 px-4 flex-shrink-0"
                  onClick={handleCheckUpdates}
                  disabled={updateStatus === "checking"}
                >
                  {updateStatus === "checking" ? (
                    <>
                      <RefreshCw className="w-4 h-4 mr-2 animate-spin" />
                      {t.settingsPage.checking}
                    </>
                  ) : (
                    t.settingsPage.checkNow
                  )}
                </Button>
              </div>
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}
