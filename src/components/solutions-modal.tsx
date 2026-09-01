"use client";

// Fenêtre « Solutions » — corrections applicables au binaire.
// Une solution déjà active dans le fichier (sa carte est détectée) est
// affichée comme telle et ne peut pas être ré-appliquée.

import { useState, useMemo } from "react";
import { X, Zap, Shield, Cpu, Check, ChevronRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  getSolutionsForECU,
  getSolutionCategories,
  solutionMapPatterns,
  type Solution,
} from "@/lib/ecu/solutions";
import { useI18n } from "@/contexts/i18n-context";
import { getModalGlassStyle } from "@/lib/modal-glass";

interface DetectedMapInfo {
  name?: string;
  address?: number;
}

interface SolutionsModalProps {
  ecuType: string | undefined;
  onClose: () => void;
  onApplySolutions: (solutionIds: string[]) => void;
  isClosing?: boolean;
  theme?: "default" | "light" | "oled";
  /** Solutions déjà appliquées dans ce projet : { id: nom de version } */
  usedSolutions?: Record<string, string>;
  /** Cartes détectées dans le fichier (pour l'état « déjà actif ») */
  detectedMaps?: DetectedMapInfo[];
}

const iconMap: Record<string, React.ComponentType<{ className?: string }>> = {
  zap: Zap,
  shield: Shield,
  cpu: Cpu,
};

const categoryIconMap: Record<string, React.ComponentType<{ className?: string }>> = {
  performance: Zap,
  other: Cpu,
};

const categoryColorMap: Record<string, string> = {
  performance: "from-orange-500 to-red-600",
  other: "from-purple-500 to-pink-600",
};

