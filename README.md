# Quench

<p align="center">
  <img src="assets/quench-studio.gif" alt="Quench Studio labeled demo: privacy switch on, match-engine sample, Analyze to Report, p99 86 µs to 74 µs, Demo data and Modeled result" />
</p>

**Quench is a local Linux x86_64 optimization pilot** that measures a real baseline, applies conservative passes, runs the project’s existing tests and benchmarks, and keeps a candidate only when those gates pass.

[Live Studio](https://harrisontran2801.github.io/fjord-fire-dove-delta/studio/) · [Pilot guide](PILOT.md) · [Pilot checklist](PILOT_CHECKLIST.md) · [Setup](SETUP.md)

[Full intro (WebM)](assets/quench-studio.webm) · [MP4](assets/quench-studio.mp4)

## What the public Studio is

The GitHub Pages Studio is a **labeled demo and inspection workbench**. Sample cards play modeled data. They are not native measurements, not a live optimize, and not a performance certificate.

Native optimize runs only through the local agent on **Linux x86_64**. Windows and macOS are not supported for native optimize. Missing tools are reported as Unavailable — Quench does not invent metrics.

## Demo walkthrough (this preview)

The 46-second clip above is the public Studio UI:

1. Landing
2. Privacy switch **on** by default — *Delete inspection data after checking (recommended)*
3. `match-engine` sample
4. Pipeline from Analyze to Report
5. Modeled p99 **86 µs → 74 µs** (demo data, not a native run)

Captions: *Measure the baseline* · *Apply safe optimization passes* · *Verify correctness* · *Keep only measured improvements* · *Local-first and privacy-aware*

## Native pilots

For a real gated run on Linux x86_64, start with [PILOT.md](PILOT.md) and tick [PILOT_CHECKLIST.md](PILOT_CHECKLIST.md). The bundled sample is `samples/match-engine/quench.yaml`.
