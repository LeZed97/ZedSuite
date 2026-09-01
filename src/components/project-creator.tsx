"use client";

import { useState, useRef } from "react";
import { Button } from "@/components/ui/button";
import { Upload, X, FileText, Check, Cpu, AlertCircle } from "lucide-react";
import { useToast } from "@/hooks/use-toast";
import { useI18n } from "@/contexts/i18n-context";
import { useSettings } from "@/contexts/settings-context";
import { useRouter } from "next/navigation";
import axios from "axios";
import { MODAL_GLASS, MODAL_GLASS_LIGHT } from "@/lib/modal-glass";
import { useThemeOptional } from "@/contexts/theme-context";
import { identifyEcu, detectMaps } from "@/lib/local/detector";
import ZedGradientDefs, { ZedFileIcon } from "@/components/zed-gradient-defs";
import { lookupEcuBrand } from "@/lib/ecu-brand-db";

interface ProjectCreatorProps {
  onProjectCreated?: (projectId: string) => void;
}

interface ECUIdentification {
  manufacturer: string;
  ecu_type: string;
  variant?: string;
  software_version?: string;
  hardware_version?: string;
  part_number?: string;
  confidence: number;
}

// Helper function to get stage icon styles based on stage value
function getStageIconStyles(stage?: string, isLight?: boolean) {
  switch (stage) {
    case 'Stage 1':
      return { background: 'bg-green-500/20', border: 'border-green-500/30', numberColor: 'text-green-500', stageNumber: '1' };
    case 'Stage 2':
      return { background: 'bg-yellow-500/20', border: 'border-yellow-500/30', numberColor: 'text-yellow-500', stageNumber: '2' };
    case 'Stage 3':
      return { background: 'bg-red-500/20', border: 'border-red-500/30', numberColor: 'text-red-500', stageNumber: '3' };
    default:
      return { background: isLight ? 'bg-black/[0.04]' : 'bg-slate-500/20', border: isLight ? 'border-black/10' : 'border-slate-500/30', numberColor: null, stageNumber: null };
  }
}

