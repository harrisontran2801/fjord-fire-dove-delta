import type {
  Artifact,
  Competitor,
  DimensionId,
  ParetoPref,
  RunResult,
  StageDef,
} from "./types";

export const PIPELINE: StageDef[] = [
  { id: "analyze", label: "Analyze", tool: "Bloaty · Layer inspector", duration: 1400 },
  { id: "headroom", label: "Headroom", tool: "Roofline · SOL gap", duration: 900 },
  { id: "profile", label: "Profile", tool: "perf LBR · eBPF", duration: 2200 },
  { id: "optimize", label: "Optimize", tool: "BOLT · strip · UPX · Zstd", duration: 2400 },
  { id: "rebuild", label: "Rebuild", tool: "containerd", duration: 1100 },
  { id: "verify", label: "Verify", tool: "Supplied test suite", duration: 1800 },
  { id: "benchmark", label: "Benchmark", tool: "Before / After harness", duration: 1400 },
  { id: "prove", label: "Report", tool: "Optimization report · K8s patch", duration: 700 },
];

export const AGENT_PIPELINE: StageDef[] = [
  { id: "analyze", label: "Validate", tool: "quench.yaml", duration: 400 },
  { id: "rebuild", label: "Build", tool: "project build command", duration: 800 },
  { id: "verify", label: "Test", tool: "supplied test suite", duration: 800 },
  { id: "benchmark", label: "Benchmark", tool: "project benchmark", duration: 800 },
  { id: "profile", label: "Profile", tool: "perf if available", duration: 600 },
  { id: "optimize", label: "Transform", tool: "llvm-bolt · strip", duration: 800 },
  { id: "headroom", label: "Retest", tool: "candidate test + bench", duration: 800 },
  { id: "prove", label: "Report", tool: "provenance JSON", duration: 400 },
];

export const INSPECT_PIPELINE: StageDef[] = [
  { id: "analyze", label: "Identify", tool: "Magic bytes · SHA-256", duration: 420 },
  { id: "headroom", label: "Headers", tool: "ELF / archive parser", duration: 480 },
  { id: "profile", label: "Contents", tool: "Parsed from the file", duration: 560 },
  { id: "optimize", label: "Optimize", tool: "Not available in browser", duration: 360 },
  { id: "rebuild", label: "Rebuild", tool: "Not available in browser", duration: 360 },
  { id: "verify", label: "Verify", tool: "Not available in browser", duration: 360 },
  { id: "benchmark", label: "Benchmark", tool: "Not available in browser", duration: 360 },
  { id: "prove", label: "Report", tool: "Inspection summary", duration: 420 },
];

export const DEMO_LABEL = "Demo data";
export const MODELED_LABEL = "Modeled result";
export const NOT_EXECUTED_LABEL = "Not executed on this machine";
export const INSPECTION_ONLY_LABEL = "Inspection only";
export const DEMO_DISCLAIMER =
  "Demo data / modeled result / not executed on this machine. This is a product demonstration, not a live optimization run.";
export const NATIVE_DISCLAIMER =
  "Local optimization report. Not an ISO certificate, third-party certificate, or certified result.";
export const INSPECT_DISCLAIMER =
  "Inspection only. Magic bytes, size, and SHA-256 were verified. No optimizer, benchmark, or test suite was executed. This is not an optimization report and not a certificate.";

export const DIMENSIONS: { id: DimensionId; label: string; blurb: string }[] = [
  {
    id: "runtime",
    label: "Runtime",
    blurb: "CPU layout, instruction cache, p99 latency, throughput. LLVM BOLT after profile.",
  },
  {
    id: "artifact",
    label: "Artifact",
    blurb: "ELF/Docker bloat, unused layers, symbols, compression. Bloaty, strip, UPX, Zstd.",
  },
  {
    id: "resource",
    label: "Resource",
    blurb: "RSS, CPU cores, OOM headroom. Modeled in demo samples; measured only by the native agent.",
  },
  {
    id: "infra",
    label: "Infrastructure",
    blurb: "K8s requests/limits, cold start, registry cost, Software Carbon Intensity.",
  },
];

