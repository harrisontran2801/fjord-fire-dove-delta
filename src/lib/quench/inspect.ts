import type { ArchiveFacts, Artifact, ArtifactKind, ElfFacts } from "./types";

export const UNSUPPORTED_FORMAT =
  "Unsupported format. Supported formats: ELF binary, Docker save, OCI archive.";

export const MAX_UPLOAD_BYTES = 96 * 1024 * 1024;
export const MAX_UPLOAD_LABEL = "96 MB";

export const FILE_TOO_LARGE = `This file is larger than ${MAX_UPLOAD_LABEL}. Choose a smaller ELF, Docker save, or OCI archive.`;

export const READ_FAILED =
  "Could not read this file. Check that it is readable and try again.";

export type InspectOutcome =
  | {
      ok: true;
      kind: ArtifactKind;
      name: string;
      sizeBytes: number;
      sha256: string;
      elf?: ElfFacts;
      archive?: ArchiveFacts;
    }
  | { ok: false; error: string };

const ELF_MACHINES: Record<number, string> = {
  0: "unspecified",
  3: "x86",
  8: "MIPS",
  20: "PowerPC",
  21: "PowerPC64",
  22: "S390",
  40: "ARM",
  42: "SuperH",
  50: "IA-64",
  62: "x86-64",
  183: "AArch64",
  243: "RISC-V",
};

function u8eq(bytes: Uint8Array, offset: number, sig: number[]): boolean {
  if (bytes.length < offset + sig.length) return false;
  return sig.every((b, i) => bytes[offset + i] === b);
}

function isElf(bytes: Uint8Array): boolean {
  return u8eq(bytes, 0, [0x7f, 0x45, 0x4c, 0x46]);
}

function isGzip(bytes: Uint8Array): boolean {
  return u8eq(bytes, 0, [0x1f, 0x8b]);
}

function latin1(bytes: Uint8Array, start: number, end: number): string {
  let out = "";
  const last = Math.min(end, bytes.length);
  for (let i = start; i < last; i++) {
    const c = bytes[i] ?? 0;
    if (c === 0) break;
    out += String.fromCharCode(c);
  }
  return out;
}

function readU16(bytes: Uint8Array, offset: number, little: boolean): number {
  const a = bytes[offset] ?? 0;
  const b = bytes[offset + 1] ?? 0;
  return little ? a | (b << 8) : (a << 8) | b;
}

function parseElf(bytes: Uint8Array): ElfFacts | null {
  if (!isElf(bytes) || bytes.length < 20) return null;
  const classByte = bytes[4] ?? 0;
  const dataByte = bytes[5] ?? 0;
  if (classByte !== 1 && classByte !== 2) return null;
  if (dataByte !== 1 && dataByte !== 2) return null;
  const little = dataByte === 1;
  const machineCode = readU16(bytes, 18, little);
  return {
    classBits: classByte === 1 ? 32 : 64,
    endian: little ? "little" : "big",
    machine: ELF_MACHINES[machineCode] ?? `machine-${machineCode}`,
    machineCode,
  };
}

interface TarEntry {
  name: string;
  size: number;
  offset: number;
}

function parseOctal(raw: string): number {
  const trimmed = raw.replace(/\0/g, "").trim();
  if (!trimmed) return 0;
  const n = Number.parseInt(trimmed, 8);
  return Number.isFinite(n) ? n : 0;
}

function looksLikeTar(bytes: Uint8Array): boolean {
  if (bytes.length < 512) return false;
  const magic = latin1(bytes, 257, 263);
  if (magic === "ustar") return true;
  const sizeField = latin1(bytes, 124, 136).trim();
  if (!/^[0-7]+$/.test(sizeField)) return false;
  const name = latin1(bytes, 0, 100);
  return name.length > 0 && /^[\x20-\x7e./_-]+$/.test(name);
}

