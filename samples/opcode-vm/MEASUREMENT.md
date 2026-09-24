# opcode-vm measurement note

Public technical sample. Not customer evidence, not a commercial proof, and not a certificate.

Measured on one Ubuntu x86_64 host on 2026-09-25 (Asia/Ho_Chi_Minh). The host has 12 logical CPUs. The full JSON reports, SHA-256 values, raw samples, and tool versions stayed on that host and are **not** in this repository. Do not reconstruct them.

Gates were unchanged for every run:

- 15 measured repetitions
- 2 warmup runs excluded
- keep only if tests pass, median improvement is at least 1%, and median and p95 regression stay within 2%
- a spread warning does not change those gates

`samples/match-engine` was also measured the same night. Its LBR run was rejected: median change **-1.831%**, 15 repetitions, `stabilityWarning` null. That rejection still stands.

## Runs

| Run | Profile | CPU governor | Kept | Median change | Spread warning |
|---|---|---|---|---|---|
| 1 | `instrument` (`lbrProbeOk` false; `perf_event_paranoid` blocked LBR) | `powersave` | yes | **+2.922%** | baseline 19.4%, candidate 11.1% |
| 2 | `lbr` (`lbrProbeOk` true, after `perf_event_paranoid=1`) | `powersave` | yes | **+9.295%** | baseline 17.0%, candidate 20.4% |
| 3 | `lbr` (`lbrProbeOk` true) | `performance` | yes | **+14.719%** | baseline 19.8%, candidate 8.1% |

All three opcode-vm runs exited 0 and passed the supplied tests. Baseline median moved from 198 ms to 187 ms to 169 ms across runs, so absolute times are not stable on this host.

### Run 1 — instrumentation fallback

| | Baseline | Candidate |
|---|---:|---:|
| Median | 198.080 ms | 192.292 ms |
| p95 | 210.470 ms | 200.654 ms |
| Min | 174.994 ms | 182.264 ms |
| Max | 213.360 ms | 203.653 ms |
| Spread | 38.366 ms | 21.389 ms |

The ranges overlap. The fastest baseline sample (174.994 ms) beat the fastest candidate sample (182.264 ms). Median gain was about 5.8 ms, smaller than the baseline spread.

### Run 2 — LBR, powersave

| | Baseline | Candidate |
|---|---:|---:|
| Median | 186.998 ms | 169.617 ms |
| p95 | 207.514 ms | 189.015 ms |
| Min | 175.955 ms | 155.530 ms |
| Max | 207.836 ms | 190.181 ms |
| Spread | 31.880 ms | 34.651 ms |

Median gain was about 17.4 ms. The ranges still overlap: the slowest candidate sample (190.181 ms) was slower than the fastest baseline sample (175.955 ms).

### Run 3 — LBR, performance governor

| | Baseline | Candidate |
|---|---:|---:|
| Median | 168.617 ms | 143.798 ms |
| p95 | 188.738 ms | 147.746 ms |
| Min | 155.939 ms | 137.368 ms |
| Max | 189.374 ms | 149.023 ms |
| Spread | 33.434 ms | 11.655 ms |

Median gain was about 24.8 ms (14.719%). This is the only run whose ranges do not overlap: the slowest candidate sample (149.023 ms) was faster than the fastest baseline sample (155.939 ms). Baseline spread was still 19.8% of the median, above the 5% warning line. Candidate spread was 8.1%, also above that line.

## How to read this

- Run 3 is the clearest of the three. It is still a noisy technical sample.
- The candidate was produced with real `perf` LBR and `llvm-bolt`. The instrumented binary was not the benchmark source. Run 1 used BOLT instrumentation only because the LBR probe failed.
- Nothing here authorizes a customer claim, a savings claim, or a statement that Quench makes Linux binaries faster in general.
- `match-engine` on the same host was a measured regression and was rejected.
- Setting the CPU governor to `performance` does not persist across reboot. `perf_event_paranoid=1` does not persist either.
