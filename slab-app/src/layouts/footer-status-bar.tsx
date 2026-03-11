import { useEffect, useMemo, useState } from "react";
import { Cpu, Gauge, MemoryStick } from "lucide-react";

import { getApiUrl } from "@/lib/tauri-api";

type GpuDeviceStatus = {
  id: number;
  name: string;
  device_type: string;
  utilization_percent: number;
  temperature_celsius: number;
  used_memory_bytes: number;
  total_memory_bytes: number;
  memory_usage_percent: number;
  power_draw_watts: number;
};

type GpuStatusResponse = {
  available: boolean;
  backend: string;
  updated_at: string;
  devices: GpuDeviceStatus[];
  error?: string | null;
};

const POLL_INTERVAL_MS = 5000;

function formatGiB(bytes: number): string {
  const gib = bytes / (1024 ** 3);
  return `${gib.toFixed(gib >= 10 ? 1 : 2)} GB`;
}

async function fetchGpuStatus(): Promise<GpuStatusResponse> {
  const apiUrl = await getApiUrl();
  const normalized = apiUrl.endsWith("/") ? apiUrl.slice(0, -1) : apiUrl;
  const response = await fetch(`${normalized}/v1/system/gpu`);
  if (!response.ok) {
    throw new Error(`GPU status request failed: ${response.status}`);
  }
  return (await response.json()) as GpuStatusResponse;
}

export default function FooterStatusBar() {
  const [snapshot, setSnapshot] = useState<GpuStatusResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let mounted = true;
    let inflight = false;

    const refresh = async () => {
      if (inflight) {
        return;
      }
      inflight = true;
      try {
        const next = await fetchGpuStatus();
        if (mounted) {
          setSnapshot(next);
          setIsLoading(false);
        }
      } catch (error) {
        if (mounted) {
          setSnapshot({
            available: false,
            backend: "all-smi",
            updated_at: new Date().toISOString(),
            devices: [],
            error: error instanceof Error ? error.message : String(error),
          });
          setIsLoading(false);
        }
      } finally {
        inflight = false;
      }
    };

    void refresh();
    const timer = window.setInterval(() => {
      void refresh();
    }, POLL_INTERVAL_MS);

    return () => {
      mounted = false;
      clearInterval(timer);
    };
  }, []);

  const summary = useMemo(() => {
    const devices = snapshot?.devices ?? [];
    if (devices.length === 0) {
      return {
        available: Boolean(snapshot?.available),
        model: "No GPU",
        memory: "-- / --",
        utilization: "--",
      };
    }

    const first = devices[0];
    const totalUsed = devices.reduce((sum, item) => sum + item.used_memory_bytes, 0);
    const totalMemory = devices.reduce((sum, item) => sum + item.total_memory_bytes, 0);
    const model = devices.length === 1 ? first.name : `${first.name} +${devices.length - 1}`;
    const memory = `${formatGiB(totalUsed)} / ${formatGiB(totalMemory)}`;
    const utilization = `${first.utilization_percent.toFixed(0)}%`;

    return {
      available: Boolean(snapshot?.available),
      model,
      memory,
      utilization,
    };
  }, [snapshot]);

  const dotClass = isLoading
    ? "bg-amber-400 animate-pulse"
    : summary.available
      ? "bg-emerald-500"
      : "bg-rose-500";

  return (
    <footer className="h-7 border-t bg-muted/55 px-3 text-[11px] text-muted-foreground">
      <div className="flex h-full items-center gap-3 overflow-hidden">
        <span className="inline-flex items-center gap-1.5 shrink-0">
          <span className={`size-1.5 rounded-full ${dotClass}`} />
          <Cpu className="size-3" />
          <span className="font-medium">GPU</span>
        </span>

        <span className="truncate" title={summary.model}>
          {isLoading ? "Detecting..." : summary.model}
        </span>

        <span className="hidden items-center gap-1 tabular-nums sm:inline-flex">
          <MemoryStick className="size-3" />
          {summary.memory}
        </span>

        <span className="hidden items-center gap-1 tabular-nums md:inline-flex">
          <Gauge className="size-3" />
          {summary.utilization}
        </span>

        <span className="ml-auto shrink-0 text-[10px] uppercase tracking-wide text-muted-foreground/80">
          {snapshot?.backend ?? "all-smi"}
        </span>
      </div>
    </footer>
  );
}
