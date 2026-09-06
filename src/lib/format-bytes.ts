/** Human-readable size (KB/MB/GB, or Ko/Mo/Go in French), one decimal. */
export function formatBytes(bytes: number, language: string): string {
  const units = language === "fr" ? ["o", "Ko", "Mo", "Go"] : ["B", "KB", "MB", "GB"];
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  const digits = unit === 0 ? 0 : 1;
  return `${value.toLocaleString(language, { minimumFractionDigits: digits, maximumFractionDigits: digits })} ${units[unit]}`;
}
