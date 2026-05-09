import { Button } from "@slab/components/button"
import { SoftPanel } from "@slab/components/workspace"

export function RecentWorkspaceList({
  recentWorkspaces,
  onOpen,
  title,
  emptyLabel,
  openLabel,
}: {
  recentWorkspaces: Array<{ rootPath: string; name: string }>
  onOpen: (rootPath: string) => Promise<void>
  title: string
  emptyLabel: string
  openLabel: string
}) {
  return (
    <SoftPanel className="rounded-[18px] px-5 py-5">
      <h3 className="text-sm font-semibold">{title}</h3>
      <div className="mt-4 grid gap-2">
        {recentWorkspaces.length === 0 ? (
          <p className="text-sm text-muted-foreground">{emptyLabel}</p>
        ) : (
          recentWorkspaces.map((workspace) => (
            <div
              key={workspace.rootPath}
              className="flex min-w-0 items-center justify-between gap-3 rounded-[12px] bg-[var(--surface-1)] px-3 py-3"
            >
              <div className="min-w-0">
                <p className="truncate text-sm font-medium">{workspace.name}</p>
                <p className="mt-0.5 truncate text-xs text-muted-foreground">{workspace.rootPath}</p>
              </div>
              <Button variant="pill" size="xs" onClick={() => void onOpen(workspace.rootPath)}>
                {openLabel}
              </Button>
            </div>
          ))
        )}
      </div>
    </SoftPanel>
  )
}