export const SAMPLES: Artifact[] = [
  {
    id: "orders-api",
    name: "orders-api",
    subtitle: "Go checkout service · fat Debian image",
    kind: "docker",
    language: "Go 1.22",
    tag: "ghcr.io/acme/orders-api:1.8.2",
    customer: "Platform / DevOps",
    sizeBytes: 851_443_712,
    sampleId: "orders-api",
    source: "sample",
  },
  {
    id: "vibe-shop",
    name: "vibe-shop",
    subtitle: "Next.js storefront · AI-generated bloat",
    kind: "docker",
    language: "Node 22",
    tag: "vibe-shop:cursor-nightly",
    customer: "Indie / Vibe coding",
    sizeBytes: 1_525_219_328,
    sampleId: "vibe-shop",
    source: "sample",
  },
  {
    id: "match-engine",
    name: "match-engine",
    subtitle: "Rust matching engine · x86_64 ELF",
    kind: "elf",
    language: "Rust 1.81",
    tag: "./target/release/match-engine",
    customer: "HFT / Systems",
    sizeBytes: 29_360_128,
    sampleId: "match-engine",
    source: "sample",
  },
  {
    id: "sci-ledger",
    name: "sci-ledger",
    subtitle: "Spring ledger · over-provisioned fleet",
    kind: "docker",
    language: "Java 21",
    tag: "registry.corp/ledger:2026.3",
    customer: "ESG / Enterprise",
    sizeBytes: 1_181_057_024,
    sampleId: "sci-ledger",
    source: "sample",
  },
];

export const COMPETITORS: Competitor[] = [
  {
    name: "Quench",
    org: "Local-first",
    year: "2026",
    focus: "Closed-loop 4-dimension optimize + verify + local report",
    localCloud: "Local GUI / CLI",
    auto: "Automatic",
    dimensions: ["runtime", "artifact", "resource", "infra"],
    verify: true,
    benchmark: true,
    cert: true,
  },
  {
    name: "Slim.ai",
    org: "Slim.AI",
    year: "2020",
    focus: "Container slimming and attack-surface reduction",
    localCloud: "Cloud SaaS + CLI",
    auto: "Automatic",
    dimensions: ["artifact"],
    verify: false,
    benchmark: true,
    cert: false,
  },
  {
    name: "LLVM BOLT",
    org: "Meta / LLVM",
    year: "2018",
    focus: "Post-link binary layout from LBR profiles",
    localCloud: "Local CLI",
    auto: "Automatic",
    dimensions: ["runtime"],
    verify: false,
    benchmark: false,
    cert: false,
  },
  {
    name: "CAST AI",
    org: "CAST AI",
    year: "2019",
    focus: "K8s autoscaling, rightsizing, bin-packing",
    localCloud: "Cloud SaaS",
    auto: "Automatic",
    dimensions: ["resource", "infra"],
    verify: false,
    benchmark: true,
    cert: false,
  },
  {
    name: "UPX",
    org: "UPX Team",
    year: "1996",
    focus: "Static executable compression",
    localCloud: "Local CLI",
    auto: "Automatic",
    dimensions: ["artifact"],
    verify: false,
    benchmark: false,
    cert: false,
  },
  {
    name: "Intel Advisor",
    org: "Intel",
    year: "2011",
    focus: "Roofline analysis and vectorization advice",
    localCloud: "Local GUI / CLI",
    auto: "Analysis only",
    dimensions: ["runtime"],
    verify: false,
    benchmark: true,
    cert: false,
  },
  {
    name: "AwareCompiler",
    org: "Research",
    year: "2025",
    focus: "LLM agent for compiler pass scheduling",
    localCloud: "Local / Cloud",
    auto: "Automatic",
    dimensions: ["runtime"],
    verify: false,
    benchmark: true,
    cert: false,
  },
];

export const PREF_COPY: Record<ParetoPref, { label: string; hint: string }> = {
  size: {
    label: "Smallest",
    hint: "Max compression. UPX on. Smaller disk, slower cold start.",
  },
  balanced: {
    label: "Balanced",
    hint: "Strip + BOLT + slim. The default Pareto point.",
  },
  speed: {
    label: "Fastest",
    hint: "BOLT + PGO, keep uncompressed. Best latency, larger artifact.",
  },
};

type Template = Omit<RunResult, "after" | "certId" | "modeled"> & {
  after: Record<ParetoPref, RunResult["before"]>;
};

