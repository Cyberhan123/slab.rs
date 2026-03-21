import { useEffect, useMemo, useState } from "react"
import { Cpu, Gauge, MemoryStick } from "lucide-react"

import { StatusPill } from "@/components/ui/workspace"
import { getApiUrl } from "@/lib/tauri-api"

type GpuDeviceStatus = {
  id: number
  name: string
  device_type: string
  utilization_percent: number
  temperature_celsius: number
  used_memory_bytes: number
  total_memory_bytes: number
  memory_usage_percent: number
  power_draw_watts: number
}

type GpuStatusResponse = {
  available: boolean
  backend: string
  updated_at: string
  devices: GpuDeviceStatus[]
  error?: string | null
}

const POLL_INTERVAL_MS = 5000

function formatGiB(bytes: number): string {
  const gib = bytes / 1024 ** 3
  return `${gib.toFixed(gib >= 10 ? 1 : 2)} GB`
}

async function fetchGpuStatus(): Promise<GpuStatusResponse> {
  const apiUrl = await getApiUrl()
  const normalized = apiUrl.endsWith("/") ? apiUrl.slice(0, -1) : apiUrl
  const response = await fetch(`${normalized}/v1/system/gpu`)

  if (!response.ok) {
    throw new Error(`GPU status request failed: ${response.status}`)
  }

  return (await response.json()) as GpuStatusResponse
}

export default function FooterStatusBar() {
  const [snapshot, setSnapshot] = useState<GpuStatusResponse | null>(null)
  const [isLoading, setIsLoading] = useState(true)

  useEffect(() => {
    let mounted = true
    let inflight = false

    const refresh = async () => {
      if (inflight) {
        return
      }

      inflight = true

      try {
        const next = await fetchGpuStatus()
        if (mounted) {
          setSnapshot(next)
          setIsLoading(false)
        }
      } catch (error) {
        if (mounted) {
          setSnapshot({
            available: false,
            backend: "all-smi",
            updated_at: new Date().toISOString(),
            devices: [],
            error: error instanceof Error ? error.message : String(error),
          })
          setIsLoading(false)
        }
      } finally {
        inflight = false
      }
    }

    void refresh()
    const timer = window.setInterval(() => {
      void refresh()
    }, POLL_INTERVAL_MS)

    return () => {
      mounted = false
      clearInterval(timer)
    }
  }, [])

  const summary = useMemo(() => {
    const devices = snapshot?.devices ?? []

    if (devices.length === 0) {
      return {
        available: Boolean(snapshot?.available),
        model: "No GPU",
        memory: "-- / --",
        utilization: "--",
      }
    }

    const first = devices[0]
    const totalUsed = devices.reduce((sum, item) => sum + item.used_memory_bytes, 0)
    const totalMemory = devices.reduce((sum, item) => sum + item.total_memory_bytes, 0)

    return {
      available: Boolean(snapshot?.available),
      model: devices.length === 1 ? first.name : `${first.name} +${devices.length - 1}`,
      memory: `${formatGiB(totalUsed)} / ${formatGiB(totalMemory)}`,
      utilization: `${first.utilization_percent.toFixed(0)}%`,
    }
  }, [snapshot])

  const status = isLoading ? "info" : summary.available ? "success" : "danger"

  return (
    <footer className="workspace-surface flex h-[var(--shell-footer-height)] items-center justify-between rounded-[28px] px-4 text-[11px] text-muted-foreground">
      <div className="flex min-w-0 items-center gap-3 overflow-hidden">
        <StatusPill status={status} className="shrink-0">
          <Cpu className="size-3" />
          GPU
        </StatusPill>

        <span className="truncate font-medium text-foreground/85" title={summary.model}>
          {isLoading ? "Detecting hardware..." : summary.model}
        </span>

        <span className="hidden items-center gap-1 rounded-full bg-[var(--surface-soft)] px-3 py-1 tabular-nums sm:inline-flex">
          <MemoryStick className="size-3" />
          {summary.memory}
        </span>

        <span className="hidden items-center gap-1 rounded-full bg-[var(--surface-soft)] px-3 py-1 tabular-nums md:inline-flex">
          <Gauge className="size-3" />
          {summary.utilization}
        </span>
      </div>

      <span className="shrink-0 text-[10px] font-semibold uppercase tracking-[0.16em] text-muted-foreground/85">
        {snapshot?.backend ?? "all-smi"}
      </span>
    </footer>
  )
}