export function ProjectCreator({ onProjectCreated }: ProjectCreatorProps) {
  const router = useRouter();
  const { toast } = useToast();
  const { t } = useI18n();
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [isUploading, setIsUploading] = useState(false);
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [ecuIdentification, setEcuIdentification] = useState<ECUIdentification | null>(null);
  const [versioningInfo, setVersioningInfo] = useState<{
    fileId?: string;
    currentVersionId?: string;
    versions?: any[];
  }>({});
  
  // Cached base64 file data (avoid re-encoding for detect call)
  const fileBase64Ref = useRef<string | null>(null);

  // Form state
  const [projectName, setProjectName] = useState("");
  const [vehicleBrand, setVehicleBrand] = useState("");
  const [vehicleModel, setVehicleModel] = useState("");
  const [engineType, setEngineType] = useState("");
  const [transmissionType, setTransmissionType] = useState("");
  const [year, setYear] = useState("");
  const [power, setPower] = useState("");
  const [customer, setCustomer] = useState("");
  const [stage, setStage] = useState("");
  const [date, setDate] = useState(new Date().toISOString().split('T')[0]);
  const [notes, setNotes] = useState("");

  const { platform } = useSettings();
  // Suit le thème de l'écran hôte
  const themeCtx = useThemeOptional();
  const L = (themeCtx?.theme ?? "default") === "light";
  const labelCls = `block text-sm font-medium mb-2 ${L ? 'text-slate-900' : 'text-white'}`;
  const inputCls = `w-full px-3 py-2 rounded-lg focus:outline-none focus:ring-0 ${L ? 'bg-black/[0.05] border border-black/20 text-slate-900 placeholder:text-black/40' : 'bg-black/15 border border-white/20 text-white placeholder:text-white/50'}`;
  const inputSmCls = `w-full px-2 py-2 rounded-lg text-sm focus:outline-none focus:ring-0 ${L ? 'bg-black/[0.05] border border-black/20 text-slate-900 placeholder:text-black/40' : 'bg-black/15 border border-white/20 text-white placeholder:text-white/50'}`;
  const selectCls = `${inputSmCls} ${L ? '[&>option]:bg-white [&>option]:text-slate-900' : '[&>option]:bg-black/90 [&>option]:text-white'}`;

  const maxFileSize = platform.maxFileSizeMB * 1024 * 1024;

  const handleFileSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      if (file.size > maxFileSize) {
        toast({
          title: t.errors.fileTooLarge,
          description: t.errors.fileTooLargeDescription,
          variant: "destructive",
        });
        return;
      }
      setSelectedFile(file);
      setEcuIdentification(null);
      
      // Auto-fill project name from filename
      if (!projectName) {
        setProjectName(file.name.replace(/\.[^/.]+$/, ""));
      }

      // Automatically analyze the file to detect ECU type
      await analyzeECUType(file);
    }
  };

  const analyzeECUType = async (file: File) => {
    setIsAnalyzing(true);
    try {
      // Read file and encode as base64 using chunked approach (fast for large files)
      const arrayBuffer = await file.arrayBuffer();
      const uint8Array = new Uint8Array(arrayBuffer);
      const chunks: string[] = [];
      const chunkSize = 8192;
      for (let i = 0; i < uint8Array.length; i += chunkSize) {
        chunks.push(String.fromCharCode(...uint8Array.subarray(i, i + chunkSize)));
      }
      const fileDataBase64 = btoa(chunks.join(''));
      fileBase64Ref.current = fileDataBase64; // Cache for later use

      // Identify the ECU type via the embedded Rust detection engine
      const identResult = await identifyEcu(fileDataBase64, file.name);

      // Check if this ECU type is enabled in the admin database
      if (identResult?.ecu_type && identResult.ecu_type !== "Unknown") {
        try {
          const statusRes = await fetch(`/api/ecu-status?ecu_type=${encodeURIComponent(identResult.ecu_type)}`);
          if (statusRes.ok) {
            const { enabled } = await statusRes.json();
            if (!enabled) {
              toast({
                title: t.errors.ecuIdentificationFailed,
                description: `ECU type "${identResult.ecu_type}" is currently disabled.`,
                variant: "destructive",
              });
              setEcuIdentification({
                manufacturer: "Unknown",
                ecu_type: "Unknown",
                confidence: 0,
              });
              return;
            }
          }
        } catch {}
      }

      setEcuIdentification(identResult);

      // Auto-fill the vehicle brand from the embedded ECU reference database
      // (Bosch 0281xxx numbers, VAG 038906019xx / 03G906016xx refs). Never
      // overwrites a brand the user already picked.
      const brandInfo = lookupEcuBrand(
        identResult?.hardware_version,
        identResult?.software_version,
        identResult?.part_number
      );
      if (brandInfo) {
        setVehicleBrand((prev) => prev || brandInfo.b);
      }
    } catch (error: any) {
      toast({
        title: t.errors.ecuIdentificationFailed,
        description: t.errors.ecuIdentificationFailedDescription,
        variant: "destructive",
      });

      // Set unknown ECU type
      setEcuIdentification({
        manufacturer: "Unknown",
        ecu_type: "Unknown",
        confidence: 0,
      });
    } finally {
      setIsAnalyzing(false);
    }
  };

  const handleRemoveFile = () => {
    setSelectedFile(null);
    setEcuIdentification(null);
  };

  const handleCreateProject = async () => {
    if (!selectedFile) {
      toast({
        title: t.errors.noFileSelected,
        description: t.errors.noFileSelectedDescription,
      });
      return;
    }

    // Block if ECU is disabled
    if (ecuIdentification?.ecu_type && ecuIdentification.ecu_type !== "Unknown") {
      try {
        const statusRes = await fetch(`/api/ecu-status?ecu_type=${encodeURIComponent(ecuIdentification.ecu_type)}`);
        if (statusRes.ok) {
          const { enabled } = await statusRes.json();
          if (!enabled) {
            toast({
              title: "ECU Disabled",
              description: `ECU type "${ecuIdentification.ecu_type}" is currently disabled.`,
              variant: "destructive",
            });
            return;
          }
        }
      } catch {}
    }

    if (!projectName.trim()) {
      toast({
        title: t.errors.projectNameRequired,
        description: t.errors.projectNameRequiredDescription,
      });
      return;
    }

    setIsUploading(true);

    try {
      // Use cached base64 from identification step, or encode now
      let fileDataBase64 = fileBase64Ref.current;
      if (!fileDataBase64) {
        const arrayBuffer = await selectedFile.arrayBuffer();
        const uint8Array = new Uint8Array(arrayBuffer);
        const chunks: string[] = [];
        const chunkSize = 8192;
        for (let i = 0; i < uint8Array.length; i += chunkSize) {
          chunks.push(String.fromCharCode(...uint8Array.subarray(i, i + chunkSize)));
        }
        fileDataBase64 = btoa(chunks.join(''));
      }

      // Run map detection via the embedded Rust detection engine
      const detectionResults = await detectMaps({
        fileDataBase64,
        fileName: selectedFile.name,
        ecuType: ecuIdentification?.ecu_type || "unknown",
      });
      const response = { data: detectionResults };
      // Clear any stale project data BEFORE attempting versioning
      // This prevents navigating to editor with stale data if versioning fails
      if (typeof window !== 'undefined') {
        sessionStorage.removeItem("currentProject");
        localStorage.removeItem("currentProject");
      }

      // Initialisation du versioning (création de l'Ori)
      let versioningData: { fileId?: string; currentVersionId?: string; versions?: any[] } = {};
      try {
        const versioning = await axios.post("/api/versioning/init", {
          projectName,
          fileName: selectedFile.name,
          fileData: fileDataBase64, // Base64 encoded binary data for PocketBase storage
          fileSize: selectedFile.size,
          ecuType: ecuIdentification?.ecu_type || "unknown",
          hardwareVersion: ecuIdentification?.hardware_version,
          softwareVersion: ecuIdentification?.software_version,
          detectionResults: response.data,
          vehicleBrand,
          vehicleModel,
          engineType,
          transmissionType,
          year,
          power,
          customer,
          stage,
          date,
          notes,
        }, { timeout: 120000 }); // 2 minutes timeout for large files
        versioningData = {
          fileId: versioning.data.fileId,
          currentVersionId: versioning.data.currentVersionId,
          versions: versioning.data.versions || [],
        };
        setVersioningInfo(versioningData);
      } catch (err: any) {
        // Versioning failed - cannot proceed without stored file data.
        // Surface the underlying error: in a local app the real cause
        // (fs permission, disk full...) is actionable for the user.
        const detail = err?.response?.data?.error || err?.message || "";
        toast({
          title: t.errors.creationError,
          description: `${t.errors.creationErrorDescription}${detail ? ` — ${detail}` : ""}`,
          variant: "destructive",
        });
        setIsUploading(false);
        return; // Stop the process, don't navigate to editor without file data
      }

      // Navigate to editor page with project data
      // CRITICAL: Clear all viewMode storage when creating a new project
      // This ensures maps always open in "text" view by default
      if (typeof window !== 'undefined') {
        // Clear all viewMode entries from sessionStorage
        const keysToRemove: string[] = [];
        for (let i = 0; i < sessionStorage.length; i++) {
          const key = sessionStorage.key(i);
          if (key && key.startsWith('viewMode_')) {
            keysToRemove.push(key);
          }
        }
        keysToRemove.forEach(key => sessionStorage.removeItem(key));
      }
      
      // Store project metadata in sessionStorage (WITHOUT file_data to avoid quota exceeded)
      // The actual binary data is stored in PocketBase via versioning
      const projectMetadata = {
        file_name: selectedFile.name,
        original_name: selectedFile.name,
        file_size: selectedFile.size,
        ecu_type: ecuIdentification?.ecu_type || "unknown",
        hardware_version: ecuIdentification?.hardware_version,
        software_version: ecuIdentification?.software_version,
        project_name: projectName,
        vehicle_brand: vehicleBrand,
        vehicle_model: vehicleModel,
        engine_type: engineType,
        transmission_type: transmissionType,
        year: year,
        power: power,
        customer: customer,
        stage: stage,
        date: date,
        notes: notes,
        created: new Date().toISOString(),
        detectionResults: response.data,
        ecuIdentification: ecuIdentification,
        fileId: versioningData.fileId,
        currentVersionId: versioningData.currentVersionId,
        versions: versioningData.versions || [],
        // Note: file_data is NOT stored here - it's in PocketBase
      };
      sessionStorage.setItem("currentProject", JSON.stringify(projectMetadata));

      // Navigate to editor - don't call onProjectCreated (it closes the modal
      // and shows the dashboard during the navigation transition)
      router.push(`/editor?project=${encodeURIComponent(projectName)}`);
    } catch (error: any) {
      const detail = error?.message || String(error ?? "");
      toast({
        title: t.errors.uploadFailed,
        description: `${t.errors.uploadFailedDescription}${detail ? ` — ${detail}` : ""}`,
        variant: "destructive",
      });
    } finally {
      setIsUploading(false);
    }
  };

  return (
    <div className="max-w-4xl mx-auto">
      <ZedGradientDefs />
      <div className="border rounded-lg p-8" style={L ? MODAL_GLASS_LIGHT : MODAL_GLASS}>
        <h2 className={`text-2xl font-bold mb-6 ${L ? "text-slate-900" : "text-white"}`}>{t.upload?.title || "Create New Project"}</h2>

        <div className="space-y-6">
          {/* File Upload Section */}
          <div>
            <label className={labelCls}>{t.upload?.ecuFile || "ECU File"} *</label>
            {!selectedFile ? (
              <label
                className="flex flex-col items-center justify-center w-full h-32 rounded-lg cursor-pointer upload-zone-hover transition-colors"
                style={{
                  // Pointillés en dégradé du logo (rect SVG — les bordures CSS
                  // n'acceptent pas de dégradé) + remplissage translucide du
                  // même dégradé, sur verre flouté
                  // Pointillés : noirs sur le thème clair, dégradé du logo en sombre
                  backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'%3E%3Cdefs%3E%3ClinearGradient id='g' x1='0%25' y1='0%25' x2='100%25' y2='0%25'%3E%3Cstop offset='0%25' stop-color='%23dc2626'/%3E%3Cstop offset='50%25' stop-color='%23ef4444'/%3E%3Cstop offset='100%25' stop-color='%23f97316'/%3E%3C/linearGradient%3E%3C/defs%3E%3Crect width='100%25' height='100%25' fill='none' rx='8' ry='8' stroke='${L ? "%23000000" : "%23ffffff"}' stroke-width='4' stroke-dasharray='8 8'/%3E%3C/svg%3E"), linear-gradient(90deg, rgba(220, 38, 38, 0.12), rgba(239, 68, 68, 0.12), rgba(249, 115, 22, 0.12))`,
                  backdropFilter: 'blur(8px) saturate(130%)',
                  WebkitBackdropFilter: 'blur(8px) saturate(130%)',
                }}
              >
                <div className="flex flex-col items-center justify-center pt-5 pb-6">
                  <Upload className="w-10 h-10 mb-3 text-muted-foreground" />
                  <p className="text-sm text-muted-foreground">
                    <span className="font-semibold">{t.upload?.clickToUpload || "Click to upload"}</span>
                  </p>
                </div>
                {/* Pas d'attribut accept : la boîte de dialogue Windows s'ouvre
                    sur "Tous les fichiers" au lieu d'un filtre personnalisé */}
                <input
                  type="file"
                  className="hidden"
                  onChange={handleFileSelect}
                />
              </label>
            ) : (
              <div className="space-y-3">
                <div className="flex items-center justify-between p-2 border rounded-lg" style={{ backgroundColor: L ? 'rgba(0, 0, 0, 0.05)' : 'hsla(0, 0%, 55.7%, 0.12)' }}>
                  <div className="flex items-center gap-3 flex-1 min-w-0">
                    {(() => {
                      const iconStyles = getStageIconStyles(stage, L);
                      return (
                        <div className={`w-10 h-10 shrink-0 rounded-lg ${iconStyles.background} flex items-center justify-center border ${iconStyles.border}`}>
                          {iconStyles.stageNumber ? (
                            <span className="text-base font-bold" style={{ fontStyle: 'italic' }}>
                              <span className={L ? "text-slate-900" : "text-white"}>ST</span>
                              <span className={iconStyles.numberColor}>{iconStyles.stageNumber}</span>
                            </span>
                          ) : (
                            <ZedFileIcon className="w-5 h-5" barColor={L ? "#334155" : "#ffffff"} />
                          )}
                        </div>
                      );
                    })()}
                    <div className="min-w-0">
                      <p className={`font-medium truncate ${L ? "text-slate-900" : "text-white"}`} title={selectedFile.name}>{selectedFile.name}</p>
                      <p className="text-sm text-muted-foreground">
                        {(selectedFile.size / 1024).toFixed(2)} KB
                      </p>
                    </div>
                  </div>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={handleRemoveFile}
                    disabled={isUploading || isAnalyzing}
                    className="shrink-0 ml-2 hover:bg-slate-500/20 text-slate-400 hover:text-white"
                  >
                    <X className="w-4 h-4" />
                  </Button>
                </div>

                {/* ECU Identification Display */}
                {isAnalyzing ? (
                  <div className="flex items-center gap-3 p-4 border rounded-lg bg-red-500/10 border-red-500/30">
                    <div className={`loader loader-sm${L ? " loader-light" : ""}`} />
                    <p className="font-medium text-white">{t.upload?.analyzingEcu || "Analyzing ECU file..."}</p>
                  </div>
                ) : ecuIdentification ? (
                  ecuIdentification.ecu_type === "Unknown" ? (
                    <div className="p-4 border rounded-lg bg-red-500/10 border-red-500/30">
                      <div className="flex items-center gap-3">
                        <AlertCircle className="w-6 h-6 flex-shrink-0 text-red-400" />
                        <p className="font-semibold leading-none text-white text-center flex-1">
                          {t.upload?.ecuNotDetected || "ECU not detected. Check that the file is the correct size and is not encrypted."}
                        </p>
                      </div>
                    </div>
                  ) : (
                    <div className={`p-4 border rounded-lg ${L ? "bg-green-600/10 border-green-600/30" : "bg-green-500/10 border-white-500/30"}`}>
                      <div className="flex items-center gap-3">
                        <Cpu className={`w-6 h-6 flex-shrink-0 ${L ? "text-green-700" : "text-white"}`} />
                        <div className="flex items-center justify-between flex-1 gap-6">
                          <p className={`font-semibold leading-none ${L ? "text-slate-900" : "text-white"}`}>
                            {ecuIdentification.manufacturer} {ecuIdentification.ecu_type}
                          </p>
                          <div className="flex items-center gap-6 flex-1 justify-center">
                            {ecuIdentification.hardware_version && (
                              <div className={`flex items-center gap-2 text-sm leading-none ${L ? "text-slate-500" : "text-slate-400"}`}>
                                <span className="font-medium">HW:</span>
                                <span className="font-medium">{ecuIdentification.hardware_version}</span>
                              </div>
                            )}
                            {ecuIdentification.software_version && (
                              <div className={`flex items-center gap-2 text-sm leading-none ${L ? "text-slate-500" : "text-slate-400"}`}>
                                <span className="font-medium">SW:</span>
                                <span className="font-medium">{ecuIdentification.software_version}</span>
                              </div>
                            )}
                          </div>
                        </div>
                      </div>
                    </div>
                  )
                ) : null}
              </div>
            )}
          </div>

          {/* Project Information */}
          <div>
            <label className={labelCls}>{t.upload?.projectName || "Project Name"} *</label>
            <input
              type="text"
              value={projectName}
              onChange={(e) => setProjectName(e.target.value)}
              className={inputCls}
              placeholder=""
              disabled={isUploading}
              spellCheck={false}
            />
          </div>

          {/* Vehicle Information */}
          <div className="border-t pt-6">
            <h3 className={`text-lg font-semibold mb-4 ${L ? "text-slate-900" : "text-white"}`}>{t.upload?.vehicleInfo || "Vehicle Information (Optional)"}</h3>
            <div className="grid md:grid-cols-3 gap-3">
              {/* Première ligne: Brand - Model - Year */}
              <div>
                <label className={labelCls}>{t.upload?.brand || "Brand"}</label>
                <select
                  value={vehicleBrand}
                  onChange={(e) => setVehicleBrand(e.target.value)}
                  className={selectCls}
                  disabled={isUploading}
                >
                  <option value="">{t.upload?.select || "Select..."}</option>
                  <option value="Audi">Audi</option>
                  <option value="Seat">Seat</option>
                  <option value="Skoda">Skoda</option>
                  <option value="Volkswagen">Volkswagen</option>
                </select>
              </div>

              <div>
                <label className={labelCls}>{t.upload?.model || "Model"}</label>
                <input
                  type="text"
                  value={vehicleModel}
                  onChange={(e) => setVehicleModel(e.target.value)}
                  className={inputSmCls}
                  placeholder=""
                  disabled={isUploading}
                  spellCheck={false}
                />
              </div>

              <div>
                <label className={labelCls}>{t.upload?.year || "Year"}</label>
                <select
                  value={year}
                  onChange={(e) => setYear(e.target.value)}
                  className={selectCls}
                  disabled={isUploading}
                >
                  <option value="">{t.upload?.select || "Select..."}</option>
                  {Array.from({ length: new Date().getFullYear() - 1996 }, (_, i) => new Date().getFullYear() - i).map(y => (
                    <option key={y} value={y}>{y}</option>
                  ))}
                </select>
              </div>

              {/* Deuxième ligne: Engine Type - Power (HP) - Transmission */}
              <div>
                <label className={labelCls}>{t.upload?.engineType || "Engine Type"}</label>
                <input
                  type="text"
                  value={engineType}
                  onChange={(e) => setEngineType(e.target.value)}
                  className={inputSmCls}
                  placeholder=""
                  disabled={isUploading}
                  spellCheck={false}
                />
              </div>

              <div>
                <label className={labelCls}>{t.upload?.powerHp || "Power (HP)"}</label>
                <input
                  type="text"
                  value={power}
                  onChange={(e) => setPower(e.target.value)}
                  className={inputSmCls}
                  placeholder=""
                  disabled={isUploading}
                  spellCheck={false}
                />
              </div>

              <div>
                <label className={labelCls}>{t.upload?.transmission || "Transmission"}</label>
                <select
                  value={transmissionType}
                  onChange={(e) => setTransmissionType(e.target.value)}
                  className={selectCls}
                  disabled={isUploading}
                >
                  <option value="">{t.upload?.select || "Select..."}</option>
                  <option value="Automatic">{t.upload?.automatic || "Automatic"}</option>
                  <option value="Manual">{t.upload?.manual || "Manual"}</option>
                </select>
              </div>

              {/* Troisième ligne: Customer - Stage - Date */}
              <div>
                <label className={labelCls}>{t.upload?.customer || "Customer"}</label>
                <input
                  type="text"
                  value={customer}
                  onChange={(e) => setCustomer(e.target.value)}
                  className={inputSmCls}
                  placeholder=""
                  disabled={isUploading}
                  spellCheck={false}
                />
              </div>

              <div>
                <label className={labelCls}>{t.upload?.stage || "Stage"}</label>
                <select
                  value={stage}
                  onChange={(e) => setStage(e.target.value)}
                  className={selectCls}
                  disabled={isUploading}
                >
                  <option value="">{t.upload?.select || "Select..."}</option>
                  <option value="Stage 1">Stage 1</option>
                  <option value="Stage 2">Stage 2</option>
                  <option value="Stage 3">Stage 3</option>
                </select>
              </div>

              <div>
                <label className={labelCls}>{t.upload?.date || "Date"}</label>
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
            <label className={labelCls}>{t.upload?.notes || "Notes (Optional)"}</label>
            <textarea
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              className={`${inputCls} resize-none`}
              rows={3}
              placeholder=""
              disabled={isUploading}
              spellCheck={false}
            />
          </div>

          {/* Action Buttons */}
          <div className="flex gap-3 pt-4">
            <Button
              onClick={handleCreateProject}
              disabled={!selectedFile || !projectName.trim() || isUploading || isAnalyzing || ecuIdentification?.ecu_type === "Unknown"}
              className="w-full bg-gradient-to-r from-red-600/90 via-red-500/90 to-orange-500/90 hover:from-red-500/90 hover:via-red-400/90 hover:to-orange-400/90 text-white shadow-lg shadow-red-500/20"
              size="lg"
            >
              {isUploading ? (
                <span className="inline-flex items-center">
                  {(t.common?.uploading || "Uploading...").replace(/\.{3}$/, '')}
                  <span className="inline-flex w-[18px]">
                    <span className="animate-[dotPulse_1.4s_infinite] [animation-delay:0s]">.</span>
                    <span className="animate-[dotPulse_1.4s_infinite] [animation-delay:0.2s]">.</span>
                    <span className="animate-[dotPulse_1.4s_infinite] [animation-delay:0.4s]">.</span>
                  </span>
                </span>
              ) : (
                <>
                  <Check className="w-4 h-4 mr-2" />
                  {t.upload?.createProject || "Create Project & Detect Maps"}
                </>
              )}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
