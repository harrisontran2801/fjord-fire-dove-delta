# Quench setup

Two processes: the web studio, and an optional native agent.

## Web studio

```sh
npm run dev
```

The UI always loads. If the agent is down, Studio shows **Local agent not connected** and stays in Demo / Inspection mode. It does not invent a live pipeline.

## Native agent (Linux x86_64)

Requires Rust (`cargo`, `rustc`). If they are missing, skip this section — the UI will show an unavailable state.

```sh
export QUENCH_WORKSPACE=/path/to/this/repo
export QUENCH_SAMPLE_CONFIG=samples/match-engine/quench.yaml   # optional, relative to the workspace
npm run agent:build
npm run agent:serve
# or:  ./agent/target/release/quench-agent serve --bind 127.0.0.1:4783
```

The agent binds **loopback only** (`127.0.0.1:4783`) and a unix socket (`/tmp/quench-agent.sock`). Browser requests from unknown origins are rejected. `configPath` must resolve to a file inside `QUENCH_WORKSPACE`.

```sh
npm run agent:doctor
quench-agent optimize --config samples/match-engine/quench.yaml
```

The match-engine sample stays at `samples/match-engine/`. Point `QUENCH_SAMPLE_CONFIG` at another in-workspace `quench.yaml` if you want a different native sample.

## Uploads

Dropping an ELF / Docker save / OCI archive identifies the file (browser fallback or native inspect). That path is **inspection-only**. It does not start a demo optimizer and does not fabricate benchmarks. To optimize, provide a real `quench.yaml` in the workspace and run the agent.

## Honest limitations

- `perf` and `llvm-bolt` are reported **Unavailable** when they are not on PATH.
- Docker/OCI rewrite is not implemented (inspect and recommendations only).
- Local reports are not ISO certificates or certified results.
