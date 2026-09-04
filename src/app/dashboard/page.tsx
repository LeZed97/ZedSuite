"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import type { FileRecord } from "@/lib/types";
import * as store from "@/lib/local/store";
import { Button } from "@/components/ui/button";
import { StyledSelect } from "@/components/styled-select";
import { MODAL_GLASS, MODAL_GLASS_LIGHT } from "@/lib/modal-glass";
import { useToast } from "@/hooks/use-toast";
import { ProjectCreator } from "@/components/project-creator";
import { WindowControls } from "@/components/window-controls";
import { PowerEstimateModal } from "@/components/power-estimate-modal";
import ZedGradientDefs, { ZedFileIcon } from "@/components/zed-gradient-defs";
import { ThemeProvider, useTheme } from "@/contexts/theme-context";
import { setAppZoom, setAppMinWidth } from "@/lib/webview-zoom";
import { useI18n } from "@/contexts/i18n-context";
import { useSettings } from "@/contexts/settings-context";
import { DashboardBackground, useDashboardWallpaper } from "@/components/dashboard-background";
import {
  Upload,
  FileText,
  ArrowRight,
  Trash2,
  X,
  MoreVertical,
  FolderOpen,
  Search,
  Settings,
  HelpCircle,
  User,
  Gauge,
  AlertTriangle,
} from "lucide-react";

type File = FileRecord;

// Function to get icon styles based on stage
function getStageIconStyles(stage?: string, isLight?: boolean) {
  switch (stage) {
    case 'Stage 1':
      return {
        background: 'bg-green-500/20',
        border: 'border-green-500/30',
        numberColor: 'text-green-500',
        stageNumber: '1'
      };
    case 'Stage 2':
      return {
        background: 'bg-yellow-500/20',
        border: 'border-yellow-500/30',
        numberColor: 'text-yellow-500',
        stageNumber: '2'
      };
    case 'Stage 3':
      return {
        background: 'bg-red-500/20',
        border: 'border-red-500/30',
        numberColor: 'text-red-500',
        stageNumber: '3'
      };
    default:
      // Thème clair : le gris translucide rend mal sur carte claire — même
      // surface que les autres éléments clairs de l'app (noir très léger).
      return {
        background: isLight ? 'bg-black/[0.04]' : 'bg-slate-500/20',
        border: isLight ? 'border-black/10' : 'border-slate-500/30',
        numberColor: null,
        stageNumber: null
      };
  }
}

