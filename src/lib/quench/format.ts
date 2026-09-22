export function formatBytes(bytes: number): string {
  const abs = Math.abs(bytes);
  if (abs >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
  if (abs >= 1024 ** 2) {
    const mb = bytes / 1024 ** 2;
    return mb >= 100 ? `${Math.round(mb)} MB` : `${mb.toFixed(1)} MB`;
  }
  if (abs >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${Math.round(bytes)} B`;
}

export function formatMs(ms: number): string {
  if (ms >= 1000) return `${(ms / 1000).toFixed(ms >= 10_000 ? 1 : 2)} s`;
  if (ms > 0 && ms < 1) {
    const microseconds = ms * 1000;
    if (microseconds >= 1) return `${Math.round(microseconds)} µs`;
    return `${ms.toFixed(3)} ms`;
  }
  return `${Math.round(ms)} ms`;
}

export function formatUsd(n: number): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: n >= 100 ? 0 : 0,
  }).format(Math.round(n));
}

export function formatPct(n: number): string {
  const pct = Math.round(n * 1000) / 10;
  const sign = pct > 0 ? "+" : "";
  return `${sign}${pct}%`;
}

export function improvePct(before: number, after: number, higherBetter = false): number {
  if (before === 0) return 0;
  if (higherBetter) return (after - before) / before;
  return (before - after) / before;
}

export function signedPct(before: number, after: number, higherBetter = false): string {
  const n = improvePct(before, after, higherBetter);
  const pct = Math.round(n * 1000) / 10;
  const sign = pct > 0 ? "−" : pct < 0 ? "+" : "";
  if (higherBetter) {
    const s = pct > 0 ? "+" : "";
    return `${s}${pct}%`;
  }
  return `${sign}${Math.abs(pct)}%`;
}

export function hashString(input: string): number {
  let h = 2166136261;
  for (let i = 0; i < input.length; i++) {
    h ^= input.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

export function certIdFrom(seed: string): string {
  const h = hashString(seed).toString(16).toUpperCase().padStart(8, "0");
  return `QC-2026-${h.slice(0, 6)}`;
}

export function relativeTime(ts: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - ts) / 1000));
  if (s < 15) return "just now";
  if (s < 60) return `${s}s ago`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h}h ago`;
  return `${Math.round(h / 24)}d ago`;
}

export function formatSha256(hex: string): string {
  return hex.toLowerCase();
}
