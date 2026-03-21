import {
  BotMessageSquare,
  ClipboardList,
  Film,
  ImageIcon,
  Mic,
  Package,
  Puzzle,
  Settings,
  type LucideIcon,
} from "lucide-react"
import { Link, useLocation } from "react-router-dom"

import { Button } from "@/components/ui/button"
import { StatusPill } from "@/components/ui/workspace"
import { cn } from "@/lib/utils"

type SidebarItem = {
  to: string
  label: string
  icon: LucideIcon
  end?: boolean
}

const primaryItems: SidebarItem[] = [
  { to: "/", label: "Chat", icon: BotMessageSquare, end: true },
  { to: "/image", label: "Image", icon: ImageIcon },
  { to: "/video", label: "Video", icon: Film },
  { to: "/audio", label: "Audio", icon: Mic },
  { to: "/hub", label: "Hub", icon: Package },
  { to: "/task", label: "Tasks", icon: ClipboardList },
  { to: "/plugins", label: "Plugins", icon: Puzzle },
]

const footerItems: SidebarItem[] = [
  { to: "/settings", label: "Settings", icon: Settings },
]

const isPathActive = (pathname: string, to: string, end = false) => {
  if (end) {
    return pathname === to
  }

  return pathname === to || pathname.startsWith(`${to}/`)
}

export function AppSidebar() {
  const { pathname } = useLocation()

  const renderItem = (item: SidebarItem) => {
    const Icon = item.icon
    const active = isPathActive(pathname, item.to, item.end)

    return (
      <Button
        key={item.to}
        asChild
        variant="rail"
        size="rail"
        data-active={active ? "true" : "false"}
        className="h-14 w-full rounded-[20px] px-2"
      >
        <Link
          to={item.to}
          aria-current={active ? "page" : undefined}
          className="flex h-full w-full flex-col items-center justify-center gap-1"
        >
          <Icon className={cn("size-4", active && "text-[var(--brand-teal)]")} />
          <span className="text-[11px] font-medium tracking-tight">{item.label}</span>
        </Link>
      </Button>
    )
  }

  return (
    <aside className="workspace-surface flex w-[var(--shell-rail-width)] shrink-0 flex-col rounded-[32px] p-2">
      <div className="flex flex-col items-center gap-3 px-1 py-2">
        <div className="flex size-12 items-center justify-center rounded-[20px] bg-[linear-gradient(180deg,color-mix(in_oklab,var(--brand-teal)_20%,var(--surface-soft))_0%,var(--surface-soft)_100%)] shadow-[0_18px_30px_-24px_color-mix(in_oklab,var(--brand-teal)_35%,transparent)]">
          <span className="text-xs font-semibold tracking-[0.18em] text-[var(--brand-teal)]">
            SLAB
          </span>
        </div>
        <StatusPill status="info" className="px-2 py-1 text-[10px]">
          AI
        </StatusPill>
      </div>

      <nav className="mt-4 flex flex-1 flex-col gap-2">
        {primaryItems.map(renderItem)}
      </nav>

      <div className="mt-4 border-t border-border/60 pt-3">
        {footerItems.map(renderItem)}
      </div>
    </aside>
  )
}
