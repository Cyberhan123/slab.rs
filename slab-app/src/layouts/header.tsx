import { ChevronDown, History, Search } from "lucide-react"
import { useGlobalHeaderMeta } from "@/hooks/use-global-header-meta"
import { WindowControls } from "@/layouts/window-controls"
import { cn } from "@/lib/utils"

type HeaderProps = {
  variant?: "default" | "chat"
}

export default function Header({ variant = "default" }: HeaderProps) {
  const { title, subtitle } = useGlobalHeaderMeta()
  const isChatVariant = variant === "chat"
  const searchPlaceholder = isChatVariant ? "Search tasks..." : "Search pages, tools, or settings..."
  const subtitleParts = isChatVariant ? subtitle.split(" - ") : [subtitle]
  const displaySubtitle = subtitleParts[0] ?? subtitle
  const shellContextLabel = isChatVariant
    ? subtitleParts.slice(1).join(" - ") || "Active Workspace"
    : "Slab Desktop"

  return (
    <header
      className={cn(
        "shell-topbar flex h-[var(--shell-topbar-height)] items-center justify-between gap-4 pl-5 md:pl-8"
      )}
    >
      <div className="flex min-w-0 items-center gap-3 md:gap-4">
        <h2 className="shrink-0 text-[17px] font-extrabold tracking-[-0.045em] text-[var(--shell-title)] md:text-[18px]">
          {title}
        </h2>
        <span className="hidden h-4 w-px shrink-0 bg-[var(--shell-divider)] sm:block" />
        <p className="hidden max-w-[28rem] min-w-0 truncate text-[13px] font-medium leading-5 text-[var(--shell-subtitle)] xl:max-w-[34rem] sm:block">
          {displaySubtitle}
        </p>
        <div className="hidden h-8 shrink-0 items-center gap-2 rounded-full border border-border/30 bg-[var(--shell-card)]/92 pl-3 pr-2.5 text-[12px] font-semibold text-foreground/70 shadow-[var(--shell-elevation)] lg:inline-flex">
          <span className="size-2 rounded-full bg-[var(--brand-gold)]" />
          <span className="max-w-[11rem] truncate">{shellContextLabel}</span>
          <ChevronDown className="size-3.5 text-muted-foreground" />
        </div>
      </div>

      <div className="ml-auto flex min-w-0 items-center gap-3 md:gap-4">
        <div className="shell-search hidden h-8 min-w-[12rem] flex-1 items-center gap-2.5 rounded-full px-3.5 text-[12px] text-[var(--shell-search-foreground)] md:flex lg:w-64">
          <Search className="size-3.5 shrink-0" />
          <span className="truncate">{searchPlaceholder}</span>
        </div>
        <span className="hidden h-4 w-px shrink-0 bg-[var(--shell-divider)] md:block" />
        <div
          aria-hidden="true"
          className="flex size-8 shrink-0 items-center justify-center rounded-full text-[var(--shell-rail-label)] transition hover:bg-[var(--shell-card)]/80 hover:text-[var(--shell-title)]"
        >
          <History className="size-4" />
        </div>
        <span className="hidden h-4 w-px shrink-0 bg-[var(--shell-divider)] md:block" />
        <WindowControls />
      </div>
    </header>
  )
}
