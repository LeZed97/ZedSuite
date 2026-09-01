"use client";

import { useState, useMemo, useCallback, useEffect } from "react";
import { X, Search, AlertTriangle, Check, ChevronRight, Power, PowerOff, Cpu, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { getModalGlassStyle } from "@/lib/modal-glass";
import {
  detectEDC15PDTCs,
  detectEDC16DTCs,
  getEDC15PDTCSystems as getDetectedDTCSystems,
  translateSystemName,
  translateDTCDescription,
  type DetectedDTC,
  type DTCDetectionResult,
  type CodeblockInfo,
  type DTCLanguage,
} from "@/lib/ecu/bosch/dtc";
import { useI18n } from "@/contexts/i18n-context";

interface DTCModalProps {
  ecuType: string | undefined;
  fileData: number[] | Uint8Array | null; // Binary file data (can be number[] or Uint8Array)
  onClose: () => void;
  onDisableDTCs: (dtcs: DetectedDTC[], codeblocks: CodeblockInfo[]) => void;
  isClosing?: boolean;
  theme?: 'default' | 'light' | 'oled';
  refreshKey?: number; // Used to force re-detection when file data changes
}

export function DTCModal({
  ecuType,
  fileData,
  onClose,
  onDisableDTCs,
  isClosing = false,
  theme = 'default',
  refreshKey = 0,
}: DTCModalProps) {
  const { t, language } = useI18n();
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedDTCs, setSelectedDTCs] = useState<Set<string>>(new Set());
  const [filterSystem, setFilterSystem] = useState<string | null>(null);
  const [isDetecting, setIsDetecting] = useState(true);
  const [detectionResult, setDetectionResult] = useState<DTCDetectionResult | null>(null);
  // Track expanded systems - all expanded by default (initialized after systems are detected)
  const [expandedSystems, setExpandedSystems] = useState<Set<string>>(new Set());

  // Detect DTCs when file data changes
  useEffect(() => {
    if (!fileData || fileData.length === 0) {
      setIsDetecting(false);
      setDetectionResult(null);
      return;
    }

    setIsDetecting(true);

    // Convert to Uint8Array if needed
    const dataArray = fileData instanceof Uint8Array ? fileData : new Uint8Array(fileData);

    // Run detection in a setTimeout to avoid blocking UI
    const timeoutId = setTimeout(() => {
      try {
        // Determine which detector to use based on ECU type
        const ecuFamily = ecuType?.toUpperCase() || '';

        let result: DTCDetectionResult;

        if (ecuFamily.includes('EDC15')) {
          result = detectEDC15PDTCs(dataArray);
        } else if (ecuFamily.includes('EDC16')) {
          result = detectEDC16DTCs(dataArray);
        } else {
          // For other ECU types, try EDC16 detector as fallback for VAG ECUs
          // since they share similar DTC structures
          result = detectEDC16DTCs(dataArray);
          if (!result.success || result.dtcs.length === 0) {
            result = {
              success: false,
              ecuType: ecuType || 'Unknown',
              codeblocks: [],
              dtcs: [],
              errors: [`DTC detection not yet implemented for ${ecuType || 'unknown ECU type'}`]
            };
          }
        }

        setDetectionResult(result);
      } catch (error) {
        console.error('DTC detection error:', error);
        setDetectionResult({
          success: false,
          ecuType: ecuType || 'Unknown',
          codeblocks: [],
          dtcs: [],
          errors: [error instanceof Error ? error.message : 'Unknown error during DTC detection']
        });
      } finally {
        setIsDetecting(false);
      }
    }, 100);

    return () => clearTimeout(timeoutId);
  }, [fileData, ecuType, refreshKey]);

  // Get list of systems from detected DTCs
  const systems = useMemo(() => {
    if (!detectionResult?.dtcs) return [];
    return getDetectedDTCSystems(detectionResult.dtcs);
  }, [detectionResult]);

  // Initialize all systems as expanded when detection completes
  useEffect(() => {
    if (systems.length > 0) {
      setExpandedSystems(new Set(systems));
    }
  }, [systems]);

  // Toggle a single system expansion
  const toggleSystem = useCallback((systemId: string) => {
    setExpandedSystems(prev => {
      const newSet = new Set(prev);
      if (newSet.has(systemId)) {
        newSet.delete(systemId);
      } else {
        newSet.add(systemId);
      }
      return newSet;
    });
  }, []);

  // Group DTCs by system
  const dtcsBySystem = useMemo(() => {
    if (!detectionResult?.dtcs) return {};

    const grouped: Record<string, DetectedDTC[]> = {};
    detectionResult.dtcs.forEach(dtc => {
      const system = dtc.system || 'Unknown';
      if (!grouped[system]) {
        grouped[system] = [];
      }
      grouped[system].push(dtc);
    });

    return grouped;
  }, [detectionResult]);

  // Filter DTCs based on search, system filter, and enabled filter
  const filteredDTCsBySystem = useMemo(() => {
    const filtered: Record<string, DetectedDTC[]> = {};
    const query = searchQuery.toLowerCase().trim();

    Object.entries(dtcsBySystem).forEach(([system, dtcs]) => {
      // Apply system filter
      if (filterSystem && system !== filterSystem) return;

      const matchingDTCs = dtcs.filter(dtc => {
        // Apply search filter
        if (query) {
          const matchesCode = dtc.code.toLowerCase().includes(query);
          const matchesDescription = (dtc.description || '').toLowerCase().includes(query);
          const matchesVagCode = dtc.vagCode.toString().includes(query);
          if (!matchesCode && !matchesDescription && !matchesVagCode) return false;
        }

        return true;
      });

      if (matchingDTCs.length > 0) {
        filtered[system] = matchingDTCs;
      }
    });

    return filtered;
  }, [dtcsBySystem, searchQuery, filterSystem]);

  // Total filtered DTCs count
  const totalFilteredDTCs = useMemo(() => {
    return Object.values(filteredDTCsBySystem).reduce((sum, dtcs) => sum + dtcs.length, 0);
  }, [filteredDTCsBySystem]);

  // Enabled DTCs count
  const enabledDTCsCount = useMemo(() => {
    if (!detectionResult?.dtcs) return 0;
    return detectionResult.dtcs.filter(dtc => dtc.enabled).length;
  }, [detectionResult]);

  // Toggle DTC selection
  const toggleDTC = useCallback((code: string) => {
    setSelectedDTCs(prev => {
      const newSet = new Set(prev);
      if (newSet.has(code)) {
        newSet.delete(code);
      } else {
        newSet.add(code);
      }
      return newSet;
    });
  }, []);

  // Select all visible DTCs (only those that are enabled and can be disabled)
  const selectAllVisible = useCallback(() => {
    const allCodes = new Set<string>();
    Object.values(filteredDTCsBySystem).forEach(dtcs => {
      dtcs.forEach(dtc => {
        if (dtc.enabled && dtc.canDisable !== false) {
          allCodes.add(dtc.code);
        }
      });
    });
    setSelectedDTCs(allCodes);
  }, [filteredDTCsBySystem]);

  // Select all enabled DTCs (only those that can be disabled)
  const selectAllEnabled = useCallback(() => {
    const enabledCodes = new Set<string>();
    Object.values(filteredDTCsBySystem).forEach(dtcs => {
      dtcs.forEach(dtc => {
        if (dtc.enabled && dtc.canDisable !== false) {
          enabledCodes.add(dtc.code);
        }
      });
    });
    setSelectedDTCs(enabledCodes);
  }, [filteredDTCsBySystem]);

  // Clear selection
  const clearSelection = useCallback(() => {
    setSelectedDTCs(new Set());
  }, []);

  // Disable selected DTCs
  const handleDisableSelected = useCallback(() => {
    if (!detectionResult) return;

    const selectedDTCList = detectionResult.dtcs.filter(dtc => selectedDTCs.has(dtc.code));
    onDisableDTCs(selectedDTCList, detectionResult.codeblocks);
    setSelectedDTCs(new Set());
  }, [selectedDTCs, detectionResult, onDisableDTCs]);


  // Theme-aware colors
  const getTextColor = () => theme === 'light' ? '#000000' : '#ffffff';
  const getSecondaryTextColor = () => theme === 'light' ? 'rgba(0, 0, 0, 0.6)' : 'rgba(255, 255, 255, 0.6)';
  const getBgHover = () => theme === 'light' ? 'hover:bg-black/5' : 'hover:bg-white/5';
  const getBorderColor = () => theme === 'light' ? 'rgba(0, 0, 0, 0.1)' : 'rgba(255, 255, 255, 0.1)';
  const getInputBg = () => theme === 'light' ? 'bg-black/5' : 'bg-white/5';
  // Couleurs d'accent renforcées sur fond clair (les teintes 400 se délavent)
  const isLight = theme === 'light';
  const greenText = isLight ? 'text-green-700' : 'text-green-400';
  const redText = isLight ? 'text-red-600' : 'text-red-400';
  const greenChip = isLight ? 'bg-green-600/15 text-green-700' : 'bg-green-500/20 text-green-400';
  const redChip = isLight ? 'bg-red-600/15 text-red-700' : 'bg-red-500/20 text-red-400';
  const activePill = isLight ? 'bg-orange-500/15 border-orange-600/60 text-orange-700' : 'bg-orange-500/20 border-orange-500/50 text-orange-400';

  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center backdrop-blur-sm"
      style={{
        backgroundColor: '#000000a2',
        animation: isClosing ? 'backdropFadeOut 0.2s ease-out forwards' : 'backdropFadeIn 0.2s ease-out forwards'
      }}
    >
      <div
        className="relative w-full max-w-5xl max-h-[90vh] overflow-hidden"
        style={{
          animation: isClosing ? 'modalCollapse 0.2s ease-out forwards' : 'modalExpand 0.2s ease-out forwards'
        }}
      >
        {/* Close button */}
        <button
          onClick={onClose}
          className={`absolute top-4 right-4 z-10 p-2 rounded-lg transition-colors ${getBgHover()}`}
          style={{ color: getSecondaryTextColor() }}
        >
          <X className="w-5 h-5" />
        </button>

        {/* Modal Content */}
        <div className="max-w-5xl mx-auto">
          <div className="border rounded-lg overflow-hidden flex flex-col" style={{ ...getModalGlassStyle(theme), maxHeight: '90vh' }}>
            {/* Header */}
            <div className="p-6 border-b flex-shrink-0" style={{ borderColor: getBorderColor() }}>
              <div className="flex items-center gap-4">
                <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-amber-400 via-yellow-500 to-amber-500 flex items-center justify-center">
                  <AlertTriangle className="w-6 h-6 text-white" />
                </div>
                <div className="flex-1">
                  <h2 className="text-2xl font-bold" style={{ color: getTextColor() }}>{t.dtcModal.title}</h2>
                  <p className="text-sm" style={{ color: getSecondaryTextColor() }}>
                    {isDetecting ? (
                      <span className="flex items-center gap-2">
                        <Loader2 className="w-4 h-4 animate-spin" />
                        {t.dtcModal.detecting}
                      </span>
                    ) : detectionResult?.success ? (
                      <>
                        <span className="font-medium" style={{ color: getTextColor() }}>{ecuType || t.dtcModal.unknownECU}</span>
                        <span className="mx-2">-</span>
                        <span>{detectionResult.dtcs.length} {t.dtcModal.codesDetected}</span>
                        <span className="mx-2">-</span>
                        <span className={greenText}>{enabledDTCsCount} {t.dtcModal.enabled}</span>
                      </>
                    ) : (
                      <span className={redText}>
                        {detectionResult?.errors?.[0] || t.dtcModal.noFileLoaded}
                      </span>
                    )}
                  </p>
                </div>
              </div>
            </div>

            {/* Search and Filters */}
            {detectionResult?.success && detectionResult.dtcs.length > 0 && (
              <div className="p-4 border-b flex-shrink-0 space-y-3" style={{ borderColor: getBorderColor() }}>
                {/* Search Bar */}
                <div className="flex gap-3">
                  <div className="relative flex-1">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4" style={{ color: getSecondaryTextColor() }} />
                    <input
                      type="text"
                      value={searchQuery}
                      onChange={(e) => setSearchQuery(e.target.value)}
                      placeholder={t.dtcModal.searchPlaceholder}
                      className={`w-full pl-10 pr-4 py-2 ${getInputBg()} border rounded-lg focus:outline-none focus:ring-2 focus:ring-orange-500/50`}
                      style={{ color: getTextColor(), borderColor: getBorderColor() }}
                    />
                  </div>
                </div>

                {/* System Filter Pills */}
                <div className="flex flex-wrap gap-2">
                  <button
                    onClick={() => setFilterSystem(null)}
                    className={`px-3 py-1 text-xs rounded-full border transition-colors ${
                      filterSystem === null
                        ? activePill
                        : `${getBgHover()} border-transparent`
                    }`}
                    style={{ borderColor: filterSystem === null ? undefined : getBorderColor(), color: filterSystem === null ? undefined : getTextColor() }}
                  >
                    {t.dtcModal.allSystems} ({detectionResult.dtcs.length})
                  </button>
                  {systems.map(system => {
                    const count = dtcsBySystem[system]?.length || 0;
                    const translatedSystem = translateSystemName(system, language as DTCLanguage);
                    return (
                      <button
                        key={system}
                        onClick={() => setFilterSystem(filterSystem === system ? null : system)}
                        className={`px-3 py-1 text-xs rounded-full border transition-colors ${
                          filterSystem === system
                            ? activePill
                            : `${getBgHover()} border-transparent`
                        }`}
                        style={{ borderColor: filterSystem === system ? undefined : getBorderColor(), color: filterSystem === system ? undefined : getTextColor() }}
                      >
                        {translatedSystem} ({count})
                      </button>
                    );
                  })}
                </div>
              </div>
            )}

            {/* DTC List */}
            <div className="flex-1 overflow-y-auto p-4 upload-scroll" style={{ minHeight: 0 }}>
              {isDetecting ? (
                <div className="text-center py-12">
                  <Loader2 className="w-16 h-16 mx-auto mb-4 animate-spin" style={{ color: getTextColor(), opacity: 0.3 }} />
                  <p className="text-lg font-medium mb-2" style={{ color: getTextColor() }}>{t.dtcModal.scanningBinary}</p>
                  <p style={{ color: getSecondaryTextColor() }}>
                    {t.dtcModal.detectingCodes}
                  </p>
                </div>
              ) : !detectionResult?.success ? (
                <div className="text-center py-12">
                  <AlertTriangle className="w-16 h-16 mx-auto mb-4 opacity-30" style={{ color: getTextColor() }} />
                  <p className="text-lg font-medium mb-2" style={{ color: getTextColor() }}>{t.dtcModal.detectionFailed}</p>
                  <p style={{ color: getSecondaryTextColor() }}>
                    {detectionResult?.errors?.[0] || t.dtcModal.unableToDetect}
                  </p>
                </div>
              ) : detectionResult.dtcs.length === 0 ? (
                <div className="text-center py-12">
                  <Cpu className="w-16 h-16 mx-auto mb-4 opacity-30" style={{ color: getTextColor() }} />
                  <p className="text-lg font-medium mb-2" style={{ color: getTextColor() }}>{t.dtcModal.noDTCsFound}</p>
                  <p style={{ color: getSecondaryTextColor() }}>
                    {t.dtcModal.noDTCsDetected}
                  </p>
                </div>
              ) : totalFilteredDTCs === 0 ? (
                <div className="text-center py-12">
                  <Search className="w-16 h-16 mx-auto mb-4 opacity-30" style={{ color: getTextColor() }} />
                  <p className="text-lg font-medium mb-2" style={{ color: getTextColor() }}>{t.dtcModal.noResults}</p>
                  <p style={{ color: getSecondaryTextColor() }}>
                    {t.dtcModal.noMatchingCodes}
                  </p>
                </div>
              ) : (
                <div className="space-y-3">
                  {Object.entries(filteredDTCsBySystem).map(([system, dtcs]) => {
                    const isExpanded = expandedSystems.has(system) || searchQuery.length > 0;
                    const enabledInSystem = dtcs.filter(d => d.enabled).length;
                    const translatedSystemName = translateSystemName(system, language as DTCLanguage);

                    return (
                      <div key={system} className="rounded-xl overflow-hidden border" style={{ borderColor: getBorderColor() }}>
                        {/* System Header */}
                        <button
                          onClick={() => toggleSystem(system)}
                          className={`w-full flex items-center gap-3 p-3 ${getBgHover()} transition-colors`}
                        >
                          <ChevronRight
                            className={`w-4 h-4 transition-transform duration-200 ${isExpanded ? 'rotate-90' : ''}`}
                            style={{ color: getSecondaryTextColor() }}
                          />
                          <span className="font-semibold" style={{ color: getTextColor() }}>{translatedSystemName}</span>
                          <span className="text-xs px-2 py-0.5 rounded-full" style={{ backgroundColor: getBorderColor(), color: getSecondaryTextColor() }}>
                            {dtcs.length}
                          </span>
                          {enabledInSystem > 0 && (
                            <span className={`text-xs px-2 py-0.5 rounded-full ${greenChip}`}>
                              {enabledInSystem} {t.dtcModal.enabled}
                            </span>
                          )}
                        </button>

                        {/* DTC Items */}
                        <div className={`overflow-hidden transition-all duration-300 ${isExpanded ? 'max-h-[2000px]' : 'max-h-0'}`}>
                          <div className="border-t" style={{ borderColor: getBorderColor() }}>
                            {dtcs.map(dtc => {
                              const isSelected = selectedDTCs.has(dtc.code);
                              const canDisable = dtc.canDisable !== false; // Default to true for backward compatibility
                              const isSelectable = dtc.enabled && canDisable; // Can only select enabled DTCs that can be disabled

                              return (
                                <div
                                  key={`${dtc.code}-${dtc.address}`}
                                  className={`flex items-center gap-3 px-4 py-2 border-b last:border-b-0 transition-colors ${
                                    isSelectable ? `${getBgHover()} cursor-pointer` : 'opacity-50 cursor-not-allowed'
                                  } ${isSelected ? 'bg-orange-500/10' : ''}`}
                                  style={{ borderColor: getBorderColor() }}
                                  onClick={() => isSelectable && toggleDTC(dtc.code)}
                                  title={!dtc.enabled ? t.dtcModal.alreadyDisabled : !canDisable ? t.dtcModal.noMapping : undefined}
                                >
                                  {/* Selection Checkbox */}
                                  <div className={`w-5 h-5 rounded border-2 flex items-center justify-center transition-colors ${
                                    isSelected
                                      ? 'bg-orange-500 border-orange-500'
                                      : isSelectable ? '' : 'opacity-30'
                                  }`} style={{ borderColor: isSelected ? undefined : (isLight ? 'rgba(0, 0, 0, 0.35)' : getBorderColor()) }}>
                                    {isSelected && <Check className="w-3 h-3 text-white" />}
                                  </div>

                                  {/* Status Indicator */}
                                  <div className={`w-8 h-8 rounded-lg flex items-center justify-center ${
                                    dtc.enabled ? (isLight ? 'bg-green-600/15' : 'bg-green-500/20') : (isLight ? 'bg-red-600/15' : 'bg-red-500/20')
                                  }`}>
                                    {dtc.enabled ? (
                                      <Power className={`w-4 h-4 ${isLight ? "text-green-600" : "text-green-500"}`} />
                                    ) : (
                                      <PowerOff className={`w-4 h-4 ${isLight ? "text-red-600" : "text-red-500"}`} />
                                    )}
                                  </div>

                                  {/* DTC Code */}
                                  <span className="font-mono text-sm font-bold w-16" style={{ color: getTextColor() }}>
                                    {dtc.code}
                                  </span>

                                  {/* VAG Code */}
                                  <span className="font-mono text-xs w-14" style={{ color: getSecondaryTextColor() }}>
                                    {dtc.vagCode}
                                  </span>

                                  {/* Description */}
                                  <span className="flex-1 text-sm truncate" style={{ color: getSecondaryTextColor() }}>
                                    {translateDTCDescription(dtc.code, dtc.description || '', language as DTCLanguage)}
                                  </span>

                                  {/* Address */}
                                  <span className="font-mono text-xs hidden md:block" style={{ color: getSecondaryTextColor() }}>
                                    0x{dtc.address.toString(16).toUpperCase().padStart(5, '0')}
                                  </span>

                                  {/* Status Badge */}
                                  <span className={`text-xs px-2 py-0.5 rounded-full ${
                                    dtc.enabled
                                      ? greenChip
                                      : redChip
                                  }`}>
                                    {dtc.enabled ? 'ON' : 'OFF'}
                                  </span>

                                  {/* No Mapping Indicator */}
                                  {!canDisable && (
                                    <span className="text-xs px-2 py-0.5 rounded-full bg-yellow-500/20 text-yellow-400" title={t.dtcModal.noMapping}>
                                      N/A
                                    </span>
                                  )}
                                </div>
                              );
                            })}
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>

            {/* Footer with Actions */}
            {detectionResult?.success && detectionResult.dtcs.length > 0 && (
              <div className="p-4 border-t flex-shrink-0 flex items-center" style={{ borderColor: getBorderColor() }}>
                {/* Left section - selection info */}
                <div className="flex items-center gap-4 flex-1">
                  <span className="text-sm" style={{ color: getSecondaryTextColor() }}>
                    {selectedDTCs.size} {t.dtcModal.codesSelected}
                  </span>
                  <button
                    onClick={clearSelection}
                    className="text-xs px-2 py-1 rounded hover:bg-white/10 transition-colors"
                    style={{ color: getSecondaryTextColor() }}
                  >
                    {t.dtcModal.clear}
                  </button>
                </div>
                {/* Center section - empty for balance */}
                <div className="flex-1" />
                {/* Right section - disable button */}
                <div className="flex-1 flex justify-end">
                  <Button
                    onClick={handleDisableSelected}
                    disabled={selectedDTCs.size === 0}
                    className="px-6 bg-gradient-to-r from-orange-600 via-red-500 to-red-600 hover:from-orange-500 hover:via-red-400 hover:to-red-500 text-white disabled:opacity-50"
                  >
                    <PowerOff className="w-4 h-4 mr-2" />
                    {t.dtcModal.disableSelected}
                  </Button>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