export function SolutionsModal({
  ecuType,
  onClose,
  onApplySolutions,
  isClosing = false,
  theme = "default",
  usedSolutions = {},
  detectedMaps = [],
}: SolutionsModalProps) {
  const [selectedSolutions, setSelectedSolutions] = useState<Set<string>>(new Set());
  const { t } = useI18n();
  const tr = t.solutions as any;

  const getTranslatedSolution = (solution: Solution) => {
    const translated = (tr as Record<string, { name?: string; description?: string }>)[solution.id];
    return {
      name: translated?.name || solution.name,
      description: translated?.description || solution.description,
    };
  };

  const getTranslatedCategory = (categoryId: string) =>
    tr?.categories?.[categoryId] || categoryId;

  // Une solution est « déjà active » quand la carte qu'elle crée est détectée
  const isSolutionAlreadyActive = useMemo(() => {
    const activeMap: Record<string, boolean> = {};
    for (const [solutionId, patterns] of Object.entries(solutionMapPatterns)) {
      activeMap[solutionId] = detectedMaps.some((map) => {
        const mapName = map.name?.toLowerCase() || "";
        return patterns.some((pattern) => mapName.includes(pattern));
      });
    }
    return activeMap;
  }, [detectedMaps]);

  const ecuConfig = useMemo(() => getSolutionsForECU(ecuType), [ecuType]);
  const categories = getSolutionCategories();

  const solutionsByCategory = useMemo(() => {
    if (!ecuConfig) return {};
    const grouped: Record<string, Solution[]> = {};
    for (const solution of ecuConfig.solutions) {
      if (!grouped[solution.category]) grouped[solution.category] = [];
      grouped[solution.category].push(solution);
    }
    return grouped;
  }, [ecuConfig]);

  const [expandedCategories, setExpandedCategories] = useState<Set<string>>(
    () => new Set(categories.map((c) => c.id))
  );

  const toggleCategory = (categoryId: string) => {
    setExpandedCategories((prev) => {
      const next = new Set(prev);
      if (next.has(categoryId)) next.delete(categoryId);
      else next.add(categoryId);
      return next;
    });
  };

  const toggleSolution = (solutionId: string) => {
    setSelectedSolutions((prev) => {
      const next = new Set(prev);
      if (next.has(solutionId)) next.delete(solutionId);
      else next.add(solutionId);
      return next;
    });
  };

  const handleApply = () => {
    onApplySolutions(Array.from(selectedSolutions));
    onClose();
  };

  const getTextColor = () => (theme === "light" ? "#000000" : "#ffffff");
  const getSecondaryTextColor = () =>
    theme === "light" ? "rgba(0, 0, 0, 0.6)" : "rgba(255, 255, 255, 0.6)";
  const getBgHover = () => (theme === "light" ? "hover:bg-black/5" : "hover:bg-white/5");
  const getBorderColor = () =>
    theme === "light" ? "rgba(0, 0, 0, 0.1)" : "rgba(255, 255, 255, 0.1)";

  return (
    <div
      className="fixed inset-0 z-[70] flex items-center justify-center backdrop-blur-sm"
      style={{
        backgroundColor: "#000000a2",
        animation: isClosing
          ? "backdropFadeOut 0.2s ease-out forwards"
          : "backdropFadeIn 0.2s ease-out forwards",
      }}
      // Ne se ferme jamais au clic sur le fond — uniquement via les boutons
      onClick={(e) => e.stopPropagation()}
    >
      <div
        className="relative w-full max-w-2xl max-h-[90vh] overflow-hidden mx-4"
        style={{
          animation: isClosing
            ? "modalCollapse 0.2s ease-out forwards"
            : "modalExpand 0.2s ease-out forwards",
        }}
      >
        <button
          onClick={onClose}
          className={`absolute top-4 right-4 z-10 p-2 rounded-lg transition-colors ${getBgHover()}`}
          style={{ color: getSecondaryTextColor() }}
        >
          <X className="w-5 h-5" />
        </button>

        <div className="border rounded-lg overflow-hidden" style={getModalGlassStyle(theme)}>
          {/* En-tête */}
          <div className="p-6 border-b" style={{ borderColor: getBorderColor() }}>
            <div className="flex items-center gap-4">
              <div className="w-12 h-12 rounded-xl bg-gradient-to-br from-red-600 via-red-500 to-orange-500 flex items-center justify-center">
                <Cpu className="w-6 h-6 text-white" />
              </div>
              <div>
                <h2 className="text-2xl font-bold" style={{ color: getTextColor() }}>
                  {tr?.title || "Solutions"}
                </h2>
                <p className="text-sm" style={{ color: getSecondaryTextColor() }}>
                  {ecuConfig ? (
                    <>
                      <span className="font-medium" style={{ color: getTextColor() }}>
                        {ecuConfig.manufacturer} {ecuConfig.ecuType}
                      </span>
                      <span className="mx-2">-</span>
                      <span>
                        {ecuConfig.solutions.length}{" "}
                        {tr?.solutionsAvailable || "solutions available"}
                      </span>
                    </>
                  ) : (
                    <>
                      {/* Calculateur sans solution : on rappelle quand même
                          lequel est chargé, puis le motif */}
                      {ecuType && (
                        <>
                          <span className="font-medium" style={{ color: getTextColor() }}>
                            {ecuType}
                          </span>
                          <span className="mx-2">-</span>
                        </>
                      )}
                      <span>
                        {tr?.noSolutionsForEcu || "No solution is available for this ECU"}
                      </span>
                    </>
                  )}
                </p>
              </div>
            </div>
          </div>

          {/* Liste */}
          <div className="p-6 overflow-y-auto max-h-[60vh] upload-scroll">
            {!ecuConfig ? (
              <div className="text-center py-10">
                <Cpu className="w-14 h-14 mx-auto mb-4 opacity-30" style={{ color: getTextColor() }} />
                <p className="text-lg font-medium mb-2" style={{ color: getTextColor() }}>
                  {tr?.noSolutionsForEcu || "No solution is available for this ECU"}
                </p>
                <p style={{ color: getSecondaryTextColor() }}>
                  {tr?.noSolutionsForEcuDescription ||
                    "Solutions currently cover Bosch EDC15P and EDC15VM."}
                </p>
              </div>
            ) : (
              <div className="space-y-4">
                {categories.map((category) => {
                  const solutions = solutionsByCategory[category.id] || [];
                  if (solutions.length === 0) return null;

                  const CategoryIcon = categoryIconMap[category.id] || Cpu;
                  const isExpanded = expandedCategories.has(category.id);
                  const categoryColor = categoryColorMap[category.id] || "from-gray-500 to-gray-600";

                  return (
                    <div
                      key={category.id}
                      className="rounded-xl overflow-hidden border"
                      style={{ borderColor: getBorderColor() }}
                    >
                      <button
                        onClick={() => toggleCategory(category.id)}
                        className={`w-full flex items-center gap-3 p-4 ${getBgHover()} transition-colors`}
                      >
                        <div
                          className={`w-10 h-10 rounded-lg bg-gradient-to-br ${categoryColor} flex items-center justify-center`}
                        >
                          <CategoryIcon className="w-5 h-5 text-white" />
                        </div>
                        <div className="flex-1 text-left">
                          <h3 className="font-semibold" style={{ color: getTextColor() }}>
                            {getTranslatedCategory(category.id)}
                          </h3>
                          <p className="text-xs" style={{ color: getSecondaryTextColor() }}>
                            {solutions.length}{" "}
                            {solutions.length > 1
                              ? tr?.solutionsCount || "solutions"
                              : tr?.solutionCount || "solution"}
                          </p>
                        </div>
                        <ChevronRight
                          className={`w-5 h-5 transition-transform duration-200 ${isExpanded ? "rotate-90" : ""}`}
                          style={{ color: getSecondaryTextColor() }}
                        />
                      </button>

                      <div
                        className={`overflow-hidden transition-all duration-300 ${isExpanded ? "max-h-[1000px]" : "max-h-0"}`}
                      >
                        <div className="p-2 grid gap-2">
                          {solutions.map((solution) => {
                            const SolutionIcon = iconMap[solution.icon] || Shield;
                            const isSelected = selectedSolutions.has(solution.id);
                            const isUsed = solution.id in usedSolutions;
                            const usedOnVersion = usedSolutions[solution.id];
                            const isAlreadyActive =
                              !isUsed && (isSolutionAlreadyActive[solution.id] || false);
                            const isDisabled = isUsed || isAlreadyActive;

                            return (
                              <button
                                key={solution.id}
                                onClick={() => !isDisabled && toggleSolution(solution.id)}
                                disabled={isDisabled}
                                className={`flex items-center gap-3 p-3 rounded-lg border transition-all ${
                                  isDisabled
                                    ? "opacity-60 cursor-not-allowed"
                                    : isSelected
                                      ? "bg-gradient-to-r from-red-500/20 to-orange-500/20 border-red-500/50"
                                      : `${getBgHover()} border-transparent`
                                }`}
                                style={{
                                  borderColor:
                                    isSelected && !isDisabled ? undefined : getBorderColor(),
                                }}
                              >
                                <div
                                  className={`w-10 h-10 rounded-lg flex items-center justify-center ${
                                    isDisabled
                                      ? "bg-gradient-to-br from-slate-500 to-slate-600"
                                      : isSelected
                                        ? "bg-gradient-to-br from-red-500 to-orange-500"
                                        : "bg-gradient-to-br from-slate-600 to-slate-700"
                                  }`}
                                >
                                  <SolutionIcon className="w-5 h-5 text-white" />
                                </div>
                                <div className="flex-1 text-left">
                                  <h4 className="font-medium" style={{ color: getTextColor() }}>
                                    {getTranslatedSolution(solution).name}
                                  </h4>
                                  <p
                                    className="text-xs line-clamp-1"
                                    style={{ color: getSecondaryTextColor() }}
                                  >
                                    {getTranslatedSolution(solution).description}
                                  </p>
                                </div>
                                <div className="flex items-center gap-2">
                                  {isUsed ? (
                                    <span
                                      className="flex items-center gap-1 text-xs px-2 py-1 rounded-full bg-blue-500/20 text-blue-400"
                                      title={usedOnVersion}
                                    >
                                      <Check className="w-3 h-3" />
                                      {usedOnVersion}
                                    </span>
                                  ) : isAlreadyActive ? (
                                    <span className="flex items-center gap-1 text-xs px-2 py-1 rounded-full bg-emerald-500/20 text-emerald-400">
                                      <Check className="w-3 h-3" />
                                      {tr?.alreadyActive || "Already active"}
                                    </span>
                                  ) : null}
                                  {!isDisabled && (
                                    <div
                                      className={`w-6 h-6 rounded-full border-2 flex items-center justify-center transition-colors ${
                                        isSelected
                                          ? "bg-red-500 border-red-500"
                                          : "border-slate-500"
                                      }`}
                                    >
                                      {isSelected && <Check className="w-4 h-4 text-white" />}
                                    </div>
                                  )}
                                </div>
                              </button>
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

          {/* Pied */}
          {ecuConfig && (
            <div
              className="p-4 border-t flex items-center justify-between"
              style={{ borderColor: getBorderColor() }}
            >
              <span className="text-sm" style={{ color: getSecondaryTextColor() }}>
                {selectedSolutions.size}{" "}
                {selectedSolutions.size !== 1
                  ? tr?.solutionsCount || "solutions"
                  : tr?.solutionCount || "solution"}{" "}
                {tr?.selected || "selected"}
              </span>
              <Button
                onClick={handleApply}
                disabled={selectedSolutions.size === 0}
                className="px-6 bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400 text-white disabled:opacity-50"
              >
                {tr?.applySolutions || "Apply Solutions"}
              </Button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
