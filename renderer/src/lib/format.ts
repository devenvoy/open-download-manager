const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

/** Format a byte count with one decimal for every unit above bytes. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    UNITS.length - 1,
  );
  const value = bytes / 1024 ** exponent;
  const unit = UNITS[exponent] ?? "B";
  return exponent === 0 ? `${value} ${unit}` : `${value.toFixed(1)} ${unit}`;
}

/** Format a transfer rate. */
export function formatSpeed(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

/** Format a 0–100 percentage, or an em dash when the total is unknown. */
export function formatPercent(downloaded: number, total: number | null): string {
  if (!total || total <= 0) return "—";
  return `${Math.min(100, Math.round((downloaded / total) * 100))}%`;
}

/** Format a seconds count as a short human duration. */
export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "—";
  const whole = Math.round(seconds);
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  const remainder = whole % 60;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${remainder}s`;
  return `${remainder}s`;
}
