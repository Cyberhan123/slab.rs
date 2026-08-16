import { useEffect, useState } from "react";

import { getServerHealth } from "@slab/core";

/**
 * Minimal health probe row: verifies the DI chain end-to-end by asking the
 * shared core usecase whether slab-server is reachable through the injected
 * transport.
 */
export function HealthStatus() {
  const [healthy, setHealthy] = useState<boolean | null>(null);

  useEffect(() => {
    let cancelled = false;
    const probe = () => {
      void getServerHealth().then((health) => {
        if (!cancelled) setHealthy(health.healthy);
      });
    };
    probe();
    const timer = window.setInterval(probe, 10_000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, []);

  return (
    <div className="mb-4 flex items-center gap-2 text-sm">
      <span
        aria-label="server health"
        className={
          healthy === null
            ? "size-2 rounded-full bg-muted-foreground/40"
            : healthy
              ? "size-2 rounded-full bg-emerald-500"
              : "size-2 rounded-full bg-red-500"
        }
      />
      <span className="text-muted-foreground">
        {healthy === null ? "connecting to slab-server…" : healthy ? "slab-server reachable" : "slab-server unreachable"}
      </span>
    </div>
  );
}
