# Quench Linux x86_64 pilot

Quench is a **local** optimization agent for **Linux x86_64**. This is not a cloud product, not a SaaS, and not a certified benchmark lab. Demo cards in Studio are modeled sample data. Native optimize only reports what this machine actually ran.

Windows and macOS: native optimize is **not supported**. Studio still opens in Demo / Inspection mode.

## Prerequisites

Required on the pilot host:

- Linux x86_64
- Rust (`rustc`, `cargo`) if the project `build` command uses Cargo
- GNU `strip` (binutils) if you want the strip transform
- POSIX `sh`
- A project with a real `quench.yaml` (see below)

Optional. Missing tools are recorded as **Unavailable**. Quench does **not** invent profile, BOLT, or benchmark numbers for them:

- `perf` — LBR profile
- `llvm-bolt` — layout rewrite
- `clang` — unused by the bundled sample
- Docker / containerd — image rewrite is **not implemented**

## Install and start

From the repository root on the Linux host:

```sh
export QUENCH_WORKSPACE="$PWD"
npm run agent:build
./agent/target/release/quench-agent doctor
./agent/target/release/quench-agent preflight --config samples/match-engine/quench.yaml
```

`doctor` inventories the host. `preflight --config` also checks that the named `quench.yaml` is valid and that build / test / benchmark / profile commands resolve. Blocking issues fail the command. Optional tools (perf, llvm-bolt) are warnings.

Serve the agent (loopback only) if you will use Studio:

```sh
npm run agent:serve
npm run dev          # Studio at http://127.0.0.1:8080
```

The agent binds `127.0.0.1:4783` and a unix socket. It does not upload binaries.

## Provide a real `quench.yaml`

Copy `quench.yaml.example` next to the crate or binary you want optimized. ELF optimize requires:

```yaml
project: your-binary
kind: elf
binary: ./target/release/your-binary
build: cargo build --release
test: ./scripts/test.sh          # must run your tests; do not skip
benchmark: ./scripts/bench.sh    # must print elapsed_ms=… or QUENCH_BENCH JSON
profile: ./scripts/workload.sh   # optional workload for perf
max_regression_percent: 2
min_improvement_percent: 1
```

`build`, `test`, and `benchmark` are required. Native optimize will not invent a build, will not claim tests passed without running them, and will not fabricate a benchmark. `profile` is optional; without a usable profile, `llvm-bolt` is skipped.

The HTTP API only accepts a `configPath` **inside** `QUENCH_WORKSPACE`. Absolute paths outside the workspace and `..` escapes are rejected.

## Commands

```sh
# Host + toolchain
./agent/target/release/quench-agent doctor --json

# Config + commands + tools (same as doctor --config)
./agent/target/release/quench-agent preflight --config path/to/quench.yaml

# Identify an ELF / Docker save / OCI archive (inspection only)
./agent/target/release/quench-agent inspect path/to/binary

# Real optimize (build, test, profile, bench, gated keep/reject)
./agent/target/release/quench-agent optimize --config path/to/quench.yaml

# Print a stored run
./agent/target/release/quench-agent report <run-id>
```

Inspect never starts an optimizer and never invents benchmarks.

## Expected outputs

A native report always includes:

- baseline and candidate identity (SHA-256, paths, sizes)
- the actual `build` / `test` / `profile` / `benchmark` commands
- tool OK / Unavailable (rustc, cargo, strip, perf, llvm-bolt, …)
- measured median (and p95) from the benchmark command
- `min_improvement_percent` and `max_regression_percent`
- **kept** or **rejected**

A candidate is **kept** only when:

1. the supplied test suite passes on the candidate, and
2. median improvement meets `min_improvement_percent`, and
3. median and p95 do not exceed `max_regression_percent`.

Otherwise the run is **failed** or **verification_failed**. Studio shows those as failures, not “complete”. Size-only changes without a runtime win are rejected. Missing `llvm-bolt` / `perf` are Unavailable, not fake speedups.

Without `llvm-bolt`, a strip-only candidate is often rejected on the match-engine sample. That is the gate working.

## Data that stays local

- Binaries, configs, logs, and reports stay on this machine (`~/.quench` by default).
- The agent never uploads artifacts.
- Studio inspection of dropped files is memory-only. Uploaded bytes are not written to `localStorage`.
- Native inspect writes a temporary file, hashes it, then deletes it before responding.

### Inspection data deletion

In Studio, **Delete inspection data after checking (recommended)** defaults to **ON**:

- Inspection runs are not persisted in browser history.
- Turning the setting ON deletes existing inspection runs immediately.
- Reloading filters legacy inspection runs out of `localStorage`.

**Clear inspection data** and per-report delete remain available. Demo and native agent runs are not removed by the inspection privacy toggle.

## Honest limitations

- Linux x86_64 only. No Windows/macOS native optimize.
- No cloud execution.
- No ISO / third-party / certified performance claims.
- Demo workloads in Studio are labeled demo data and are not native results.
- Docker/OCI rewrite is not implemented.
- First paid pilots are hands-on local runs against a customer `quench.yaml`, not a hosted pipeline.
