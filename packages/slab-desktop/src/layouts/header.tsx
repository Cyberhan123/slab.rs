import { BotMessageSquare, History, Search } from "lucide-react"
import { useTranslation } from "@slab/i18n"
import { Input } from "@slab/components/input"
import { useHeader } from "@/hooks/use-header"
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
import type { ComponentType } from "react"

export type HeaderIcon = ComponentType<{
  className?: string
}>

export type HeaderMeta = {
  title: string
  subtitle: string
  icon: HeaderIcon
  contextLabel?: string | null
}

export const DEFAULT_HEADER_META: HeaderMeta = {
  title: "Slab",
  subtitle: "ML Inference Platform",
  icon: BotMessageSquare,
  contextLabel: null,
}

export type HeaderSelectOption = {
  id: string
  label: string
  disabled?: boolean
  children?: {
    groupLabel: string
    options: HeaderSelectOption[]
  }
}

export type HeaderSelectConfig = {
  value: string
  options: HeaderSelectOption[]
  placeholder?: string
  onChange: (value: string) => void
  groupLabel?: string
  loading?: boolean
  disabled?: boolean
  emptyLabel?: string
}

export type HeaderSearchConfig = {
  value: string
  placeholder?: string
  onChange: (value: string) => void
  ariaLabel?: string
  disabled?: boolean
}

export const HEADER_SELECT_KEYS = {
  assistantModel: "assistant:model",
  audioModel: "audio:model",
  imageModel: "image:model",
  videoModel: "video:model",
} as const

function HeaderSelectGroup({
  groupLabel,
  options,
}: {
  groupLabel: string
  options: HeaderSelectOption[]
}) {
  return (
    <SelectGroup>
      <SelectLabel>{groupLabel}</SelectLabel>
      {options.map((option) => (
        <SelectItem key={option.id} value={option.id} disabled={option.disabled}>
          {option.label}
        </SelectItem>
      ))}
    </SelectGroup>
  )
}

function flattenOptions(options: HeaderSelectOption[]): HeaderSelectOption[] {
  return options.flatMap((option) =>
    option.children ? option.children.options : [option]
  )
}

function HeaderSelect({ select }: { select: HeaderSelectConfig }) {
  const { t } = useTranslation()
  const flatOptions = flattenOptions(select.options)
  const selectedOption = flatOptions.find((option) => option.id === select.value)
  const hasSelectableOptions = flatOptions.some((option) => !option.disabled)
  const placeholder = select.loading
    ? t("layouts.header.select.loadingOptions")
    : select.placeholder ?? t("layouts.header.select.selectOption")
  const disabled = select.disabled || !hasSelectableOptions

  return (
    <Select value={select.value} onValueChange={select.onChange} disabled={disabled}>
      <SelectTrigger
        size="sm"
        variant="default"
        title={selectedOption?.label ?? placeholder}
        className="shell-context hidden h-8 max-w-[18rem] shrink-0 border-border/30 bg-glass-bg-strong pl-3 pr-2.5 text-label font-semibold text-foreground/70 lg:flex"
      >
        <span className="size-2 shrink-0 rounded-full bg-[var(--brand-gold)]" />
        <SelectValue placeholder={placeholder} className="max-w-[11rem] truncate" />
      </SelectTrigger>
      <SelectContent variant="default" position="popper" align="start" className="max-h-80 min-w-[18rem]">
        {select.options.length === 0 ? (
          <SelectGroup>
            <SelectLabel>{select.groupLabel ?? t("layouts.header.select.options")}</SelectLabel>
            <SelectItem value="__no_options__" disabled>
              {select.emptyLabel ?? t("layouts.header.select.noOptions")}
            </SelectItem>
          </SelectGroup>
        ) : (
          select.options.map((option) =>
            option.children ? (
              <HeaderSelectGroup
                key={option.id}
                groupLabel={option.children.groupLabel}
                options={option.children.options}
              />
            ) : (
              <HeaderSelectGroup
                key={option.id}
                groupLabel={select.groupLabel ?? t("layouts.header.select.options")}
                options={[option]}
              />
            )
          )
        )}
      </SelectContent>
    </Select>
  )
}

export default function Header() {
  const { t } = useTranslation()
  const {
    meta: { title, subtitle, contextLabel },
    select,
    search,
  } = useHeader()
  const searchPlaceholder = search?.placeholder ?? t("layouts.header.search.default")
  const searchAriaLabel = search?.ariaLabel ?? searchPlaceholder

  return (
    <header
      className={cn(
        "shell-topbar text-body flex h-[var(--shell-topbar-height)] items-center justify-between gap-4 pl-5 md:pl-8"
      )}
      data-tauri-drag-region="true"
    >
      <div className="flex min-w-0 items-center gap-3 md:gap-4">
        <h2 className="shrink-0 text-lg font-extrabold tracking-display text-[color:var(--shell-title)]">
          {title}
        </h2>
        <span aria-hidden="true" className="hairline-v hidden h-4 w-px shrink-0 sm:block" />
        <p className="hidden max-w-[28rem] min-w-0 truncate text-body font-medium leading-5 text-[color:var(--shell-subtitle)] xl:max-w-[34rem] sm:block">
          {subtitle}
        </p>
        {select ? (
          <HeaderSelect select={select} />
        ) : contextLabel ? (
          <div className="shell-context hidden h-8 shrink-0 items-center gap-2 rounded-full border border-border/30 bg-glass-bg-strong pl-3 pr-2.5 text-label font-semibold text-foreground/70 lg:inline-flex">
            <span className="size-2 rounded-full bg-[var(--brand-gold)]" />
            <span className="max-w-[11rem] truncate">{contextLabel}</span>
          </div>
        ) : null}
      </div>

      <div className="ml-auto flex min-w-0 items-center gap-3 md:gap-4">
        {search ? (
          <>
            <div className="shell-search hidden h-8 min-w-[12rem] flex-1 items-center gap-2.5 rounded-full px-3.5 text-label text-[color:var(--shell-search-foreground)] md:flex lg:w-64">
              <Search className="size-3.5 shrink-0" />
              <Input
                type="search"
                value={search.value}
                onChange={(event) => search.onChange(event.target.value)}
                placeholder={searchPlaceholder}
                aria-label={searchAriaLabel}
                disabled={search.disabled}
                className="h-full border-0 bg-transparent p-0 text-label text-[color:var(--shell-search-foreground)] shadow-none outline-none placeholder:text-[color:var(--shell-search-foreground)]/70 focus-visible:border-transparent focus-visible:ring-0"
              />
            </div>
            <span aria-hidden="true" className="hairline-v hidden h-4 w-px shrink-0 md:block" />
            <div
              aria-hidden="true"
              data-testid="header-history-control"
              className="flex size-8 shrink-0 items-center justify-center rounded-full text-[color:var(--shell-rail-label)] transition duration-[var(--dur-180)] ease-out-expo hover:bg-glass-bg-strong hover:text-[color:var(--shell-title)]"
            >
              <History className="size-4" />
            </div>
            <span aria-hidden="true" className="hairline-v hidden h-4 w-px shrink-0 md:block" />
          </>
        ) : null}
        <WindowControls placement="header" />
      </div>
    </header>
  )
}