const ORDERS: Template = {
  before: {
    artifactBytes: 851_443_712,
    layers: 14,
    rssMb: 186,
    cpuCores: 0.62,
    p99Ms: 42,
    throughputRps: 1840,
    coldStartMs: 4800,
    vulns: 38,
    k8sCpu: "500m",
    k8sMem: "512Mi",
    sci: 12.4,
    monthlyUsd: 1240,
  },
  after: {
    size: {
      artifactBytes: 96_468_992,
      layers: 4,
      rssMb: 118,
      cpuCores: 0.58,
      p99Ms: 44,
      throughputRps: 1910,
      coldStartMs: 1640,
      vulns: 3,
      k8sCpu: "300m",
      k8sMem: "192Mi",
      sci: 3.8,
      monthlyUsd: 410,
    },
    balanced: {
      artifactBytes: 123_731_968,
      layers: 5,
      rssMb: 94,
      cpuCores: 0.53,
      p99Ms: 36,
      throughputRps: 2112,
      coldStartMs: 1100,
      vulns: 4,
      k8sCpu: "250m",
      k8sMem: "160Mi",
      sci: 4.1,
      monthlyUsd: 470,
    },
    speed: {
      artifactBytes: 188_743_680,
      layers: 6,
      rssMb: 102,
      cpuCores: 0.51,
      p99Ms: 31,
      throughputRps: 2280,
      coldStartMs: 920,
      vulns: 6,
      k8sCpu: "250m",
      k8sMem: "176Mi",
      sci: 4.6,
      monthlyUsd: 540,
    },
  },
  headroom: [
    {
      id: "runtime",
      label: "Runtime",
      before: 61,
      after: 86,
      sol: 93,
      note: "Hot loops scattered across 4 pages. BOLT packs them into L1 i-cache.",
    },
    {
      id: "artifact",
      label: "Artifact",
      before: 22,
      after: 88,
      sol: 94,
      note: "Debug + apt lists + unused libc variants account for 71% of the image.",
    },
    {
      id: "resource",
      label: "Resource",
      before: 48,
      after: 81,
      sol: 90,
      note: "RSS 186 MiB vs 94 MiB after strip; 512Mi request was 2.7× measured p95.",
    },
    {
      id: "infra",
      label: "Infra",
      before: 35,
      after: 84,
      sol: 91,
      note: "Cold start 4.8s blocks HPA. Slim image + rightsized requests recover SLA.",
    },
  ],
  layers: [
    { name: "debian:bookworm-slim", beforeMb: 78, afterMb: 28, action: "distroless static" },
    { name: "ca-certificates", beforeMb: 8, afterMb: 2, action: "kept" },
    { name: "go toolchain leftovers", beforeMb: 90, afterMb: 0, action: "removed" },
    { name: "app binary + dwarf", beforeMb: 64, afterMb: 18, action: "strip + BOLT" },
    { name: "apt lists / cache", beforeMb: 22, afterMb: 0, action: "removed" },
    { name: "unused shared libs", beforeMb: 370, afterMb: 41, action: "closed-world slim" },
    { name: "source + testdata", beforeMb: 180, afterMb: 0, action: "removed" },
  ],
  tests: [
    { name: "go test ./...", detail: "142 passed", durationMs: 3120, status: "pass" },
    { name: "golden HTTP corpus", detail: "28/28 bodies match", durationMs: 880, status: "pass" },
    { name: "response hash equivalence", detail: "sha256 stable", durationMs: 140, status: "pass" },
    { name: "open-port smoke", detail: ":8080 ready", durationMs: 210, status: "pass" },
  ],
  tools: [
    { tool: "Bloaty", action: "ELF section map · 29% debug_info", dimension: "artifact" },
    { tool: "strip --strip-unneeded", action: "Drop DWARF from shipping binary", dimension: "artifact" },
    { tool: "llvm-bolt", action: "Reorder text from 30s LBR profile", dimension: "runtime" },
    { tool: "Docker layer inspector", action: "Delete unused glibc/apt layers", dimension: "artifact" },
    { tool: "K8s patch", action: "requests 500m/512Mi → 250m/160Mi", dimension: "infra" },
  ],
  k8sBefore: `resources:
  requests:
    cpu: "500m"
    memory: "512Mi"
  limits:
    cpu: "1"
    memory: "1Gi"`,
  k8sAfter: `resources:
  requests:
    cpu: "250m"
    memory: "160Mi"
  limits:
    cpu: "500m"
    memory: "256Mi"
# quench: p95 RSS 94Mi + 40% headroom
# cert: QC-PLACEHOLDER`,
  logs: [
    {
      id: "analyze",
      lines: [
        "quench analyze ghcr.io/acme/orders-api:1.8.2",
        "engine: linux/amd64  ELF64  Go 1.22",
        "bloaty  .text 12.4MiB  41%   .rodata 4.1MiB",
        "bloaty  debug_info 8.8MiB  29%  strip candidate",
        "docker  14 layers  812MiB uncompressed",
        "docker  layer 6 unused shared libs  370MiB",
      ],
    },
    {
      id: "headroom",
      lines: [
        "roofline  compute-bound at 61% of SOL on Xeon Ice Lake",
        "headroom  artifact 78%   runtime 32%   rss 49%   infra 65%",
        "pareto    UPX would cut 22MiB more and add ~540ms cold start",
        "recommend balanced (strip + BOLT + distroless)",
      ],
    },
    {
      id: "profile",
      lines: [
        "sandbox  containerd run --cpus 1 --memory 512m  30s",
        "perf     LBR samples  4.8e6  mispredict 2.1%",
        "ebpf    rss p95 186MiB  cpu 0.62  p99 42ms",
        "hot      checkout.(*Service).Reserve  18.4%  3 i-cache misses/call",
      ],
    },
    {
      id: "optimize",
      lines: [
        "strip    --strip-unneeded  −46MiB",
        "llvm-bolt  -reorder-blocks=ext-tsp  packed 214 funcs",
        "repack   distroless/static  5 layers",
        "skip     UPX (balanced) — preserves cold start",
      ],
    },
    {
      id: "rebuild",
      lines: [
        "containerd  commit quench/orders-api:opt  118MiB",
        "digest     sha256:9c1e…a04b",
        "sbom       cyclonedx  41 components  (−390)",
      ],
    },
    {
      id: "verify",
      lines: [
        "sandbox  replay go test ./...  142 passed",
        "equiv    HTTP golden 28/28",
        "equiv    body sha256 match",
        "status   SAMPLE VERIFICATION PASSED",
      ],
    },
    {
      id: "benchmark",
      lines: [
        "bench    wrk  8 threads  30s  same seed",
        "before   1840 rps  p99 42ms  rss 186",
        "after    2112 rps  p99 36ms  rss 94",
        "delta    +14.8% throughput  −49% rss  −85% image",
      ],
    },
    {
      id: "prove",
      lines: [
        "report   QC-PLACEHOLDER",
        "sci      12.4 → 4.1  (−67%)  modeled",
        "patch    example k8s resources, not measured",
        "note     Demo data / modeled result / not executed on this machine",
      ],
    },
  ],
  summary:
    "Fat Debian Go image. Closed-world slim + BOLT recovered 14.8% throughput and 85% disk without breaking tests.",
};