// Project Info Edit Modal Component
function ProjectInfoEditModal({
  file,
  onClose,
  onSave,
  isClosing = false,
  t,
}: {
  file: File;
  onClose: () => void;
  onSave: (updatedInfo: Partial<File>) => void;
  isClosing?: boolean;
  t: any;
}) {
  // La modale suit le thème de son écran (dashboard ou éditeur)
  const { theme } = useTheme();
  const L = theme === 'light';
  const labelCls = `block text-sm font-medium mb-2 ${L ? 'text-slate-900' : 'text-white'}`;
  const inputCls = `w-full px-3 py-2 rounded-lg focus:outline-none focus:ring-0 ${L ? 'bg-black/[0.05] border border-black/20 text-slate-900 placeholder:text-black/40' : 'bg-black/15 border border-white/20 text-white placeholder:text-white/50'}`;
  const inputSmCls = `w-full px-2 py-2 rounded-lg text-sm focus:outline-none focus:ring-0 ${L ? 'bg-black/[0.05] border border-black/20 text-slate-900 placeholder:text-black/40' : 'bg-black/15 border border-white/20 text-white placeholder:text-white/50'}`;
  const [projectName, setProjectName] = useState(file.project_name || file.original_name || "");
  const [vehicleBrand, setVehicleBrand] = useState(file.vehicle_brand || "");
  const [vehicleModel, setVehicleModel] = useState(file.vehicle_model || "");
  const [engineType, setEngineType] = useState(file.engine_type || "");
  const [transmissionType, setTransmissionType] = useState(file.transmission_type || "");
  const [year, setYear] = useState(file.year || "");
  const [power, setPower] = useState(file.power || "");
  const [customer, setCustomer] = useState(file.customer || "");
  const [stage, setStage] = useState(file.stage || "");
  const [notes, setNotes] = useState(file.notes || "");
  const date = file.date || new Date(file.created).toISOString().split('T')[0];

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSave({
      project_name: projectName,
      vehicle_brand: vehicleBrand,
      vehicle_model: vehicleModel,
      engine_type: engineType,
      transmission_type: transmissionType,
      year: year,
      power: power,
      customer: customer,
      stage: stage,
      notes: notes,
    });
  };

  const brands = ["Audi", "Seat", "Skoda", "Volkswagen"];
  const stages = ["Stage 1", "Stage 2", "Stage 3"];
  const transmissions = ["Automatic", "Manual"];
  const years = Array.from({ length: new Date().getFullYear() - 1996 }, (_, i) => new Date().getFullYear() - i);

  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center backdrop-blur-sm"
      style={{
        backgroundColor: '#000000a2',
        animation: isClosing ? 'backdropFadeOut 0.2s ease-out forwards' : 'backdropFadeIn 0.2s ease-out forwards'
      }}
      // Ne se ferme JAMAIS au clic sur le fond — uniquement via les boutons
      onClick={(e) => e.stopPropagation()}
    >
      <div
        className="relative w-full max-w-4xl max-h-[95vh] overflow-y-auto upload-scroll"
        style={{
          animation: isClosing ? 'modalCollapse 0.2s ease-out forwards' : 'modalExpand 0.2s ease-out forwards'
        }}
      >
        {/* Close button */}
        <button
          onClick={onClose}
          className={`absolute top-4 right-4 z-10 p-2 rounded-lg transition-colors ${L ? 'hover:bg-black/5' : 'hover:bg-white/5'}`}
          style={{ color: L ? 'rgba(0, 0, 0, 0.5)' : 'rgba(255, 255, 255, 0.6)' }}
        >
          <X className="w-5 h-5" />
        </button>

        {/* Project Info Form */}
        <div className="max-w-4xl mx-auto">
          <div className="border rounded-lg p-8" style={L ? MODAL_GLASS_LIGHT : MODAL_GLASS}>
            <h2 className={`text-2xl font-bold mb-6 ${L ? "text-slate-900" : "text-white"}`}>{t.projectInfo.title}</h2>

            <form onSubmit={handleSubmit} className="space-y-6">
              {/* File Info Section */}
              <div>
                <label className={labelCls}>{t.projectInfo.ecuFile}</label>
                <div className="p-4 rounded-lg border" style={{ backgroundColor: L ? 'rgba(0, 0, 0, 0.05)' : 'rgba(142, 142, 142, 0.13)' }}>
                  <div className="flex items-center gap-4">
                    {(() => {
                      const iconStyles = getStageIconStyles(stage, L);
                      return (
                        <div className={`w-12 h-12 shrink-0 rounded-xl ${iconStyles.background} flex items-center justify-center border ${iconStyles.border}`}>
                          {iconStyles.stageNumber ? (
                            <span className="text-xl font-bold" style={{ fontStyle: 'italic' }}>
                              <span className={L ? "text-slate-900" : "text-white"}>ST</span>
                              <span className={iconStyles.numberColor}>{iconStyles.stageNumber}</span>
                            </span>
                          ) : (
                            <ZedFileIcon className="w-6 h-6" barColor={L ? "#334155" : "#ffffff"} />
                          )}
                        </div>
                      );
                    })()}
                    <div className="flex-1 min-w-0">
                      <h4 className={`font-semibold truncate ${L ? "text-slate-900" : "text-white"}`} title={file.original_name}>{file.original_name}</h4>
                      <div className="flex items-center gap-4">
                        <p className="text-sm text-slate-400">
                          {(file.file_size / 1024).toFixed(2)} KB
                        </p>
                        <div className="flex items-center gap-4 flex-1 justify-center">
                          {file.hardware_version && (
                            <div className="flex items-center gap-2 text-sm text-slate-400">
                              <span>HW:</span>
                              <span>{file.hardware_version}</span>
                            </div>
                          )}
                          {file.software_version && (
                            <div className="flex items-center gap-2 text-sm text-slate-400">
                              <span>SW:</span>
                              <span>{file.software_version}</span>
                            </div>
                          )}
                        </div>
                      </div>
                    </div>
                    {file.ecu_type && (
                      <span className={`text-xs px-2 py-0.5 rounded-full text-white ${L ? "bg-gradient-to-r from-red-600 via-red-500 to-orange-500 border border-red-600/50" : "bg-gradient-to-r from-red-600/50 via-red-500/50 to-orange-500/50 border border-red-500/40"}`}>
                        {file.ecu_type}
                      </span>
                    )}
                  </div>
                </div>
              </div>

              {/* Project Name */}
              <div>
                <label className={labelCls}>{t.projectInfo.projectName}</label>
                <input
                  type="text"
                  value={projectName}
                  onChange={(e) => setProjectName(e.target.value)}
                  className={inputCls}
                  placeholder={t.projectInfo.projectNamePlaceholder}
                  spellCheck={false}
                />
              </div>

              {/* Vehicle Information */}
              <div className="border-t pt-6">
                <h3 className={`text-lg font-semibold mb-4 ${L ? "text-slate-900" : "text-white"}`}>{t.projectInfo.vehicleInfo}</h3>
                <div className="grid md:grid-cols-3 gap-3">
                  {/* Première ligne: Brand - Model - Year */}
                  <div>
                    <label className={labelCls}>{t.projectInfo.brand}</label>
                    <StyledSelect
                      appearance="auto"
                      value={vehicleBrand}
                      onChange={setVehicleBrand}
                      className="w-full"
                      options={[
                        { value: "", label: t.projectInfo.select },
                        ...brands.map((brand) => ({ value: brand, label: brand })),
                      ]}
                    />
                  </div>

                  <div>
                    <label className={labelCls}>{t.projectInfo.model}</label>
                    <input
                      type="text"
                      value={vehicleModel}
                      onChange={(e) => setVehicleModel(e.target.value)}
                      className={inputSmCls}
                      placeholder={t.projectInfo.modelPlaceholder}
                      spellCheck={false}
                    />
                  </div>

                  <div>
                    <label className={labelCls}>{t.projectInfo.year}</label>
                    <StyledSelect
                      appearance="auto"
                      value={String(year)}
                      onChange={setYear}
                      className="w-full"
                      options={[
                        { value: "", label: t.projectInfo.select },
                        ...years.map((y) => ({ value: String(y), label: String(y) })),
                      ]}
                    />
                  </div>

                  {/* Deuxième ligne: Engine Type - Power (HP) - Transmission */}
                  <div>
                    <label className={labelCls}>{t.projectInfo.engineType}</label>
                    <input
                      type="text"
                      value={engineType}
                      onChange={(e) => setEngineType(e.target.value)}
                      className={inputSmCls}
                      placeholder={t.projectInfo.engineTypePlaceholder}
                      spellCheck={false}
                    />
                  </div>

                  <div>
                    <label className={labelCls}>{t.projectInfo.power}</label>
                    <input
                      type="text"
                      value={power}
                      onChange={(e) => setPower(e.target.value)}
                      className={inputSmCls}
                      placeholder={t.projectInfo.powerPlaceholder}
                      spellCheck={false}
                    />
                  </div>

                  <div>
                    <label className={labelCls}>{t.projectInfo.transmission}</label>
                    <StyledSelect
                      appearance="auto"
                      value={transmissionType}
                      onChange={setTransmissionType}
                      className="w-full"
                      options={[
                        { value: "", label: t.projectInfo.select },
                        { value: "Automatic", label: t.projectInfo.automatic },
                        { value: "Manual", label: t.projectInfo.manual },
                      ]}
                    />
                  </div>

                  {/* Troisième ligne: Customer - Stage - Date */}
                  <div>
                    <label className={labelCls}>{t.projectInfo.customer}</label>
                    <input
                      type="text"
                      value={customer}
                      onChange={(e) => setCustomer(e.target.value)}
                      className={inputSmCls}
                      placeholder={t.projectInfo.customerPlaceholder}
                      spellCheck={false}
                    />
                  </div>

                  <div>
                    <label className={labelCls}>{t.projectInfo.stage}</label>
                    <StyledSelect
                      appearance="auto"
                      value={stage}
                      onChange={setStage}
                      className="w-full"
                      options={[
                        { value: "", label: t.projectInfo.select },
                        ...stages.map((s) => ({ value: s, label: s })),
                      ]}
                    />
                  </div>

                  <div>
                    <label className={labelCls}>{t.projectInfo.date}</label>
                    <input
                      type="date"
                      value={date}
                      readOnly
                      className={`${inputSmCls} cursor-not-allowed opacity-75`}
                    />
                  </div>
                </div>
              </div>

              {/* Notes */}
              <div>
                <label className={labelCls}>{t.projectInfo.notes}</label>
                <textarea
                  value={notes}
                  onChange={(e) => setNotes(e.target.value)}
                  rows={3}
                  className={`${inputCls} resize-none`}
                  placeholder={t.projectInfo.notesPlaceholder}
                  spellCheck={false}
                />
              </div>

              {/* Action Buttons */}
              <div className="flex gap-3 pt-4">
                <Button
                  type="submit"
                  className="flex-1 bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400 text-white"
                >
                  {t.common.save}
                </Button>
                <Button
                  type="button"
                  onClick={onClose}
                  variant="outline"
                  className="flex-1"
                >
                  {t.common.close}
                </Button>
              </div>
            </form>
          </div>
        </div>
      </div>
    </div>
  );
}

