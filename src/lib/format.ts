import type { Language } from "./types";

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let idx = 0;
  while (value >= 1024 && idx < units.length - 1) {
    value /= 1024;
    idx++;
  }
  const decimals = idx === 0 ? 0 : value >= 100 ? 0 : value >= 10 ? 1 : 2;
  return `${value.toFixed(decimals)} ${units[idx]}`;
}

export function formatTimestamp(iso: string | null | undefined, lang: Language): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(lang === "tr" ? "tr-TR" : "en-US", {
    year: "numeric",
    month: "short",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function formatRelativeTime(iso: string | null | undefined, lang: Language): string {
  if (!iso) return lang === "tr" ? "Hiç" : "Never";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const diffSec = Math.max(0, Math.floor((Date.now() - d.getTime()) / 1000));
  if (diffSec < 10) return lang === "tr" ? "şimdi" : "just now";
  if (diffSec < 60) return lang === "tr" ? `${diffSec} sn önce` : `${diffSec}s ago`;
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return lang === "tr" ? `${diffMin} dk önce` : `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return lang === "tr" ? `${diffHr} sa önce` : `${diffHr}h ago`;
  const diffDay = Math.floor(diffHr / 24);
  return lang === "tr" ? `${diffDay} gün önce` : `${diffDay}d ago`;
}

export function formatPercent(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0%";
  return `${Math.min(100, value).toFixed(1)}%`;
}

export function shortenPath(fullPath: string, maxLen = 48): string {
  if (fullPath.length <= maxLen) return fullPath;
  const normalized = fullPath.replace(/\\/g, "/");
  const parts = normalized.split("/").filter(Boolean);
  if (parts.length <= 2) return `…${fullPath.slice(-(maxLen - 1))}`;
  const tail = parts.slice(-2).join("/");
  return `…/${tail}`;
}