const VIBE: Template = {
  before: {
    artifactBytes: 1_525_219_328,
    layers: 22,
    rssMb: 640,
    cpuCores: 1.4,
    p99Ms: 380,
    throughputRps: 92,
    coldStartMs: 11200,
    vulns: 64,
    k8sCpu: "2",
    k8sMem: "2Gi",
    sci: 28.6,
    monthlyUsd: 890,
  },
  after: {
    size: {
      artifactBytes: 186_646_528,
      layers: 6,
      rssMb: 248,
      cpuCores: 0.72,
      p99Ms: 210,
      throughputRps: 148,
      coldStartMs: 3400,
      vulns: 9,
      k8sCpu: "400m",
      k8sMem: "384Mi",
      sci: 9.2,
      monthlyUsd: 210,
    },
    balanced: {
      artifactBytes: 241_172_480,
      layers: 7,
      rssMb: 214,
      cpuCores: 0.64,
      p99Ms: 168,
      throughputRps: 174,
      coldStartMs: 2600,
      vulns: 8,
      k8sCpu: "350m",
      k8sMem: "320Mi",
      sci: 8.4,
      monthlyUsd: 240,
    },
    speed: {
      artifactBytes: 356_515_840,
      layers: 8,
      rssMb: 226,
      cpuCores: 0.6,
      p99Ms: 142,
      throughputRps: 196,
      coldStartMs: 2100,
      vulns: 11,
      k8sCpu: "350m",
      k8sMem: "352Mi",
      sci: 9.0,
      monthlyUsd: 280,
    },
  },
  headroom: [
    {
      id: "runtime",
      label: "Runtime",
      before: 34,
      after: 72,
      sol: 88,
      note: "Duplicate isomorphic fetch paths from codegen. Dead code still in the graph.",
    },
    {
      id: "artifact",
      label: "Artifact",
      before: 12,
      after: 79,
      sol: 90,
      note: "node_modules copied twice. Playwright, webpack cache, and three UI kits unused at runtime.",
    },
    {
      id: "resource",
      label: "Resource",
      before: 18,
      after: 74,
      sol: 86,
      note: "640 MiB RSS OOMs a 1 GiB VPS. After prune, p95 is 214 MiB.",
    },
    {
      id: "infra",
      label: "Infra",
      before: 20,
      after: 78,
      sol: 88,
      note: "11s cold start makes serverless unusable. Distroless + standalone output fixes HPA.",
    },
  ],
  layers: [
    { name: "node:22-bookworm", beforeMb: 380, afterMb: 0, action: "replaced by distroless" },
    { name: "pnpm store + caches", beforeMb: 290, afterMb: 0, action: "removed" },
    { name: "playwright + browsers", beforeMb: 412, afterMb: 0, action: "dev-only, dropped" },
    { name: "unused UI kits", beforeMb: 96, afterMb: 0, action: "tree-shaken" },
    { name: "next standalone", beforeMb: 84, afterMb: 62, action: "kept" },
    { name: "duplicate node_modules", beforeMb: 188, afterMb: 0, action: "deduped" },
    { name: "app + public", beforeMb: 18, afterMb: 18, action: "kept" },
  ],
  tests: [
    { name: "npm test", detail: "87 passed", durationMs: 6400, status: "pass" },
    { name: "playwright smoke", detail: "12/12", durationMs: 9100, status: "pass" },
    { name: "SSR html hash", detail: "home/pdp/cart match", durationMs: 420, status: "pass" },
    { name: "UPX self-extract", detail: "skipped — native addons", durationMs: 0, status: "skip" },
  ],
  tools: [
    { tool: "Layer inspector", action: "Drop Playwright browsers and pnpm store", dimension: "artifact" },
    { tool: "standalone output", action: "Next.js traced files only", dimension: "artifact" },
    { tool: "strip + Zstd", action: "Compress remaining node native addons", dimension: "artifact" },
    { tool: "K8s patch", action: "2 CPU / 2Gi → 350m / 320Mi", dimension: "infra" },
  ],
  k8sBefore: `resources:
  requests:
    cpu: "2"
    memory: "2Gi"
  limits:
    cpu: "2"
    memory: "2Gi"`,
  k8sAfter: `resources:
  requests:
    cpu: "350m"
    memory: "320Mi"
  limits:
    cpu: "1"
    memory: "512Mi"
# quench: vibe-coded image, closed-world runtime graph`,
  logs: [
    {
      id: "analyze",
      lines: [
        "quench analyze vibe-shop:cursor-nightly",
        "engine: linux/amd64  Node 22  Next 15",
        "warn    duplicated node_modules  188MiB",
        "warn    playwright chromium in production layer",
        "docker  22 layers  1.42GiB",
      ],
    },
    {
      id: "headroom",
      lines: [
        "bloat    81% of image never mapped at runtime",
        "oom      rss 640MiB on a 1Gi VPS — kernel will kill",
        "recommend drop browsers, standalone trace, distroless",
      ],
    },
    {
      id: "profile",
      lines: [
        "sandbox  30s traffic  / /pdp /cart",
        "ebpf    rss p95 640MiB  cpu 1.4  p99 380ms",
        "maps    11% of node_modules pages ever touched",
      ],
    },
    {
      id: "optimize",
      lines: [
        "prune    playwright, three UI kits, webpack cache",
        "next     output: standalone  traced 1,204 files",
        "skip     UPX — native better-sqlite3",
      ],
    },
    {
      id: "rebuild",
      lines: [
        "containerd  commit quench/vibe-shop:opt  230MiB",
        "sbom       64 → 8 high vulns remaining",
      ],
    },
    {
      id: "verify",
      lines: [
        "npm test  87 passed",
        "playwright  12/12  (dev image, not shipping browsers)",
        "status   SAMPLE VERIFICATION PASSED",
      ],
    },
    {
      id: "benchmark",
      lines: [
        "before   92 rps  p99 380ms  rss 640  cold 11.2s",
        "after    174 rps  p99 168ms  rss 214  cold 2.6s",
        "delta    −84% image  −67% rss  no more OOM on 1Gi",
      ],
    },
    {
      id: "prove",
      lines: [
        "report   QC-PLACEHOLDER",
        "sci      28.6 → 8.4",
        "note     typical vibe-coded Next image, 2026",
      ],
    },
  ],
  summary:
    "AI-generated storefront shipping Playwright and duplicate node_modules. 1.42 GB → 230 MB; VPS no longer OOM-kills.",
};

