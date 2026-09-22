import type { ReactNode } from "react";
import { Link } from "@tanstack/react-router";
import { Logo } from "@/components/brand/logo";
import { cn } from "@/lib/utils";

const LINKS = [
  { to: "/studio", label: "Studio" },
  { to: "/research", label: "Research" },
] as const;

export function Shell({
  children,
  wide,
}: {
  children: ReactNode;
  wide?: boolean;
}) {
  return (
    <div className="min-h-dvh bg-bg text-fg">
      <header className="sticky top-0 z-30 border-b border-border bg-bg/85 backdrop-blur-md">
        <div
          className={cn(
            "mx-auto flex h-14 items-center justify-between gap-4 px-4 sm:px-6",
            wide ? "max-w-7xl" : "max-w-6xl",
          )}
        >
          <Link to="/" className="inline-flex min-h-11 min-w-11 shrink-0 items-center">
            <Logo />
          </Link>
          <nav className="flex items-center gap-1">
            {LINKS.map((l) => (
              <Link
                key={l.to}
                to={l.to}
                className="inline-flex h-11 items-center rounded-md px-3 text-sm text-muted transition-colors duration-150 hover:text-fg"
                activeProps={{ className: "text-fg" }}
              >
                {l.label}
              </Link>
            ))}
          </nav>
        </div>
      </header>
      {children}
    </div>
  );
}

export function Footer() {
  return (
    <footer className="border-t border-border">
      <div className="mx-auto flex max-w-6xl flex-col gap-2 px-4 py-8 text-xs text-subtle sm:flex-row sm:items-center sm:justify-between sm:px-6">
        <p>Quench · local-first optimization · binaries never leave this machine</p>
        <p>Free local studio · CI minutes for teams</p>
      </div>
    </footer>
  );
}
