import { cn } from "@/lib/utils";

export function QuenchMark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 32 32"
      className={cn("size-7", className)}
      aria-hidden="true"
      fill="none"
    >
      <rect width="32" height="32" rx="8" className="fill-surface-2" />
      <path
        d="M16 6.5c-5.2 0-9.5 3.9-9.5 9.2 0 6.2 5.4 10.8 9.5 13.3 4.1-2.5 9.5-7.1 9.5-13.3 0-5.3-4.3-9.2-9.5-9.2Z"
        className="stroke-accent"
        strokeWidth="1.6"
      />
      <path
        d="M16 10.2c-3.2 0-5.8 2.4-5.8 5.5 0 3.8 3.3 6.6 5.8 8.2 2.5-1.6 5.8-4.4 5.8-8.2 0-3.1-2.6-5.5-5.8-5.5Z"
        className="stroke-accent/70"
        strokeWidth="1.2"
      />
      <circle cx="16" cy="15.6" r="2.1" className="fill-accent" />
    </svg>
  );
}

export function Logo({ className, markOnly }: { className?: string; markOnly?: boolean }) {
  return (
    <span className={cn("inline-flex items-center gap-2 text-fg", className)}>
      <QuenchMark />
      {markOnly ? null : (
        <span className="font-display text-lg font-semibold tracking-tight">Quench</span>
      )}
    </span>
  );
}
