import { useEffect, useMemo, useState } from "react"
import { Bell } from "lucide-react"

import { getApiUrl } from "@/lib/tauri-api"
import { cn } from "@/lib/utils"

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

type FooterStatusBarProps = {
  variant?: "default" | "chat"
}

type FooterMetricProps = {
  label: string
  value: string
  title?: string
  className?: string
}

function FooterMetric({ label, value, title, className }: FooterMetricProps) {
  return (
    <div className={cn("flex min-w-0 items-center gap-2", className)} title={title ?? value}>
      <span className="shrink-0 text-[10px] font-bold uppercase tracking-[-0.04em] text-[var(--shell-footer-label)]">
        {label}
      </span>
      <span className="truncate text-[10px] font-bold uppercase tracking-[-0.025em] text-[var(--shell-footer-value)]">
        {value}
      </span>
    </div>
  )
}

export default function FooterStatusBar({ variant = "default" }: FooterStatusBarProps) {
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

  const isChatVariant = variant === "chat"
  const gpuText = isLoading ? "Detecting GPU..." : summary.model
  const telemetryLabel = snapshot?.backend ?? "all-smi"

  return (
    <footer
      className="shell-footer-bar flex h-[var(--shell-footer-height)] items-center justify-between px-4 sm:px-6"
    >
      <div className="flex min-w-0 items-center gap-4 overflow-hidden sm:gap-6">
        <FooterMetric
          label="GPU"
          value={gpuText}
          title={summary.model}
          className={cn(isChatVariant ? "max-w-[9rem] sm:max-w-[14rem]" : "max-w-[11rem] sm:max-w-[18rem]")}
        />
        <FooterMetric label="VRAM" value={summary.memory} className="hidden sm:flex" />
        <FooterMetric label="LOAD" value={summary.utilization} className="hidden md:flex" />
      </div>

      <div className="ml-4 flex shrink-0 items-center gap-3">
        <span className="hidden text-[10px] font-bold uppercase tracking-[0.12em] text-[var(--shell-footer-label)] lg:block">
          {telemetryLabel}
        </span>
        <div
          aria-hidden="true"
          className={cn(
            "flex size-5 items-center justify-center text-[var(--shell-footer-label)]",
            summary.available && !isLoading && "text-[var(--shell-footer-value)]"
          )}
        >
          <Bell className="size-3.5" />
        </div>
      </div>
    </footer>
  )
}