const MATCH: Template = {
  before: {
    artifactBytes: 29_360_128,
    layers: 1,
    rssMb: 48,
    cpuCores: 0.91,
    p99Ms: 0.086,
    throughputRps: 1_240_000,
    coldStartMs: 42,
    vulns: 0,
    k8sCpu: "1",
    k8sMem: "128Mi",
    sci: 3.2,
    monthlyUsd: 2100,
  },
  after: {
    size: {
      artifactBytes: 11_534_336,
      layers: 1,
      rssMb: 61,
      cpuCores: 0.84,
      p99Ms: 0.094,
      throughputRps: 1_180_000,
      coldStartMs: 78,
      vulns: 0,
      k8sCpu: "1",
      k8sMem: "128Mi",
      sci: 3.0,
      monthlyUsd: 1980,
    },
    balanced: {
      artifactBytes: 18_874_368,
      layers: 1,
      rssMb: 46,
      cpuCores: 0.8,
      p99Ms: 0.074,
      throughputRps: 1_390_000,
      coldStartMs: 38,
      vulns: 0,
      k8sCpu: "800m",
      k8sMem: "96Mi",
      sci: 2.7,
      monthlyUsd: 1760,
    },
    speed: {
      artifactBytes: 27_262_976,
      layers: 1,
      rssMb: 47,
      cpuCores: 0.78,
      p99Ms: 0.068,
      throughputRps: 1_470_000,
      coldStartMs: 36,
      vulns: 0,
      k8sCpu: "800m",
      k8sMem: "96Mi",
      sci: 2.6,
      monthlyUsd: 1680,
    },
  },
  headroom: [
    {
      id: "runtime",
      label: "Runtime",
      before: 78,
      after: 92,
      sol: 96,
      note: "Already tight. BOLT still recovers ~12% by packing the match loop into fewer i-cache lines.",
    },
    {
      id: "artifact",
      label: "Artifact",
      before: 71,
      after: 84,
      sol: 90,
      note: "Mostly already stripped. Remaining dwarf and panic paths are optional.",
    },
    {
      id: "resource",
      label: "Resource",
      before: 82,
      after: 90,
      sol: 94,
      note: "RSS already low. Small win from panic=abort and fewer pages.",
    },
    {
      id: "infra",
      label: "Infra",
      before: 70,
      after: 86,
      sol: 92,
      note: "Rightsizing 1 CPU → 800m from measured 0.78 cores with LBR.",
    },
  ],
  layers: [
    { name: "ELF .text", beforeMb: 14.2, afterMb: 13.1, action: "BOLT reorder" },
    { name: "ELF .rodata", beforeMb: 3.4, afterMb: 3.4, action: "kept" },
    { name: "DWARF leftover", beforeMb: 6.8, afterMb: 0.4, action: "strip" },
    { name: "eh_frame / panic", beforeMb: 3.6, afterMb: 1.1, action: "abort unwind" },
  ],
  tests: [
    { name: "cargo test", detail: "214 passed", durationMs: 1820, status: "pass" },
    { name: "replay corpus 1e7 msgs", detail: "bit-identical book", durationMs: 6400, status: "pass" },
    { name: "latency invariant p99<100µs", detail: "held", durationMs: 900, status: "pass" },
    { name: "UPX roundtrip", detail: "rejected — self-check pages", durationMs: 40, status: "warn" },
  ],
  tools: [
    { tool: "perf LBR", action: "30s production-shaped replay", dimension: "runtime" },
    { tool: "llvm-bolt", action: "ext-tsp block layout on match loop", dimension: "runtime" },
    { tool: "strip", action: "Drop leftover DWARF", dimension: "artifact" },
    { tool: "UPX", action: "Rejected — would inflate RSS and p99", dimension: "artifact" },
  ],
  k8sBefore: `resources:
  requests:
    cpu: "1"
    memory: "128Mi"
  limits:
    cpu: "1"
    memory: "128Mi"`,
  k8sAfter: `resources:
  requests:
    cpu: "800m"
    memory: "96Mi"
  limits:
    cpu: "1"
    memory: "128Mi"
# quench: HFT path, BOLT only, UPX refused`,
  logs: [
    {
      id: "analyze",
      lines: [
        "quench analyze ./target/release/match-engine",
        "engine: ELF64 x86_64  rustc 1.81  lto=fat",
        "bloaty  already stripped  dwarf leftover 6.8MiB",
      ],
    },
    {
      id: "headroom",
      lines: [
        "roofline  78% of SOL — remaining gap is i-cache layout",
        "warn      UPX on this binary raises p99 (self-check pages)",
        "recommend speed or balanced, never size",
      ],
    },
    {
      id: "profile",
      lines: [
        "replay    1e7 synthetic messages  30s",
        "perf      LBR  2.1e7 branches  match_one 41% of samples",
      ],
    },
    {
      id: "optimize",
      lines: [
        "llvm-bolt  packed match_one into 2 cache lines (was 7)",
        "strip      leftover dwarf",
        "refuse     UPX — verification would fail latency invariant",
      ],
    },
    {
      id: "rebuild",
      lines: ["write     match-engine.bolt  18MiB", "note      no container, ELF only"],
    },
    {
      id: "verify",
      lines: [
        "cargo test  214 passed",
        "replay      bit-identical order book",
        "warn        size-mode UPX rejected",
      ],
    },
    {
      id: "benchmark",
      lines: [
        "before   1.24M rps  p99 86µs",
        "after    1.39M rps  p99 74µs  (+12.1%)",
      ],
    },
    {
      id: "prove",
      lines: ["cert     QC-PLACEHOLDER", "note     HFT class, runtime-first"],
    },
  ],
  summary:
    "Already-lean Rust binary. BOLT recovers 12% throughput; UPX is refused to protect p99.",
};

