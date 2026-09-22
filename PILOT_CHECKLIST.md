# Pilot acceptance checklist

Use this on the Linux x86_64 pilot host. Do not tick a box from demo Studio cards. Demo results are modeled; native results come from `quench-agent optimize`.

## 0. Host

- [ ] `uname -m` is `x86_64` and the OS is Linux
- [ ] `./agent/target/release/quench-agent doctor` lists platform OK
- [ ] `rustc` / `cargo` reported OK if the project builds with Cargo
- [ ] Missing `perf` / `llvm-bolt` / `strip` are listed **Unavailable**, not assumed present

## 1. Bundled sample

```sh
export QUENCH_WORKSPACE="$PWD"
./agent/target/release/quench-agent preflight --config samples/match-engine/quench.yaml
./agent/target/release/quench-agent optimize --config samples/match-engine/quench.yaml
```

- [ ] Preflight is ready (blocking issues empty). Warnings for perf/llvm-bolt are OK
- [ ] Optimize actually ran `cargo build --release`, `./scripts/test.sh`, `./scripts/workload.sh`, `./scripts/bench.sh`
- [ ] Report includes **baseline SHA-256** and **candidate SHA-256** (or candidate `-` if none produced)
- [ ] Report includes `min_improvement_percent` and `max_regression_percent`
- [ ] Report includes measured `medianMs` / `p95Ms` from the bench command (not demo 86 µs cards)
- [ ] Kept vs rejected matches the gates. A rejected/regressed candidate is **failed**, not complete
- [ ] Studio **Run native sample** shows `quench-agent · native pipeline`, not “Demo data”

## 2. Customer workspace config

Point `quench.yaml` at the customer project (inside `QUENCH_WORKSPACE`):

```sh
./agent/target/release/quench-agent preflight --config path/to/quench.yaml
./agent/target/release/quench-agent optimize --config path/to/quench.yaml
```

- [ ] Preflight fails honestly if the file is missing, YAML is invalid, or `build` / `test` / `benchmark` are missing
- [ ] Preflight fails if a required script (e.g. `./scripts/bench.sh`) is not on disk
- [ ] `configPath` outside the workspace is rejected (HTTP 403)
- [ ] Optimize uses the customer `project` name, not a hard-coded match-engine card
- [ ] Build, test, workload/profile, and benchmark commands in the report match the customer yaml
- [ ] Baseline and candidate SHA-256 (when a candidate exists) are recorded
- [ ] Candidate **kept** only if tests passed **and** improvement/regression gates passed
- [ ] Candidate **rejected/failed** is shown as failed in CLI and Studio
- [ ] Successful native keep is labeled a local optimization report, **not** Demo data / Modeled result

## 3. Failure honesty

- [ ] `preflight --config /tmp/does-not-exist.yaml` exits non-zero and does not invent a report
- [ ] Missing cargo/rustc on a Cargo project is BLOCKING, not a fake pass
- [ ] Failed supplied tests → `verification_failed`, candidate not kept
- [ ] Inspection of a dropped file does not start optimize and does not fabricate benchmarks
- [ ] Studio inspection privacy toggle still defaults ON; inspection history can be deleted

## 4. Out of scope (must remain false)

- [ ] No claim of Windows/macOS native support
- [ ] No claim of cloud execution
- [ ] No claim of certified / ISO performance
- [ ] No billing or multi-tenant SaaS behavior was required to complete this pilot