// Le dashboard suit le même système de thèmes que l'éditeur (défaut / clair /
// OLED) — le provider vit ici, le contenu consomme useTheme.
export default function DashboardPage() {
  return (
    <ThemeProvider scope="dashboard">
      <ZedGradientDefs />
      <DashboardContent />
    </ThemeProvider>
  );
}

function DashboardContent() {
  const [files, setFiles] = useState<File[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  // Affichage du dashboard réduit à 90 % (zoom natif webview — les vh
  // suivent, pas de trou en bas). Rendu à 100 % en quittant l'écran.
  // Vrai dès que la page est défilée : pilote l'apparition du voile derrière
  // l'en-tête fixe (voir plus bas).
  const [isScrolled, setIsScrolled] = useState(false);
  useEffect(() => {
    const onScroll = () => setIsScrolled(window.scrollY > 4);
    onScroll();
    window.addEventListener('scroll', onScroll, { passive: true });
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  useEffect(() => {
    setAppZoom(0.9);
    // Le dashboard n'a pas la barre d'outils de l'éditeur : il se contente
    // d'une largeur plus modeste, elle aussi ramenée à l'échelle du zoom.
    setAppMinWidth(1100, 0.9);
    return () => {
      setAppZoom(1);
    };
  }, []);
  const [showUploadModal, setShowUploadModal] = useState(false);
  const [isClosingUploadModal, setIsClosingUploadModal] = useState(false);
  const [selectedFileInfo, setSelectedFileInfo] = useState<File | null>(null);
  const [isClosingProjectInfo, setIsClosingProjectInfo] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [itemsPerPage, setItemsPerPage] = useState(7);
  const [currentPage, setCurrentPage] = useState(1);
  const [pageTransition, setPageTransition] = useState(false);
  const [versionCounts, setVersionCounts] = useState<Record<string, number>>({});
  const [showDeleteConfirmModal, setShowDeleteConfirmModal] = useState(false);
  const [isClosingDeleteConfirmModal, setIsClosingDeleteConfirmModal] = useState(false);
  const [fileToDelete, setFileToDelete] = useState<File | null>(null);
  const [showAbout, setShowAbout] = useState(false);
  const [powerFile, setPowerFile] = useState<File | null>(null);
  const router = useRouter();
  const { toast } = useToast();
  const { t, language } = useI18n();

  const handleUploadClick = () => {
    setShowUploadModal(true);
  };
  const { settings, updateSettings, saveSettings } = useSettings();
  const { theme } = useTheme();

  // Fond d'écran (réglage + thème + image personnalisée) : logique partagée
  // avec la page Paramètres (components/dashboard-background)
  const { wallpaper, isLight, pageBg, customWallpaper } = useDashboardWallpaper(settings.dashboardWallpaper, theme);
  // Image personnalisée sur thème sombre : les surfaces translucides se
  // noient dans une photo claire → tuiles et recherche un peu plus opaques,
  // en gardant l'image visible derrière comme sur le thème clair (bg-white/60)
  const onCustomDark = wallpaper === "custom" && !isLight;
  // Image personnalisée sur thème clair : boutons du haut, pagination et
  // sélecteur du bas en noir (pas de pastille, demande Enzo)
  const onCustomLight = wallpaper === "custom" && isLight;
  const paginationText = onCustomLight ? "text-slate-900" : "text-slate-700";
  // Image personnalisée : les gris se perdent sur une photo → tous les textes
  // secondaires (dates, HW/SW, client, « Versions : », pagination, compteur)
  // et les icônes en blanc sur thème sombre, en noir sur thème clair
  const subText = onCustomDark ? "text-white" : onCustomLight ? "text-slate-900" : null;
  const darkIcon = onCustomDark ? "text-white hover:text-white hover:bg-white/10" : "text-slate-400 hover:text-white hover:bg-white/10";

  useEffect(() => {
    loadDashboardData();
  }, []);

  const handleCloseUploadModal = () => {
    setIsClosingUploadModal(true);
    setTimeout(() => {
      setShowUploadModal(false);
      setIsClosingUploadModal(false);
    }, 200);
  };

  const handleCloseProjectInfo = () => {
    setIsClosingProjectInfo(true);
    setTimeout(() => {
      setSelectedFileInfo(null);
      setIsClosingProjectInfo(false);
    }, 200);
  };

  const loadDashboardData = async () => {
    try {
      // Read all projects from the local store (app data directory)
      const filesData = await store.listFiles();
      setFiles(filesData);

      // Get version counts for each file
      const counts: Record<string, number> = {};
      for (const file of filesData) {
        try {
          const versions = await store.listVersions(file.id);
          counts[file.id] = versions.length || 1;
        } catch (err) {
          console.error(`Failed to fetch version count for file ${file.id}:`, err);
          counts[file.id] = 1; // Default to 1 if error
        }
      }
      setVersionCounts(counts);
    } catch (error: any) {
      console.error("Error loading dashboard:", error);
      toast({
        title: t.dashboard.error,
        description: t.dashboard.unableToLoadDashboard,
        variant: "destructive",
      });
    } finally {
      setIsLoading(false);
    }
  };

  const handleOpenProject = async (file: File) => {
    try {
      setIsLoading(true);

      // Récupérer les versions du fichier (les plus récentes d'abord)
      const versionItems = [...(await store.listVersions(file.id))].sort(
        (a, b) => (b.created > a.created ? 1 : -1)
      );

      const versions = versionItems.map((v: any) => ({
        id: v.id,
        fileId: v.file,
        name: v.name,
        isCurrent: v.is_current,
        baseVersionId: v.base_version || null,
        createdAt: v.created,
      }));

      const currentVersion = versions.find(v => v.isCurrent);

      // Note: We don't load file_data here anymore - the editor page will load it from PocketBase
      // This avoids QuotaExceededError since binary data (1-2MB) exceeds sessionStorage limits

      // Parser detection_data si c'est une chaîne JSON
      let detectionResults = { maps: [], total_maps: 0, processing_time_ms: 0 };
      if (file.detection_data) {
        try {
          detectionResults = typeof file.detection_data === 'string'
            ? JSON.parse(file.detection_data)
            : file.detection_data;
        } catch (err) {
          console.error("Failed to parse detection_data:", err);
        }
      }

      // Stocker les métadonnées du projet dans sessionStorage (SANS file_data pour éviter QuotaExceededError)
      // file_data sera chargé par l'éditeur via /api/versioning/file-data/[fileId]
      sessionStorage.setItem("currentProject", JSON.stringify({
        project_name: file.project_name || file.file_name,
        file_name: file.original_name,
        original_name: file.original_name,
        // Note: file_data is NOT stored here - it will be loaded from PocketBase in the editor
        file_size: file.file_size,
        ecu_type: file.ecu_type,
        vehicle_brand: file.vehicle_brand,
        vehicle_model: file.vehicle_model,
        engine_type: file.engine_type,
        transmission_type: file.transmission_type,
        year: file.year,
        power: file.power,
        customer: file.customer,
        stage: file.stage,
        notes: file.notes,
        hardware_version: file.hardware_version,
        software_version: file.software_version,
        created: file.created,
        detectionResults: detectionResults,
        fileId: file.id,
        currentVersionId: currentVersion?.id || null,
        versions: versions,
      }));

      // Rediriger vers l'éditeur
      router.push(`/editor?project=${encodeURIComponent(file.file_name)}`);
      // Ne pas désactiver isLoading ici pour éviter le flash de retour au dashboard
    } catch (error: any) {
      console.error("Error opening project:", error);
      setIsLoading(false); // Désactiver seulement en cas d'erreur
      toast({
        title: t.dashboard.error,
        description: t.dashboard.unableToOpenProject,
        variant: "destructive",
      });
    }
  };

  const handleDeleteProject = (file: File, e: React.MouseEvent) => {
    e.stopPropagation();
    setFileToDelete(file);
    setShowDeleteConfirmModal(true);
  };

  const handleCloseDeleteConfirmModal = () => {
    setIsClosingDeleteConfirmModal(true);
    setTimeout(() => {
      setShowDeleteConfirmModal(false);
      setIsClosingDeleteConfirmModal(false);
      setFileToDelete(null);
    }, 200);
  };

  const handleConfirmDelete = async () => {
    if (!fileToDelete) return;

    try {
      setIsLoading(true);
      handleCloseDeleteConfirmModal();

      // Supprimer le dossier projet complet (binaire, versions, éditions)
      await store.deleteFile(fileToDelete.id);

      toast({
        title: t.dashboard.projectDeleted,
        description: t.dashboard.projectDeletedDescription,
      });

      // Recharger la liste des fichiers
      await loadDashboardData();
    } catch (error: any) {
      console.error("Error deleting project:", error);
      toast({
        title: t.dashboard.error,
        description: t.dashboard.unableToDeleteProject,
        variant: "destructive",
      });
    } finally {
      setIsLoading(false);
    }
  };

  // Filter files based on search query
  const filteredFiles = files.filter((file) => {
    if (!searchQuery.trim()) return true;

    const query = searchQuery.toLowerCase();
    return (
      file.project_name?.toLowerCase().includes(query) ||
      file.original_name?.toLowerCase().includes(query) ||
      file.hardware_version?.toLowerCase().includes(query) ||
      file.software_version?.toLowerCase().includes(query) ||
      file.ecu_type?.toLowerCase().includes(query) ||
      file.vehicle_brand?.toLowerCase().includes(query) ||
      file.vehicle_model?.toLowerCase().includes(query) ||
      file.engine_type?.toLowerCase().includes(query) ||
      file.customer?.toLowerCase().includes(query) ||
      file.stage?.toLowerCase().includes(query) ||
      file.year?.toLowerCase().includes(query)
    );
  });

  // Calculate pagination
  const totalPages = Math.ceil(filteredFiles.length / itemsPerPage);
  const startIndex = (currentPage - 1) * itemsPerPage;
  const endIndex = startIndex + itemsPerPage;
  const paginatedFiles = filteredFiles.slice(startIndex, endIndex);

  // Reset to page 1 when search query or items per page changes
  useEffect(() => {
    setCurrentPage(1);
  }, [searchQuery, itemsPerPage]);

  // Handle page change with animation
  const handlePageChange = (newPage: number) => {
    if (newPage === currentPage) return;
    setPageTransition(true);
    setTimeout(() => {
      setCurrentPage(newPage);
      setPageTransition(false);
    }, 150);
  };

  const handleSaveProjectInfo = async (updatedInfo: Partial<File>) => {
    if (!selectedFileInfo) return;

    try {
      setIsLoading(true);

      // Mettre à jour uniquement les champs modifiables
      await store.updateFile(selectedFileInfo.id, {
        project_name: updatedInfo.project_name,
        vehicle_brand: updatedInfo.vehicle_brand,
        vehicle_model: updatedInfo.vehicle_model,
        engine_type: updatedInfo.engine_type,
        transmission_type: updatedInfo.transmission_type,
        year: updatedInfo.year,
        power: updatedInfo.power,
        customer: updatedInfo.customer,
        stage: updatedInfo.stage,
        notes: updatedInfo.notes,
      });

      toast({
        title: t.dashboard.infoSaved,
        description: t.dashboard.infoSavedDescription,
      });

      // Fermer le modal avec animation et recharger les données
      handleCloseProjectInfo();
      await loadDashboardData();
    } catch (error: any) {
      console.error("Error updating project info:", error);
      toast({
        title: t.dashboard.error,
        description: t.dashboard.unableToSaveInfo,
        variant: "destructive",
      });
    } finally {
      setIsLoading(false);
    }
  };

  if (isLoading) {
    return (
      <div className="min-h-screen flex items-center justify-center" style={{ backgroundColor: pageBg }}>
        <div className="flex flex-col items-center gap-4">
          <div className={`loader loader-lg${theme === "light" ? " loader-light" : ""}`} />
          <p className={theme === "light" ? "text-slate-500" : "text-slate-400"}>{t.dashboard.loading}</p>
        </div>
      </div>
    );
  }

  return (
    // overflow-x: clip (et non hidden) : hidden ferait de cette div le
    // conteneur de référence du sticky de l'en-tête, qui ne collerait
    // jamais — clip coupe le débordement sans créer de contexte de scroll
    <div className="min-h-screen relative" style={{ backgroundColor: pageBg, overflowX: 'clip' }}>
      <DashboardBackground wallpaper={wallpaper} theme={theme} isLight={isLight} customWallpaper={customWallpaper} />

      {/* Zone fixe au-dessus des tuiles : barre fenêtre + titre/recherche/
          upload. Reste en place au défilement ; les tuiles disparaissent en
          fondu derrière (verre + masque dégradé). */}
      <div className="sticky top-0 z-[60]">
        {/* Le voile n'apparaît qu'une fois la page défilée : au repos il
            masquerait le fond d'écran (halos), qui doit courir jusqu'en
            haut de la fenêtre. */}
        <div
          aria-hidden
          className="absolute inset-x-0 top-0 -bottom-8 pointer-events-none transition-opacity duration-200"
          style={{
            opacity: isScrolled ? 1 : 0,
            background: isLight
              ? 'linear-gradient(to bottom, rgba(244,246,250,0.92) 0%, rgba(244,246,250,0.82) 70%, rgba(244,246,250,0) 100%)'
              : 'linear-gradient(to bottom, rgba(9,11,17,0.92) 0%, rgba(9,11,17,0.82) 70%, rgba(9,11,17,0) 100%)',
            backdropFilter: isScrolled ? 'blur(10px)' : 'none',
            WebkitBackdropFilter: isScrolled ? 'blur(10px)' : 'none',
            maskImage: 'linear-gradient(to bottom, black 70%, transparent 100%)',
            WebkitMaskImage: 'linear-gradient(to bottom, black 70%, transparent 100%)',
          }}
        />

      {/* Header — frameless-window title bar, fully transparent so it reads
          as part of the dashboard (no distinct background or border) */}
      <header data-tauri-drag-region className="relative z-[60]" style={{ animation: 'slideInFromTop 0.6s ease-out' }}>
        <div data-tauri-drag-region className="pl-4 pr-2 pt-1 pb-1">
          <div data-tauri-drag-region className="flex items-start justify-between">
            <div data-tauri-drag-region className="flex items-center gap-3 pt-2 pl-2 min-w-0 overflow-hidden">
              {/* Même wordmark que l'éditeur : « Zed » en dégradé + BETA */}
              <div data-tauri-drag-region className="relative inline-block select-none">
                <h1 data-tauri-drag-region className="text-xl font-bold">
                  <span className="bg-gradient-to-r from-red-600 via-red-500 to-orange-500 bg-clip-text text-transparent">Zed</span><span className={isLight ? 'text-slate-900' : 'text-white'}>Suite</span>
                </h1>
              </div>
            </div>

            {/* Single row: app buttons (help, settings) then a hairline, then
                the window controls.
                flex-shrink-0 : réduire/agrandir/fermer restent toujours
                visibles quand la fenêtre rétrécit (c'est le wordmark à gauche
                qui se comprime). */}
            <div className="flex items-center pt-1 flex-shrink-0">
              <button
                onClick={() => setShowAbout(true)}
                className={`h-8 w-10 flex items-center justify-center rounded-md transition-colors ${isLight ? (onCustomLight ? 'text-slate-900 hover:text-black hover:bg-black/10' : 'text-slate-500 hover:text-black hover:bg-black/10') : darkIcon}`}
                title="Help"
              >
                <HelpCircle className="w-4 h-4" />
              </button>
              <button
                onClick={() => router.push('/settings')}
                className={`h-8 w-10 flex items-center justify-center rounded-md transition-colors ${isLight ? (onCustomLight ? 'text-slate-900 hover:text-black hover:bg-black/10' : 'text-slate-500 hover:text-black hover:bg-black/10') : darkIcon}`}
                title={t.userMenu.settings}
              >
                <Settings className="w-4 h-4" />
              </button>
              <div className="w-px h-4 bg-white/[0.12] mx-1.5" />
              <WindowControls strong={onCustomLight || onCustomDark} />
            </div>
          </div>
        </div>
      </header>

        {/* Rangée titre / recherche / upload — fixe avec la barre fenêtre */}
        <div className="relative container mx-auto px-4 pt-3 pb-4">
          <div className="flex flex-wrap items-center justify-between gap-4">
            <div className="flex items-baseline gap-3">
              <h3 className={`text-sm font-bold uppercase tracking-widest ${subText ?? (isLight ? 'text-slate-900' : 'text-slate-300')}`}>{t.dashboard.recentFiles}</h3>
              <span className={`text-xs tabular-nums ${subText ?? (isLight ? 'text-slate-700' : 'text-slate-500')}`}>{filteredFiles.length}</span>
            </div>

            {/* Search Bar */}
            <div className="relative w-full sm:w-80">
              <Search className={`absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 ${isLight ? 'text-slate-600' : 'text-slate-500'}`} />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder={t.dashboard.searchPlaceholder}
                // Thème clair : fond blanc quasi opaque + placeholder foncé, sinon
                // le texte se noyait sur un fond d'écran personnalisé sombre
                className={`w-full pl-10 pr-4 py-2 rounded-lg text-sm focus:outline-none focus:ring-0 transition-colors ${isLight ? 'bg-white/80 border border-black/15 text-slate-900 placeholder:text-slate-600 focus:border-black/30' : onCustomDark ? 'bg-[#0d1017]/45 border border-white/20 text-white placeholder:text-slate-300 focus:border-white/40' : 'bg-white/[0.04] border border-white/10 text-white placeholder:text-slate-500 focus:border-white/25'}`}
                spellCheck={false}
              />
            </div>

            <Button
              onClick={handleUploadClick}
              className="bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400 text-white shadow-lg shadow-red-500/25 group"
            >
              <Upload className="w-4 h-4 mr-2" />
              <span>{t.dashboard.upload}</span>
              <ArrowRight className="w-4 h-4 ml-2 group-hover:translate-x-1 transition-transform" />
            </Button>
          </div>
        </div>
      </div>

      {/* Main Content */}
      <main className="relative z-10 container mx-auto px-4 pb-8 pt-2">
        {/* Recent projects — vehicle-style card grid (Autotuner-inspired) */}
        <div style={{ animation: 'slideInFromBottom 0.5s ease-out 0.2s backwards' }}>
          {files.length === 0 ? (
            <div className="text-center py-16 rounded-2xl border border-dashed border-white/[0.08]">
              <div className="w-20 h-20 rounded-full bg-white/[0.04] border border-white/[0.08] flex items-center justify-center mx-auto mb-4">
                <FileText className="w-10 h-10 text-slate-600" />
              </div>
              <h4 className="text-lg font-semibold text-slate-300 mb-2">{t.dashboard.noFiles}</h4>
              <p className="text-slate-500">{t.dashboard.startUpload}</p>
            </div>
          ) : filteredFiles.length === 0 ? (
            <div className="text-center py-16 rounded-2xl border border-dashed border-white/[0.08]">
              <div className="w-20 h-20 rounded-full bg-white/[0.04] border border-white/[0.08] flex items-center justify-center mx-auto mb-4">
                <Search className="w-10 h-10 text-slate-600" />
              </div>
              <h4 className="text-lg font-semibold text-slate-300 mb-2">{t.dashboard.noResults}</h4>
              <p className="text-slate-500">{t.dashboard.noMatchingFiles} "{searchQuery}"</p>
            </div>
          ) : (
            <>
              <div
                className={`space-y-3 transition-all duration-150 ${pageTransition ? 'opacity-0 translate-y-2' : 'opacity-100 translate-y-0'}`}
              >
                {paginatedFiles.map((file) => {
                  const iconStyles = getStageIconStyles(file.stage, isLight);
                  return (
                    <div
                      key={file.id}
                      className={`group flex items-center justify-between p-4 rounded-xl transition-all cursor-pointer ${isLight ? 'bg-white/60 border border-black/[0.08] hover:bg-white/90 hover:border-black/[0.16] shadow-sm' : onCustomDark ? 'bg-[#0d1017]/45 border border-white/[0.12] hover:bg-[#0d1017]/60 hover:border-white/[0.2]' : 'bg-white/[0.03] border border-white/[0.07] hover:bg-white/[0.06] hover:border-white/[0.14]'}`}
                      onClick={() => handleOpenProject(file)}
                    >
                      <div className="flex items-center gap-4 flex-1">
                        <div className={`w-12 h-12 rounded-xl ${iconStyles.background} flex items-center justify-center border ${iconStyles.border}`}>
                          {iconStyles.stageNumber ? (
                            <span className="text-xl font-bold" style={{ fontStyle: 'italic' }}>
                              <span className={isLight ? "text-slate-900" : "text-white"}>ST</span>
                              <span className={iconStyles.numberColor}>{iconStyles.stageNumber}</span>
                            </span>
                          ) : (
                            <ZedFileIcon className="w-5 h-5" barColor={isLight ? "#334155" : "#ffffff"} />
                          )}
                        </div>
                        <div className="flex-1 min-w-0">
                          <h4 className={`font-semibold transition-colors truncate ${isLight ? 'text-slate-900' : 'text-white'}`} title={file.project_name || file.original_name}>
                            {file.project_name || file.original_name}
                          </h4>
                          <div className="flex items-center gap-3 mt-1">
                            <span className={`text-xs ${subText ?? 'text-slate-500'}`}>
                              {new Date(file.created).toLocaleDateString("fr-FR")}
                            </span>
                            {file.customer && (
                              <span className={`flex items-center gap-1 text-xs font-medium ${subText ?? (isLight ? "text-slate-700" : "text-slate-300")}`}>
                                <User className={`w-3 h-3 ${subText ?? "text-slate-500"}`} />
                                {file.customer}
                              </span>
                            )}
                            {file.ecu_type && (
                              <span className={`text-xs px-2 py-0.5 rounded-full text-white ${isLight ? "bg-gradient-to-r from-red-600 via-red-500 to-orange-500 border border-red-600/50" : "bg-gradient-to-r from-red-600/50 via-red-500/50 to-orange-500/50 border border-red-500/40"}`}>
                                {file.ecu_type}
                              </span>
                            )}
                            {file.vehicle_brand && (
                              <span className={`text-xs ${subText ?? 'text-slate-400'}`}>
                                {file.vehicle_brand}
                              </span>
                            )}
                            {file.year && (
                              <span className={`text-xs ${subText ?? 'text-slate-400'}`}>
                                {file.year}
                              </span>
                            )}
                            {file.hardware_version && (
                              <span className={`text-xs ml-4 ${subText ?? 'text-slate-400'}`}>
                                HW: {file.hardware_version}
                              </span>
                            )}
                            {file.software_version && (
                              <span className={`text-xs ${subText ?? 'text-slate-400'}`}>
                                SW: {file.software_version}
                              </span>
                            )}
                          </div>
                        </div>
                      </div>

                      <div className="flex items-center gap-2">
                        <div className={`px-2.5 py-0.5 rounded-full border ${isLight ? 'border-black/[0.10] bg-black/[0.05]' : 'border-white/[0.08] bg-white/[0.05]'}`}>
                          <span className={`text-xs tabular-nums ${subText ?? (isLight ? 'text-slate-500' : 'text-slate-400')}`}>
                            {t.dashboard.versions} : <span className={subText ?? (isLight ? 'text-slate-900' : 'text-slate-200')}>{versionCounts[file.id] || 1}</span>
                          </span>
                        </div>
                        {/* Actions groupées dans une pastille — même langage que
                            la pastille Versions, atténuée au repos */}
                        <div className={`flex items-center rounded-full border overflow-hidden opacity-70 group-hover:opacity-100 transition-opacity ${isLight ? 'border-black/[0.12] bg-black/[0.04]' : 'border-white/[0.08] bg-white/[0.04]'}`}>
                          <button
                            className={`h-7 w-9 flex items-center justify-center transition-colors ${isLight ? (onCustomLight ? 'text-slate-900 hover:text-black hover:bg-black/10' : 'text-slate-500 hover:text-black hover:bg-black/10') : darkIcon}`}
                            title={t.projectInfo?.title || "Project info"}
                            onClick={(e) => {
                              e.stopPropagation();
                              setSelectedFileInfo(file);
                            }}
                          >
                            <MoreVertical className="w-4 h-4" />
                          </button>
                          <div className={`w-px h-4 ${isLight ? 'bg-black/[0.12]' : 'bg-white/[0.08]'}`} />
                          <button
                            className={`h-7 w-9 flex items-center justify-center transition-colors ${isLight ? 'text-blue-600 hover:text-blue-700 hover:bg-blue-500/10' : 'text-blue-400 hover:text-blue-300 hover:bg-blue-500/10'}`}
                            title={t.dashboard.powerTitle}
                            onClick={(e) => {
                              e.stopPropagation();
                              setPowerFile(file);
                            }}
                          >
                            <Gauge className="w-4 h-4" />
                          </button>
                          <div className={`w-px h-4 ${isLight ? 'bg-black/[0.12]' : 'bg-white/[0.08]'}`} />
                          <button
                            className={`h-7 w-9 flex items-center justify-center transition-colors ${isLight ? (onCustomLight ? 'text-slate-900 hover:text-black hover:bg-black/10' : 'text-slate-500 hover:text-black hover:bg-black/10') : darkIcon}`}
                            title={t.dashboard.openFolder}
                            onClick={(e) => {
                              e.stopPropagation();
                              store.openProjectDir(file.id).catch(() => {});
                            }}
                          >
                            <FolderOpen className="w-4 h-4" />
                          </button>
                          <div className={`w-px h-4 ${isLight ? 'bg-black/[0.12]' : 'bg-white/[0.08]'}`} />
                          <button
                            className="h-7 w-9 flex items-center justify-center text-red-500/90 hover:text-white hover:bg-red-500/70 transition-colors"
                            title={t.dashboard.delete}
                            onClick={(e) => handleDeleteProject(file, e)}
                          >
                            <Trash2 className="w-4 h-4" />
                          </button>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>

              {/* Pagination */}
              {filteredFiles.length > 0 && (
                <div className="flex items-center justify-between mt-6">
                  <div className="flex-1" />

                  {totalPages > 1 && (
                    <div className="flex items-center justify-center gap-2">
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => handlePageChange(Math.max(1, currentPage - 1))}
                        disabled={currentPage === 1}
                        className={isLight
                          ? `${paginationText} hover:text-black hover:bg-black/10 disabled:opacity-50 disabled:cursor-not-allowed`
                          : `${onCustomDark ? 'text-white' : 'text-slate-400'} hover:text-white hover:bg-white/10 disabled:opacity-50 disabled:cursor-not-allowed`}
                      >
                        {t.dashboard.previous}
                      </Button>

                      <div className="flex items-center gap-1">
                        {Array.from({ length: totalPages }, (_, i) => i + 1).map((page) => (
                          <Button
                            key={page}
                            size="sm"
                            variant="ghost"
                            onClick={() => handlePageChange(page)}
                            className={`w-8 h-8 p-0 ${
                              currentPage === page
                                ? 'bg-gradient-to-r from-red-600 via-red-500 to-orange-500 text-white'
                                : (isLight ? `${paginationText} hover:text-black hover:bg-black/10` : `${onCustomDark ? 'text-white' : 'text-slate-400'} hover:text-white hover:bg-white/5`)
                            }`}
                          >
                            {page}
                          </Button>
                        ))}
                      </div>

                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => handlePageChange(Math.min(totalPages, currentPage + 1))}
                        disabled={currentPage === totalPages}
                        className={isLight
                          ? `${paginationText} hover:text-black hover:bg-black/10 disabled:opacity-50 disabled:cursor-not-allowed`
                          : `${onCustomDark ? 'text-white' : 'text-slate-400'} hover:text-white hover:bg-white/10 disabled:opacity-50 disabled:cursor-not-allowed`}
                      >
                        {t.dashboard.next}
                      </Button>
                    </div>
                  )}

                  {/* Items per page selector */}
                  <div className="flex items-center gap-2 flex-1 justify-end">
                    <span className={`text-sm ${isLight ? paginationText : (onCustomDark ? 'text-white' : 'text-slate-400')}`}>{t.dashboard.show}:</span>
                    <StyledSelect
                      appearance="auto"
                      value={String(itemsPerPage)}
                      onChange={(v) => setItemsPerPage(Number(v))}
                      minWidth={70}
                      options={[
                        { value: "7", label: "7" },
                        { value: "10", label: "10" },
                        { value: "20", label: "20" },
                        { value: "50", label: "50" },
                        { value: "100", label: "100" },
                      ]}
                    />
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </main>

      {/* Upload Modal */}
      {showUploadModal && (
        <div
          className="fixed inset-0 z-[70] flex items-center justify-center backdrop-blur-sm"
          style={{
            backgroundColor: '#000000a2',
            animation: isClosingUploadModal ? 'backdropFadeOut 0.2s ease-out forwards' : 'backdropFadeIn 0.2s ease-out forwards'
          }}
          // Ne se ferme JAMAIS au clic sur le fond — uniquement via les boutons
          onClick={(e) => e.stopPropagation()}
        >
          <div
            className="relative w-full max-w-4xl max-h-[95vh] overflow-y-auto upload-scroll"
            style={{
              animation: isClosingUploadModal ? 'modalCollapse 0.2s ease-out forwards' : 'modalExpand 0.2s ease-out forwards'
            }}
          >
            {/* Close button */}
            <button
              onClick={handleCloseUploadModal}
              className={`absolute top-4 right-4 z-10 p-2 rounded-lg transition-colors ${isLight ? 'hover:bg-black/5' : 'hover:bg-white/5'}`}
              style={{ color: isLight ? 'rgba(0, 0, 0, 0.6)' : 'rgba(255, 255, 255, 0.6)' }}
            >
              <X className="w-5 h-5" />
            </button>

            {/* ProjectCreator component */}
            <ProjectCreator
              onProjectCreated={(projectName) => {
                handleCloseUploadModal();
                loadDashboardData(); // Recharger la liste des fichiers
              }}
            />
          </div>
        </div>
      )}

      {/* Project Info Modal */}
      {selectedFileInfo && (
        <ProjectInfoEditModal
          file={selectedFileInfo}
          onClose={handleCloseProjectInfo}
          onSave={handleSaveProjectInfo}
          isClosing={isClosingProjectInfo}
          t={t}
        />
      )}

      {/* Delete Confirmation Modal */}
      {showDeleteConfirmModal && fileToDelete && (
        <div
          className="fixed inset-0 flex items-center justify-center backdrop-blur-sm z-[100]"
          style={{
            backgroundColor: '#000000a2',
            animation: isClosingDeleteConfirmModal ? 'fadeOut 0.2s ease-out forwards' : 'fadeIn 0.2s ease-out forwards'
          }}
          onClick={handleCloseDeleteConfirmModal}
        >
          <div
            className="rounded-lg shadow-2xl p-6 max-w-md w-full mx-4 border"
            style={{
              ...MODAL_GLASS,
              animation: isClosingDeleteConfirmModal ? 'scaleOut 0.2s ease-out forwards' : 'scaleIn 0.2s ease-out forwards'
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center gap-3 mb-4">
              <div className="p-2 rounded-full bg-red-500/20">
                <AlertTriangle className="w-6 h-6 text-red-500" />
              </div>
              <h3 className="text-lg font-semibold text-white">
                {t.dashboard.deleteConfirmTitle}
              </h3>
            </div>
            <p className="mb-2 text-white/90">
              {t.dashboard.confirmDelete} &quot;{(() => {
                const name = fileToDelete.project_name || fileToDelete.original_name || "";
                return name.length > 45 ? name.slice(0, 45) + "…" : name;
              })()}&quot; ?
            </p>
            <p className="mb-6 text-white/60 text-sm">
              {t.dashboard.deleteWarning}
            </p>
            <div className="flex gap-3 justify-end">
              <Button
                variant="ghost"
                size="sm"
                className="h-9 px-4 text-white/70 hover:text-white hover:bg-white/10"
                onClick={handleCloseDeleteConfirmModal}
              >
                {t.dashboard.cancel}
              </Button>
              <Button
                size="sm"
                className="h-9 px-6 text-white bg-gradient-to-r from-red-700 via-red-600 to-red-500 hover:from-red-600 hover:via-red-500 hover:to-red-400 transition-all duration-300"
                onClick={handleConfirmDelete}
              >
                {t.dashboard.delete}
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* Power Estimate Modal */}
      {powerFile && (
        <PowerEstimateModal file={powerFile} onClose={() => setPowerFile(null)} />
      )}

      {/* Help / About Modal */}
      {showAbout && (
        <div
          className="fixed inset-0 flex items-center justify-center backdrop-blur-sm z-[100]"
          style={{ backgroundColor: '#000000a2', animation: 'fadeIn 0.2s ease-out forwards' }}
          onClick={() => setShowAbout(false)}
        >
          <div
            className="rounded-lg shadow-2xl p-6 max-w-5xl w-full mx-4 border max-h-[85vh] overflow-y-auto"
            style={{ ...(theme === "light" ? MODAL_GLASS_LIGHT : MODAL_GLASS), animation: 'scaleIn 0.2s ease-out forwards' }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center gap-3 mb-4">
              <img src="/zedsuite-icon.svg" alt="ZedSuite" className="w-10 h-10 object-contain" />
              <div>
                <h3 className={`text-lg font-semibold leading-tight ${theme === "light" ? "text-slate-900" : "text-white"}`}>ZedSuite</h3>
                <p className={`text-xs ${theme === "light" ? "text-slate-500" : "text-slate-400"}`}>v1.0.0 — Open source, GPL-3.0</p>
              </div>
            </div>
            <p className={`text-sm mb-4 ${theme === "light" ? "text-slate-700" : "text-white/80"}`}>{t.appInfo.intro}</p>
            <h4 className={`text-sm font-semibold mb-2 ${theme === "light" ? "text-slate-900" : "text-white"}`}>{t.appInfo.whatYouCanDo}</h4>
            {/* La grille se remplit ligne par ligne : le dyno (feature4) est
                rendu en dernier pour occuper le bas du groupe */}
            <div className={`text-sm grid grid-cols-1 sm:grid-cols-2 gap-x-6 gap-y-1.5 mb-4 ${theme === "light" ? "text-slate-600" : "text-white/60"}`}>
              <p>• {t.appInfo.feature1}</p>
              <p>• {t.appInfo.feature2}</p>
              <p>• {t.appInfo.feature3}</p>
              <p>• {t.appInfo.feature5}</p>
              <p>• {t.appInfo.feature6}</p>
              <p>• {t.appInfo.feature7}</p>
              <p>• {t.appInfo.feature8}</p>
              <p>• {t.appInfo.feature4}</p>
            </div>
            <div className={`rounded-md border p-3 mb-3 ${theme === "light" ? "border-black/[0.08] bg-black/[0.04]" : "border-white/[0.08] bg-white/[0.03]"}`}>
              <p className={`text-sm font-semibold mb-1 ${theme === "light" ? "text-slate-900" : "text-white"}`}>{t.appInfo.updatesTitle}</p>
              <p className={`text-sm ${theme === "light" ? "text-slate-600" : "text-white/60"}`}>{t.appInfo.updatesText}</p>
            </div>
            <div className={`text-sm mb-5 ${theme === "light" ? "text-slate-600" : "text-white/60"}`}>
              <p className={`font-semibold mb-1 ${theme === "light" ? "text-slate-900" : "text-white"}`}>{t.appInfo.feedbackTitle}</p>
              <p>{t.appInfo.feedbackText}</p>
              <p className="mt-1.5">Source code: <span className={theme === "light" ? "text-slate-800" : "text-white/80"}>github.com/LeZed97/ZedSuite</span></p>
            </div>
            <div className="relative flex justify-end items-center">
              {/* Logo ZedPerf centré en bas — noir/rouge en clair, blanc/rouge en sombre */}
              <img
                src={theme === "light" ? "/zedperf-light.png" : "/zedperf-dark.png"}
                alt="ZedPerf"
                className="absolute left-1/2 -translate-x-1/2 h-6 w-auto object-contain pointer-events-none select-none"
              />
              <Button
                size="sm"
                className="h-9 px-6 text-white bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400"
                onClick={() => setShowAbout(false)}
              >
                {t.common?.close || "Close"}
              </Button>
            </div>
          </div>
        </div>
      )}

      <style jsx>{`
        @keyframes fadeIn {
          from { opacity: 0; }
          to { opacity: 1; }
        }
        @keyframes fadeOut {
          from { opacity: 1; }
          to { opacity: 0; }
        }
        @keyframes scaleIn {
          from { transform: scale(0.95); opacity: 0; }
          to { transform: scale(1); opacity: 1; }
        }
        @keyframes scaleOut {
          from { transform: scale(1); opacity: 1; }
          to { transform: scale(0.95); opacity: 0; }
        }
      `}</style>
    </div>
  );
}
