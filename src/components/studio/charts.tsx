import {
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  PolarAngleAxis,
  PolarGrid,
  PolarRadiusAxis,
  Radar,
  RadarChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { formatBytes, formatMs, signedPct } from "@/lib/quench/format";
import type { Headroom, Layer, Metrics } from "@/lib/quench/types";

const tooltipStyle = {
  background: "#131513",
  border: "1px solid color-mix(in oklab, #e8ebe4 12%, transparent)",
  borderRadius: 8,
  fontSize: 12,
  color: "#e8ebe4",
};

export function HeadroomRadar({ headroom }: { headroom: Headroom[] }) {
  const data = headroom.map((h) => ({
    dim: h.label,
    Before: h.before,
    After: h.after,
    SOL: h.sol,
  }));
  return (
    <div className="h-64 w-full min-w-0">
      <ResponsiveContainer>
        <RadarChart data={data} cx="50%" cy="50%" outerRadius="72%">
          <PolarGrid stroke="color-mix(in oklab, #e8ebe4 14%, transparent)" />
          <PolarAngleAxis dataKey="dim" tick={{ fill: "#8d9388", fontSize: 11 }} />
          <PolarRadiusAxis domain={[0, 100]} tick={false} axisLine={false} />
          <Radar dataKey="SOL" stroke="#6a7066" fill="none" strokeDasharray="3 3" />
          <Radar dataKey="Before" stroke="#8d9388" fill="#8d9388" fillOpacity={0.12} />
          <Radar dataKey="After" stroke="#7eae86" fill="#7eae86" fillOpacity={0.28} />
          <Legend wrapperStyle={{ fontSize: 12, color: "#8d9388" }} />
        </RadarChart>
      </ResponsiveContainer>
    </div>
  );
}

function PairBar({
  label,
  before,
  after,
  beforeLabel,
  afterLabel,
  invert,
}: {
  label: string;
  before: number;
  after: number;
  beforeLabel: string;
  afterLabel: string;
  invert?: boolean;
}) {
  const max = Math.max(before, after, 0.0001);
  return (
    <div className="space-y-1.5">
      <div className="flex items-baseline justify-between gap-2">
        <p className="text-xs text-muted">{label}</p>
        <p className="font-mono text-xs tabular-nums text-signal">
          {signedPct(before, after, invert)}
        </p>
      </div>
      <div className="space-y-1">
        <div className="flex items-center gap-2">
          <span className="w-10 shrink-0 text-[10px] text-subtle">Before</span>
          <div className="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-surface-2">
            <div
              className="h-full rounded-full bg-muted"
              style={{ width: `${Math.min(100, (before / max) * 100)}%` }}
            />
          </div>
          <span className="w-16 shrink-0 text-right font-mono text-[11px] tabular-nums text-muted">
            {beforeLabel}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-10 shrink-0 text-[10px] text-subtle">After</span>
          <div className="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-surface-2">
            <div
              className="h-full rounded-full bg-signal"
              style={{ width: `${Math.min(100, (after / max) * 100)}%` }}
            />
          </div>
          <span className="w-16 shrink-0 text-right font-mono text-[11px] tabular-nums text-fg">
            {afterLabel}
          </span>
        </div>
      </div>
    </div>
  );
}

export function BeforeAfterBars({ before, after }: { before: Metrics; after: Metrics }) {
  return (
    <div className="space-y-4">
      <PairBar
        label="Artifact"
        before={before.artifactBytes}
        after={after.artifactBytes}
        beforeLabel={formatBytes(before.artifactBytes)}
        afterLabel={formatBytes(after.artifactBytes)}
      />
      <PairBar
        label="RSS"
        before={before.rssMb}
        after={after.rssMb}
        beforeLabel={`${before.rssMb} MiB`}
        afterLabel={`${after.rssMb} MiB`}
      />
      <PairBar
        label="p99 latency"
        before={before.p99Ms}
        after={after.p99Ms}
        beforeLabel={formatMs(before.p99Ms)}
        afterLabel={formatMs(after.p99Ms)}
      />
      <PairBar
        label="Throughput"
        before={before.throughputRps}
        after={after.throughputRps}
        beforeLabel={`${before.throughputRps.toLocaleString()} rps`}
        afterLabel={`${after.throughputRps.toLocaleString()} rps`}
        invert
      />
      <PairBar
        label="Cold start"
        before={before.coldStartMs}
        after={after.coldStartMs}
        beforeLabel={formatMs(before.coldStartMs)}
        afterLabel={formatMs(after.coldStartMs)}
      />
    </div>
  );
}

export function LayerBars({ layers }: { layers: Layer[] }) {
  const data = layers.map((l) => ({
    name: l.name.length > 18 ? `${l.name.slice(0, 16)}…` : l.name,
    Before: l.beforeMb,
    After: l.afterMb,
  }));
  return (
    <div className="h-72 w-full min-w-0">
      <ResponsiveContainer>
        <BarChart data={data} layout="vertical" margin={{ left: 8, right: 8 }}>
          <CartesianGrid horizontal={false} stroke="color-mix(in oklab, #e8ebe4 10%, transparent)" />
          <XAxis type="number" tick={{ fill: "#8d9388", fontSize: 11 }} axisLine={false} tickLine={false} />
          <YAxis
            type="category"
            dataKey="name"
            width={118}
            tick={{ fill: "#8d9388", fontSize: 10 }}
            axisLine={false}
            tickLine={false}
          />
          <Tooltip contentStyle={tooltipStyle} />
          <Bar dataKey="Before" fill="#6a7066" radius={[0, 4, 4, 0]} barSize={8} />
          <Bar dataKey="After" fill="#7eae86" radius={[0, 4, 4, 0]} barSize={8} />
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