function tarEntries(bytes: Uint8Array, limit = 400): TarEntry[] {
  const entries: TarEntry[] = [];
  let offset = 0;
  while (offset + 512 <= bytes.length && entries.length < limit) {
    const header = bytes.subarray(offset, offset + 512);
    let zero = true;
    for (let i = 0; i < 512; i++) {
      if (header[i] !== 0) {
        zero = false;
        break;
      }
    }
    if (zero) break;

    const name = latin1(header, 0, 100);
    const prefix = latin1(header, 345, 500);
    const full = prefix ? `${prefix}/${name}` : name;
    const size = parseOctal(latin1(header, 124, 136));
    const dataOffset = offset + 512;
    if (full) entries.push({ name: full.replace(/^\.\//, ""), size, offset: dataOffset });
    const padded = Math.ceil(Math.max(0, size) / 512) * 512;
    offset = dataOffset + padded;
  }
  return entries;
}

function entryBytes(bytes: Uint8Array, entry: TarEntry, max = 512 * 1024): Uint8Array | null {
  if (entry.size <= 0 || entry.size > max) return null;
  if (entry.offset + entry.size > bytes.length) return null;
  return bytes.subarray(entry.offset, entry.offset + entry.size);
}

function parseJson(bytes: Uint8Array | null): unknown {
  if (!bytes || bytes.length === 0) return null;
  try {
    return JSON.parse(new TextDecoder("utf-8").decode(bytes));
  } catch {
    return null;
  }
}

function isDockerManifest(value: unknown): value is Array<Record<string, unknown>> {
  if (!Array.isArray(value) || value.length === 0) return false;
  const first = value[0];
  if (!first || typeof first !== "object") return false;
  return "Layers" in first || "RepoTags" in first || "Config" in first;
}

function isOciLayout(value: unknown): boolean {
  return Boolean(value && typeof value === "object" && "imageLayoutVersion" in value);
}

function isOciIndex(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  const rec = value as { schemaVersion?: unknown; manifests?: unknown };
  return rec.schemaVersion === 2 && Array.isArray(rec.manifests);
}

function basename(path: string): string {
  const parts = path.split("/");
  return parts[parts.length - 1] ?? path;
}

function inspectTar(bytes: Uint8Array): { kind: "docker" | "oci"; archive: ArchiveFacts } | null {
  if (!looksLikeTar(bytes)) return null;
  const entries = tarEntries(bytes);
  if (entries.length === 0) return null;
  const names = entries.map((e) => e.name);
  const byBase = new Map<string, TarEntry>();
  for (const entry of entries) byBase.set(basename(entry.name), entry);

  const layoutEntry = byBase.get("oci-layout");
  const indexEntry = byBase.get("index.json");
  const layout = parseJson(layoutEntry ? entryBytes(bytes, layoutEntry, 4096) : null);
  const index = parseJson(indexEntry ? entryBytes(bytes, indexEntry, 256 * 1024) : null);
  const hasBlobs = names.some((n) => n.startsWith("blobs/") || n.includes("/blobs/"));

  if (isOciLayout(layout) || isOciIndex(index) || (Boolean(indexEntry) && hasBlobs)) {
    const layerCount = names.filter((n) => /blobs\/sha256\//.test(n)).length || undefined;
    return {
      kind: "oci",
      archive: {
        format: "oci",
        entryCount: entries.length,
        names: names.slice(0, 16),
        layerCount,
      },
    };
  }

  const manifestEntry = byBase.get("manifest.json");
  const manifest = parseJson(manifestEntry ? entryBytes(bytes, manifestEntry, 512 * 1024) : null);
  if (isDockerManifest(manifest)) {
    const repoTags = manifest.flatMap((item) =>
      Array.isArray(item.RepoTags) ? item.RepoTags.filter((t): t is string => typeof t === "string") : [],
    );
    const layerCount = manifest.reduce((sum, item) => {
      return sum + (Array.isArray(item.Layers) ? item.Layers.length : 0);
    }, 0);
    let architecture: string | undefined;
    const configName = typeof manifest[0]?.Config === "string" ? manifest[0].Config : "";
    if (configName) {
      const configEntry = entries.find((e) => e.name === configName || basename(e.name) === basename(configName));
      const config = parseJson(configEntry ? entryBytes(bytes, configEntry, 256 * 1024) : null);
      if (config && typeof config === "object" && typeof (config as { architecture?: unknown }).architecture === "string") {
        architecture = (config as { architecture: string }).architecture;
      }
    }
    return {
      kind: "docker",
      archive: {
        format: "docker-save",
        entryCount: entries.length,
        names: names.slice(0, 16),
        repoTags: repoTags.length ? repoTags : undefined,
        architecture,
        layerCount: layerCount || undefined,
      },
    };
  }

  return null;
}

function uint8Buffer(bytes: Uint8Array): ArrayBuffer {
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  return copy.buffer;
}

async function gunzip(bytes: Uint8Array): Promise<Uint8Array | null> {
  try {
    const stream = new Blob([uint8Buffer(bytes)]).stream().pipeThrough(new DecompressionStream("gzip"));
    const buffer = await new Response(stream).arrayBuffer();
    return new Uint8Array(buffer);
  } catch {
    return null;
  }
}

export async function sha256Hex(data: BufferSource): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", data);
  const bytes = new Uint8Array(digest);
  let hex = "";
  for (const b of bytes) hex += b.toString(16).padStart(2, "0");
  return hex;
}

export async function inspectBytes(name: string, bytes: Uint8Array, sha256: string): Promise<InspectOutcome> {
  if (isElf(bytes)) {
    const elf = parseElf(bytes);
    if (!elf) return { ok: false, error: UNSUPPORTED_FORMAT };
    return {
      ok: true,
      kind: "elf",
      name,
      sizeBytes: bytes.byteLength,
      sha256,
      elf,
    };
  }

  let archiveBytes = bytes;
  if (isGzip(bytes)) {
    const unzipped = await gunzip(bytes);
    if (!unzipped) return { ok: false, error: UNSUPPORTED_FORMAT };
    archiveBytes = unzipped;
  }

  const archive = inspectTar(archiveBytes);
  if (archive) {
    return {
      ok: true,
      kind: archive.kind,
      name,
      sizeBytes: bytes.byteLength,
      sha256,
      archive: archive.archive,
    };
  }

  return { ok: false, error: UNSUPPORTED_FORMAT };
}

export async function inspectFile(file: File): Promise<InspectOutcome> {
  try {
    if (file.size > MAX_UPLOAD_BYTES) {
      return { ok: false, error: FILE_TOO_LARGE };
    }
    const buffer = await file.arrayBuffer();
    const sha256 = await sha256Hex(buffer);
    return inspectBytes(file.name, new Uint8Array(buffer), sha256);
  } catch {
    return { ok: false, error: READ_FAILED };
  }
}

function kindSubtitle(outcome: Extract<InspectOutcome, { ok: true }>): string {
  if (outcome.kind === "elf" && outcome.elf) {
    return `ELF${outcome.elf.classBits} ${outcome.elf.endian}-endian ${outcome.elf.machine}`;
  }
  if (outcome.kind === "oci") return "OCI image archive";
  const tag = outcome.archive?.repoTags?.[0];
  return tag ? `Docker save · ${tag}` : "Docker save archive";
}

export function artifactFromInspection(outcome: Extract<InspectOutcome, { ok: true }>): Artifact {
  const subtitle = kindSubtitle(outcome);
  const language =
    outcome.kind === "elf" && outcome.elf
      ? `ELF${outcome.elf.classBits} ${outcome.elf.machine}`
      : outcome.kind === "oci"
        ? "OCI"
        : outcome.archive?.architecture
          ? `Docker · ${outcome.archive.architecture}`
          : "Docker";
  return {
    id: `upload-${outcome.sha256.slice(0, 16)}`,
    name: outcome.name,
    subtitle,
    kind: outcome.kind,
    language,
    tag: outcome.name,
    customer: "Local upload",
    sizeBytes: outcome.sizeBytes,
    source: "upload",
    sha256: outcome.sha256,
    elf: outcome.elf,
    archive: outcome.archive,
  };
}

export async function artifactFromFile(file: File): Promise<
  { ok: true; artifact: Artifact } | { ok: false; error: string }
> {
  const outcome = await inspectFile(file);
  if (!outcome.ok) return { ok: false, error: outcome.error };
  return { ok: true, artifact: artifactFromInspection(outcome) };
}
