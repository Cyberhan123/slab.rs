import { SidebarTrigger } from "@/components/ui/sidebar";
import { BackendStatus } from "@/components/backend-status";
import { useGlobalHeaderMeta } from "@/hooks/use-global-header-meta";

export default function Header() {
  const { title, subtitle, icon: Icon } = useGlobalHeaderMeta();

  return (
    <header className="flex h-20 items-center gap-4 border-b bg-background px-4">
      <div className="flex items-center gap-3 py-2">
        <SidebarTrigger className="-ml-1 h-9 w-9" />
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-primary/20 bg-primary/10">
          <Icon className="size-4 text-primary" />
        </div>
        <div className="flex flex-col justify-center overflow-hidden">
          <h2 className="line-clamp-1 text-sm font-semibold leading-none tracking-tight text-foreground">
            {title}
          </h2>
          <p className="mt-1 line-clamp-1 text-xs text-muted-foreground">
            {subtitle}
          </p>
        </div>
      </div>
      <div className="ml-auto flex items-center gap-4">
        <BackendStatus />
      </div>
    </header>
  );
}
