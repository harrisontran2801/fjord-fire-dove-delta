function publishedHost(): string {
  const raw = String(import.meta.env?.VITE_PUBLIC_HOSTNAME ?? "").trim();
  const host = raw.replace(/^https?:\/\//i, "").split("/")[0]?.toLowerCase() ?? "";
  if (!host || !host.includes(".")) return "";
  if (host === "localhost" || host.endsWith(".localhost")) return "";
  if (host.endsWith(".vercel.app") || host.endsWith(".vercel.com")) return "";
  if (/^\d{1,3}(?:\.\d{1,3}){3}$/.test(host)) return "";
  if (host.endsWith(".local") || host === "127.0.0.1") return "";
  return host;
}

export function reportPublicUrl(runId: string): string | null {
  const published = publishedHost();
  if (!published) return null;
  return `https://${published}/cert/${encodeURIComponent(runId)}`;
}

export function badgeUnavailableReason(mode: "demo" | "inspect" | "agent"): string {
  if (mode === "inspect") {
    return "Uploaded files produce a local inspection report, not a public badge.";
  }
  if (mode === "agent") {
    return "Local agent reports stay on this machine. A README badge is only enabled when a public hostname is configured.";
  }
  return "No public URL is configured for this report, so the README badge is disabled.";
}
