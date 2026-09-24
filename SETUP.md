# Quench setup

Two processes: the web studio, and an optional native agent.

**Paid Linux x86_64 pilots:** start with [PILOT.md](PILOT.md) and tick [PILOT_CHECKLIST.md](PILOT_CHECKLIST.md). This repo is not a hosted SaaS.

## Web studio

```sh
npm run dev
```

The UI always loads. If the agent is down, Studio shows **Local agent not connected** and stays in Demo / Inspection mode. It does not invent a live pipeline.

Startup scripts resolve the repository root from their own location. They do not assume `/workspace`. Override with:

| Variable | Default |
| --- | --- |
| `QUENCH_WORKSPACE` | directory that contains `startup.sh` |
| `QUENCH_SAMPLE_CONFIG` | `samples/match-engine/quench.yaml` (relative to the workspace) |
| `QUENCH_AGENT_BIN` | `$ROOT/agent/target/release/quench-agent` |
| `QUENCH_AGENT_SOCKET` | `/tmp/quench-agent.sock` |
| `QUENCH_AGENT_BIND` | `127.0.0.1:4783` (loopback only) |
| `VITE_QUENCH_AGENT_URL` | `http://127.0.0.1:4783` for direct browser fallback |

## Native agent — Linux x86_64 only

Native optimize **does not run on Windows or macOS**. On those hosts the Studio UI still works in Demo / Inspection mode. `quench-agent doctor` reports the platform as Unavailable and `optimize` refuses to start.

### Linux prerequisites

Required for `samples/match-engine/quench.yaml`:

- Linux x86_64
- `rustc` and `cargo` (Rust toolchain)
- GNU `strip` (`binutils`)
- POSIX `sh`

Optional. Missing tools are recorded as **Unavailable**; the agent does not invent native metrics for them:

- `perf` (LBR profile: `perf record -e cycles:u -j any,u`). Preferred when the VM/host exposes branch-stack sampling.
- `llvm-bolt` and `libbolt_rt_instr.a` (layout rewrite). If the LBR probe fails, Quench instruments a **copy**, runs the real profile workload against that copy, then BOLTs the original uninstrumented binary.
- `clang` (not used by the match-engine sample)
- `docker` / `containerd` (Docker/OCI rewrite is not implemented)

The instrumented copy is slower and is **never** the source of final median/p95 numbers. Instrumentation is not production-traffic sampling. GNU `strip` is skipped after `llvm-bolt` because it can break BOLT section layout. A fallback profile can still reject the candidate when gates fail. Do not treat a synthetic sample keep as commercial proof.

Benchmarks use the same command for the baseline and the candidate. The default is **15 measured repetitions after 2 warmup runs**. Warmup timings are excluded from median, p95, min, max, and spread. Gates stay at 1% minimum median improvement and 2% maximum median or p95 regression. A wide spread is a stability warning only; it does not keep or reject a candidate by itself. Override with `benchmark_repetitions` / `benchmark_warmup` in `quench.yaml` (1–30 and 0–10) or `QUENCH_BENCH_REPETITIONS` / `QUENCH_BENCH_WARMUP`.

### Native run (match-engine)

From the repository root:

```sh
export QUENCH_WORKSPACE="$PWD"
export QUENCH_SAMPLE_CONFIG=samples/match-engine/quench.yaml   # optional; this is the default
npm run agent:build
npm run agent:doctor
./agent/target/release/quench-agent preflight --config samples/match-engine/quench.yaml
./agent/target/release/quench-agent optimize --config samples/match-engine/quench.yaml
```

That command builds the sample with `cargo build --release` and `-Wl,--emit-relocs`, runs `./scripts/test.sh` (`cargo test` plus the binary `--self-test`), profiles with `./scripts/workload.sh` (LBR if the probe passes, otherwise BOLT instrumentation on a copy), and benches with `./scripts/bench.sh`. `perf` / `llvm-bolt` / `libbolt_rt_instr.a` are skipped with **Unavailable** or **No profile** when they are not usable. `strip` is applied when present. The report includes `profileMode`, baseline and candidate identity, the real commands, tool availability, measured median improvement, `min_improvement_percent`, `max_regression_percent`, and whether the candidate was kept or rejected. Without a usable profile, a strip-only candidate is often **rejected** when median improvement is below `min_improvement_percent` (default 1%). That is the gate working — size-only change is not invented as a runtime win.

Serve the agent so Studio can run the same sample:

```sh
npm run agent:serve
# or:  ./agent/target/release/quench-agent serve --bind 127.0.0.1:4783
```

Then open Studio and use **Run native sample**. Studio labels the run from the native report (`project`, artifact path, kind, size, hashes), not from a modeled demo card.

When using a non-default bind, set `QUENCH_AGENT_BIND` for the Vite proxy and set `VITE_QUENCH_AGENT_URL` to the same HTTP address if direct browser fallback is needed.

## Local agent security

The agent binds **loopback only** (`127.0.0.1:4783`) and a unix socket (`QUENCH_AGENT_SOCKET`, default `/tmp/quench-agent.sock`) with owner-only permissions (`0600`). Browser requests from unknown origins are rejected. `configPath` and inspect paths must resolve to a file inside `QUENCH_WORKSPACE`. CORS is never `*`.

Origin allowlisting and CORS are **not authentication**. TCP mutation endpoints (`POST /v1/optimize`, `POST /v1/inspect`) stay reachable without credentials on loopback so the local CLI and the Vite unix-socket proxy can call them. Any process on this machine can still POST to `127.0.0.1:4783` if it omits `Origin` (or sends an allowlisted one). Do not expose the agent beyond loopback.

## Uploads

Dropping an ELF / Docker save / OCI archive identifies the file (browser fallback or native inspect). That path is **inspection-only**. It does not start a demo optimizer and does not fabricate benchmarks. To optimize, provide a real `quench.yaml` in the workspace and run the agent.

Inspection bytes are processed in memory. The native inspection endpoint writes a temporary upload, inspects it, and deletes that file before responding. Studio does **not** persist uploaded bytes in browser storage.

## Inspection privacy

Studio’s **Delete inspection data after checking (recommended)** control defaults to **ON**.

When ON:

- Inspection metadata and reports are not written to browser history (`localStorage`).
- Enabling the setting removes any existing inspection runs from the current history immediately.
- Reloading or rehydrating older local state filters legacy inspection runs out of history.

When OFF, inspection history may be retained locally alongside demo and native agent runs.

**Clear inspection data** still removes inspection runs at any time, and each report keeps its per-run delete action. Demo and native agent runs are not affected by the inspection privacy setting.

## Honest limitations

- `perf` and `llvm-bolt` are reported **Unavailable** when they are not on PATH.
- Docker/OCI rewrite is not implemented (inspect and recommendations only).
- Local reports are not ISO certificates or certified results.
- Windows/macOS: no native optimize. Use Demo / Inspection in Studio.
