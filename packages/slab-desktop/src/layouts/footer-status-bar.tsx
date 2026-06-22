import { useEffect, useMemo } from "react"
import { Bell } from "lucide-react"
import { useTranslation } from "@slab/i18n"
import { useInterval } from "@mantine/hooks"
import { sumBy } from "lodash-es"

import { getErrorMessage } from "@slab/api"
import type { components } from "@slab/api/v1"
import api from "@slab/api"
import { cn } from "@/lib/utils"

type GpuStatusResponse = components["schemas"]["GpuStatusResponse"]

const POLL_INTERVAL_MS = 30000

function formatGiB(bytes: number): string {
  const gib = bytes / 1024 ** 3
  return `${gib.toFixed(gib >= 10 ? 1 : 2)} GB`
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
      <span className="shrink-0 text-micro font-bold uppercase tracking-tight text-[var(--shell-footer-label)]">
        {label}
      </span>
      <span className="truncate text-micro font-bold uppercase tracking-tight text-[var(--shell-footer-value)]">
        {value}
      </span>
    </div>
  )
}

export default function FooterStatusBar({ variant = "default" }: FooterStatusBarProps) {
  const { t } = useTranslation()
  const {
    data,
    error,
    isLoading,
    refetch,
  } = api.useQuery("get", "/v1/system/gpu")

  const { start: startGpuPoll, stop: stopGpuPoll } = useInterval(() => {
    void refetch()
  }, POLL_INTERVAL_MS)

  useEffect(() => {
    startGpuPoll()
    return stopGpuPoll
  }, [startGpuPoll, stopGpuPoll])

  const snapshot = useMemo<GpuStatusResponse | null>(() => {
    if (data) {
      return data
    }

    if (!error) {
      return null
    }

    return {
      available: false,
      backend: "all-smi",
      updated_at: new Date().toISOString(),
      devices: [],
      error: getErrorMessage(error),
    }
  }, [data, error])

  const summary = useMemo(() => {
    const devices = snapshot?.devices ?? []

    if (devices.length === 0) {
      return {
        available: Boolean(snapshot?.available),
        model: t("layouts.footerStatusBar.values.noGpu"),
        memory: "-- / --",
        utilization: "--",
      }
    }

    const first = devices[0]
    const totalUsed = sumBy(devices, "used_memory_bytes")
    const totalMemory = sumBy(devices, "total_memory_bytes")

    return {
      available: Boolean(snapshot?.available),
      model: devices.length === 1 ? first.name : `${first.name} +${devices.length - 1}`,
      memory: `${formatGiB(totalUsed)} / ${formatGiB(totalMemory)}`,
      utilization: `${first.utilization_percent.toFixed(0)}%`,
    }
  }, [snapshot, t])

  const isChatVariant = variant === "chat"
  const gpuText = isLoading ? t("layouts.footerStatusBar.values.detectingGpu") : summary.model

  return (
    <footer
      className="shell-footer-bar flex h-[var(--shell-footer-height)] items-center justify-between px-4 sm:px-6"
    >
      <div className="flex min-w-0 items-center gap-4 overflow-hidden sm:gap-6">
        <FooterMetric
          label={t("layouts.footerStatusBar.metrics.gpu")}
          value={gpuText}
          title={summary.model}
          className={cn(isChatVariant ? "max-w-[9rem] sm:max-w-[14rem]" : "max-w-[11rem] sm:max-w-[18rem]")}
        />
        <FooterMetric
          label={t("layouts.footerStatusBar.metrics.vram")}
          value={summary.memory}
          className="hidden sm:flex"
        />
        <FooterMetric
          label={t("layouts.footerStatusBar.metrics.load")}
          value={summary.utilization}
          className="hidden md:flex"
        />
      </div>

      <div className="ml-4 flex shrink-0 items-center gap-3">
        {/* <span className="hidden text-micro font-bold uppercase tracking-eyebrow text-[var(--shell-footer-label)] lg:block">
          {telemetryLabel}
        </span> */}
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
