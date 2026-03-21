import { Sparkles } from "lucide-react"

import { BackendStatus } from "@/components/backend-status"
import { useGlobalHeaderMeta } from "@/hooks/use-global-header-meta"

export default function Header() {
  const { title, subtitle, icon: Icon } = useGlobalHeaderMeta()

  return (
    <header className="workspace-surface flex h-[var(--shell-topbar-height)] items-center gap-4 rounded-[32px] px-5">
      <div className="flex min-w-0 items-center gap-4">
        <div className="flex size-11 shrink-0 items-center justify-center rounded-[20px] bg-[linear-gradient(180deg,color-mix(in_oklab,var(--brand-teal)_14%,var(--surface-soft))_0%,var(--surface-soft)_100%)] text-[var(--brand-teal)] shadow-[0_18px_30px_-24px_color-mix(in_oklab,var(--brand-teal)_35%,transparent)]">
          <Icon className="size-5" />
        </div>
        <div className="min-w-0">
          <div className="mb-1 inline-flex items-center gap-2">
            <span className="text-[11px] font-semibold uppercase tracking-[0.18em] text-muted-foreground">
              Workspace
            </span>
            <Sparkles className="size-3 text-[var(--brand-gold)]" />
          </div>
          <h2 className="line-clamp-1 text-lg font-semibold leading-none tracking-tight text-foreground">
            {title}
          </h2>
          <p className="mt-1 line-clamp-1 text-sm text-muted-foreground">
            {subtitle}
          </p>
        </div>
      </div>
      <div className="ml-auto flex items-center gap-3">
        <BackendStatus />
      </div>
    </header>
  )
}
