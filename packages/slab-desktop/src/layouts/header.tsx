import { History, Search } from "lucide-react"
import { useGlobalHeaderState } from "@/hooks/use-global-header-meta"
import type { HeaderSelectControl } from "@/layouts/header-controls"
import { WindowControls } from "@/layouts/window-controls"
import { cn } from "@/lib/utils"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@slab/components/select"

type HeaderProps = {
  variant?: "default" | "chat" | "minimal"
}

function HeaderSelect({ control }: { control: HeaderSelectControl }) {
  const selectedOption = control.options.find((option) => option.id === control.value)
  const hasSelectableOptions = control.options.some((option) => !option.disabled)
  const placeholder = control.loading ? "Loading options..." : control.placeholder ?? "Select option"
  const disabled = control.disabled || !hasSelectableOptions

  return (
    <Select value={control.value || undefined} onValueChange={control.onValueChange} disabled={disabled}>
      <SelectTrigger
        size="sm"
        variant="pill"
        title={selectedOption?.label ?? placeholder}
        className="shell-context hidden h-8 max-w-[18rem] shrink-0 border-border/30 bg-[var(--shell-card)]/92 pl-3 pr-2.5 text-[12px] font-semibold text-foreground/70 shadow-[var(--shell-elevation)] lg:flex"
      >
        <span className="size-2 shrink-0 rounded-full bg-[var(--brand-gold)]" />
        <SelectValue placeholder={placeholder} className="max-w-[11rem] truncate" />
      </SelectTrigger>
      <SelectContent variant="pill" position="popper" align="start" className="max-h-80 min-w-[18rem]">
        <SelectGroup>
          <SelectLabel>{control.groupLabel ?? "Options"}</SelectLabel>
          {control.options.length === 0 ? (
            <SelectItem value="__no_options__" disabled>
              {control.emptyLabel ?? "No options available"}
            </SelectItem>
          ) : (
            control.options.map((option) => (
              <SelectItem key={option.id} value={option.id} disabled={option.disabled}>
                {option.label}
              </SelectItem>
            ))
          )}
        </SelectGroup>
      </SelectContent>
    </Select>
  )
}

export default function Header({ variant = "default" }: HeaderProps) {
  const {
    meta: { title, subtitle },
    control,
  } = useGlobalHeaderState()
  const isChatVariant = variant === "chat"
  const isMinimalVariant = variant === "minimal"
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
      data-tauri-drag-region="true"
    >
      <div className="flex min-w-0 items-center gap-3 md:gap-4">
        <h2 className="shrink-0 text-[17px] font-extrabold tracking-[-0.045em] text-[var(--shell-title)] md:text-[18px]">
          {title}
        </h2>
        <span className="hidden h-4 w-px shrink-0 bg-[var(--shell-divider)] sm:block" />
        <p className="hidden max-w-[28rem] min-w-0 truncate text-[13px] font-medium leading-5 text-[var(--shell-subtitle)] xl:max-w-[34rem] sm:block">
          {displaySubtitle}
        </p>
        {!isMinimalVariant ? (
          control?.type === "select" ? (
            <HeaderSelect control={control} />
          ) : (
            <div className="shell-context hidden h-8 shrink-0 items-center gap-2 rounded-full border border-border/30 bg-[var(--shell-card)]/92 pl-3 pr-2.5 text-[12px] font-semibold text-foreground/70 shadow-[var(--shell-elevation)] lg:inline-flex">
              <span className="size-2 rounded-full bg-[var(--brand-gold)]" />
              <span className="max-w-[11rem] truncate">{shellContextLabel}</span>
            </div>
          )
        ) : null}
      </div>

      <div className="ml-auto flex min-w-0 items-center gap-3 md:gap-4">
        {!isMinimalVariant ? (
          <>
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
          </>
        ) : null}
        <WindowControls placement="header" />
      </div>
    </header>
  )
}
