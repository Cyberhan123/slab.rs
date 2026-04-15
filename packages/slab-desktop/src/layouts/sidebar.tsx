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
import { useTranslation } from "@slab/i18n"
import { Link, useLocation } from "react-router-dom"

import { cn } from "@/lib/utils"
import { WindowControls } from "@/layouts/window-controls"

type SidebarItem = {
  to: string
  labelKey: string
  icon: LucideIcon
  end?: boolean
}

const primaryItems: SidebarItem[] = [
  { to: "/", labelKey: "layouts.sidebar.items.assistant", icon: BotMessageSquare, end: true },
  { to: "/image", labelKey: "layouts.sidebar.items.image", icon: ImageIcon },
  { to: "/video", labelKey: "layouts.sidebar.items.video", icon: Film },
  { to: "/audio", labelKey: "layouts.sidebar.items.audio", icon: Mic },
  { to: "/hub", labelKey: "layouts.sidebar.items.hub", icon: Package },
  { to: "/task", labelKey: "layouts.sidebar.items.task", icon: ClipboardList },
  { to: "/plugins", labelKey: "layouts.sidebar.items.plugins", icon: Puzzle },
]

const footerItems: SidebarItem[] = [
  { to: "/settings", labelKey: "layouts.sidebar.items.settings", icon: Settings },
]

const isPathActive = (pathname: string, to: string, end = false) => {
  if (end) {
    return pathname === to
  }

  return pathname === to || pathname.startsWith(`${to}/`)
}

type AppSidebarProps = {
  variant?: "default" | "chat"
}

export function AppSidebar({ variant = "default" }: AppSidebarProps) {
  const { t } = useTranslation()
  const { pathname } = useLocation()
  const isChatVariant = variant === "chat"

  const renderItem = (item: SidebarItem) => {
    const Icon = item.icon
    const active = isPathActive(pathname, item.to, item.end)

    return (
      <Link
        key={item.to}
        to={item.to}
        aria-current={active ? "page" : undefined}
        data-active={active ? "true" : "false"}
        className={cn(
          "flex flex-col items-center justify-center rounded-[12px] outline-none transition-[background-color,color,box-shadow,opacity,transform] duration-200 focus-visible:ring-2 focus-visible:ring-[color-mix(in_oklab,var(--brand-teal)_28%,transparent)] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--shell-rail-bg)]",
          active
            ? "size-[52px] bg-[var(--shell-card)] text-[var(--shell-rail-active)] opacity-100 shadow-[var(--shell-elevation)]"
            : "size-12 text-[var(--shell-rail-label)] opacity-70 hover:-translate-y-px hover:bg-[var(--shell-card)]/80 hover:text-[var(--shell-title)] hover:opacity-100"
        )}
      >
        <Icon className="size-[18px]" />
        <span
          className={cn(
            "pt-1 text-[10px] font-medium leading-[15px] tracking-[-0.025em]",
            active && "text-[var(--shell-rail-active)]"
          )}
        >
          {t(item.labelKey)}
        </span>
      </Link>
    )
  }

  return (
    <aside
      className={cn(
        "flex min-h-0 w-[var(--shell-rail-width)] shrink-0 flex-col bg-[var(--shell-rail-bg)] py-6",
        !isChatVariant && "shadow-[inset_-1px_0_0_var(--shell-line)]"
      )}
    >
      <div className="flex flex-1 flex-col items-center justify-between">
        <div className="flex flex-col items-center gap-6">
          <WindowControls placement="sidebar" />
          <div className="flex h-[54px] w-[59px] items-center justify-center rounded-[16px] bg-[var(--shell-card)] shadow-[var(--shell-elevation)]">
            <span className="text-[20px] font-bold tracking-[-0.045em] text-[var(--brand-teal)]">
              Slab
            </span>
          </div>

          <nav className="flex flex-col items-center gap-4">
            {primaryItems.map(renderItem)}
          </nav>
        </div>

        <div className="flex flex-col items-center gap-4">
          {footerItems.map(renderItem)}
        </div>
      </div>
    </aside>
  )
}