const LEDGER: Template = {
  before: {
    artifactBytes: 1_181_057_024,
    layers: 18,
    rssMb: 1220,
    cpuCores: 0.42,
    p99Ms: 96,
    throughputRps: 310,
    coldStartMs: 8600,
    vulns: 51,
    k8sCpu: "4",
    k8sMem: "8Gi",
    sci: 41.0,
    monthlyUsd: 6240,
  },
  after: {
    size: {
      artifactBytes: 268_435_456,
      layers: 6,
      rssMb: 640,
      cpuCores: 0.38,
      p99Ms: 90,
      throughputRps: 340,
      coldStartMs: 4100,
      vulns: 11,
      k8sCpu: "700m",
      k8sMem: "1Gi",
      sci: 11.4,
      monthlyUsd: 1680,
    },
    balanced: {
      artifactBytes: 334_495_744,
      layers: 7,
      rssMb: 580,
      cpuCores: 0.36,
      p99Ms: 78,
      throughputRps: 372,
      coldStartMs: 3200,
      vulns: 10,
      k8sCpu: "600m",
      k8sMem: "896Mi",
      sci: 10.2,
      monthlyUsd: 1520,
    },
    speed: {
      artifactBytes: 419_430_400,
      layers: 8,
      rssMb: 610,
      cpuCores: 0.34,
      p99Ms: 70,
      throughputRps: 398,
      coldStartMs: 2800,
      vulns: 12,
      k8sCpu: "600m",
      k8sMem: "960Mi",
      sci: 10.8,
      monthlyUsd: 1640,
    },
  },
  headroom: [
    {
      id: "runtime",
      label: "Runtime",
      before: 52,
      after: 74,
      sol: 86,
      note: "JIT-friendly layout after AOT. Modest BOLT-class gain on JNI glue.",
    },
    {
      id: "artifact",
      label: "Artifact",
      before: 28,
      after: 76,
      sol: 88,
      note: "JDK + unused locales + docs. jlink custom runtime cuts 700 MB.",
    },
    {
      id: "resource",
      label: "Resource",
      before: 24,
      after: 80,
      sol: 90,
      note: "Measured 0.42 cores / 1.2 GiB vs requested 4 CPU / 8 GiB.",
    },
    {
      id: "infra",
      label: "Infra",
      before: 16,
      after: 85,
      sol: 92,
      note: "Fleet of 40 pods over-provisioned. SCI 41 → 10.2; FinOps reportable.",
    },
  ],
  layers: [
    { name: "eclipse-temurin:21", beforeMb: 420, afterMb: 0, action: "jlink custom" },
    { name: "locales / man / src", beforeMb: 210, afterMb: 0, action: "removed" },
    { name: "fat jar + deps", beforeMb: 340, afterMb: 188, action: "unused code drop" },
    { name: "native libs", beforeMb: 96, afterMb: 64, action: "strip" },
    { name: "app classes", beforeMb: 48, afterMb: 48, action: "kept" },
  ],
  tests: [
    { name: "mvn test", detail: "408 passed", durationMs: 24100, status: "pass" },
    { name: "ledger replay 90d", detail: "checksum match", durationMs: 18800, status: "pass" },
    { name: "SCI sample window", detail: "GSF methodology", durationMs: 400, status: "pass" },
  ],
  tools: [
    { tool: "jlink", action: "Custom JDK, no locales", dimension: "artifact" },
    { tool: "Bloaty", action: "JNI .so section map", dimension: "artifact" },
    { tool: "K8s patch", action: "4 CPU / 8Gi → 600m / 896Mi", dimension: "infra" },
    { tool: "SCI report", action: "GSF Software Carbon Intensity", dimension: "infra" },
  ],
  k8sBefore: `resources:
  requests:
    cpu: "4"
    memory: "8Gi"
  limits:
    cpu: "4"
    memory: "8Gi"`,
  k8sAfter: `resources:
  requests:
    cpu: "600m"
    memory: "896Mi"
  limits:
    cpu: "1"
    memory: "1536Mi"
# quench: p95 RSS 580Mi, p95 CPU 0.36
# sci: 41.0 → 10.2  GSF SCI`,
  logs: [
    {
      id: "analyze",
      lines: [
        "quench analyze registry.corp/ledger:2026.3",
        "engine: linux/amd64  jdk 21  spring 3.4",
        "warn    full temurin + all locales  210MiB",
        "k8s     requests 4 CPU 8Gi  vs rss 1.2Gi cpu 0.42",
      ],
    },
    {
      id: "headroom",
      lines: [
        "finops   40 replicas × over-request = $6.2k/mo waste",
        "sci      41.0 gCO2e/unit  (GSF)",
        "recommend jlink + rightsizing, not UPX on JVM",
      ],
    },
    {
      id: "profile",
      lines: [
        "sandbox  30s  posting + query mix",
        "ebpf    rss p95 1220MiB  cpu 0.42  p99 96ms",
      ],
    },
    {
      id: "optimize",
      lines: [
        "jlink    custom runtime  −700MiB",
        "strip    jni .so",
        "skip     UPX on JVM (startup regression)",
      ],
    },
    {
      id: "rebuild",
      lines: ["containerd  commit quench/sci-ledger:opt  319MiB"],
    },
    {
      id: "verify",
      lines: ["mvn test  408 passed", "replay 90d checksum match"],
    },
    {
      id: "benchmark",
      lines: [
        "before   310 rps  rss 1220  image 1.13Gi",
        "after    372 rps  rss 580   image 319Mi",
        "fleet    modeled $6,240 → $1,520 /mo",
      ],
    },
    {
      id: "prove",
      lines: [
        "report   QC-PLACEHOLDER  GSF SCI (modeled)",
        "sci      41.0 → 10.2  (−75%)",
      ],
    },
  ],
  summary:
    "Enterprise Java ledger requested 4 CPU / 8 GiB, used 0.42 / 1.2. jlink + rightsizing is the ESG story.",
};

export const TEMPLATES: Record<string, Template> = {
  "orders-api": ORDERS,
  "vibe-shop": VIBE,
  "match-engine": MATCH,
  "sci-ledger": LEDGER,
};

export const SAMPLE_BY_ID = Object.fromEntries(SAMPLES.map((s) => [s.id, s]));
