import {
  Puzzle,
  Subtitles,
} from "lucide-react"
import { useTranslation } from "@slab/i18n"
import { Link, useLocation } from "react-router-dom"

import { cn } from "@slab/ui/lib/utils"
import { WindowControls } from "@slab/ui/layouts/window-controls"
import { useRuntimePlugins } from "@slab/ui/pages/plugins/hooks/use-runtime-plugins"
import { getDesktopRouteEntries, type DesktopRouteObject } from "@slab/ui/routes/route-meta"
import type { HeaderIcon } from "@slab/ui/layouts/header"

type SidebarItem = {
  to: string
  labelKey?: string
  label?: string
  icon: HeaderIcon
  end?: boolean
}

const isPathActive = (pathname: string, to: string, end = false) => {
  if (end) {
    return pathname === to
  }

  return pathname === to || pathname.startsWith(`${to}/`)
}

type AppSidebarProps = {
  routes: readonly DesktopRouteObject[]
  variant?: "default" | "chat"
}

export function AppSidebar({ routes, variant = "default" }: AppSidebarProps) {
  const { t } = useTranslation()
  const { pathname } = useLocation()
  const { data: runtimePlugins = [] } = useRuntimePlugins()
  const isChatVariant = variant === "chat"
  const routeSidebarItems = getDesktopRouteEntries(routes)
    .map(({ path, route }): (SidebarItem & { group: "primary" | "footer" }) | null => {
      const meta = route.meta
      const sidebar = meta?.sidebar
      if (!meta || !sidebar) return null

      return {
        to: path,
        labelKey: sidebar.labelKey,
        label: sidebar.label,
        icon: meta.icon,
        end: sidebar.end,
        group: sidebar.group,
      }
    })
    .filter((item): item is SidebarItem & { group: "primary" | "footer" } => item !== null)
  const primaryItems = routeSidebarItems.filter((item) => item.group === "primary")
  const footerItems = routeSidebarItems.filter((item) => item.group === "footer")
  const pluginItems = runtimePlugins
    .filter((plugin) => plugin.valid && plugin.enabled && plugin.uiEntry && plugin.uiUrl)
    .flatMap((plugin) =>
      (plugin.contributions?.sidebar ?? [])
        .map((item): SidebarItem | null => {
          const targetRoute = (plugin.contributions?.routes ?? []).find(
            (contributedRoute) =>
              contributedRoute.id === item.route || contributedRoute.path === item.route
          )
          if (!targetRoute) return null
          return {
            to: targetRoute.path,
            labelKey: item.labelKey ?? targetRoute.titleKey ?? undefined,
            label: item.label ?? targetRoute.title ?? item.id,
            icon: item.icon === "subtitles" ? Subtitles : Puzzle,
          }
        })
        .filter((item): item is SidebarItem => item !== null)
    )
  const visiblePrimaryItems = [...primaryItems, ...pluginItems]

  const renderItem = (item: SidebarItem) => {
    const Icon = item.icon
    const active = isPathActive(pathname, item.to, item.end)

    return (
      <Link
        key={item.to}
        to={item.to}
        aria-current={active ? "page" : undefined}
        data-testid={`sidebar-link-${item.to === "/" ? "assistant" : item.to.replace(/^\/+/, "").replaceAll("/", "-")}`}
        data-active={active ? "true" : "false"}
        className={cn(
          "focus-ring flex flex-col items-center justify-center rounded-xl transition-[background-color,color,box-shadow,opacity,transform] duration-[var(--dur-180)] ease-out-expo",
          active
            ? "size-[52px] bg-[var(--shell-card)] text-[color:var(--shell-rail-active)] opacity-100"
            : "size-12 text-[color:var(--shell-rail-label)] opacity-70 hover:-translate-y-px hover:bg-glass-bg-strong hover:text-[color:var(--shell-title)] hover:opacity-100"
        )}
      >
        <Icon className="size-[18px]" />
        <span
          className={cn(
            "pt-1 text-micro font-medium leading-[15px] tracking-tight",
            active && "text-[color:var(--shell-rail-active)]"
          )}
        >
          {item.labelKey ? t(item.labelKey) : item.label}
        </span>
      </Link>
    )
  }

  return (
    <aside
      className={cn(
        "flex min-h-0 w-[var(--shell-rail-width)] shrink-0 flex-col bg-[var(--shell-rail-bg)] py-6",
        !isChatVariant && "hairline-inset-right"
      )}
    >
      <div className="flex flex-1 flex-col items-center justify-between">
        <div className="flex flex-col items-center gap-6">
          <WindowControls placement="sidebar" />
          <div className="flex h-[54px] w-[59px] items-center justify-center rounded-[16px] bg-[var(--shell-card)]">
            <span className="text-xl font-bold tracking-display text-[color:var(--brand-teal)]">
              Slab
            </span>
          </div>

          <nav className="flex flex-col items-center gap-4">
            {visiblePrimaryItems.map(renderItem)}
          </nav>
        </div>

        <div className="flex flex-col items-center gap-4">
          {footerItems.map(renderItem)}
        </div>
      </div>
    </aside>
  )
}
