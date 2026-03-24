import { MoreHorizontal, Plus, Trash2 } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { ScrollArea } from "@/components/ui/scroll-area"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { cn } from "@/lib/utils"

type ConversationItem = {
  key: string
  label?: string
  group?: string
}

type ChatSessionSheetProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
  conversations: ConversationItem[]
  currentConversation: string
  activeConversation?: string
  busy?: boolean
  onSelect: (key: string) => void
  onCreate: () => void
  onDelete: (key: string) => void
}

export function ChatSessionSheet({
  open,
  onOpenChange,
  conversations,
  currentConversation,
  activeConversation,
  busy = false,
  onSelect,
  onCreate,
  onDelete,
}: ChatSessionSheetProps) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="right"
        className="w-full border-l border-border/60 bg-[var(--surface-1)] p-0 sm:max-w-xl"
      >
        <SheetHeader className="border-b border-border/60 px-6 py-5">
          <div className="flex items-start justify-between gap-4">
            <div className="space-y-1">
              <SheetTitle className="text-xl">Manage sessions</SheetTitle>
              <SheetDescription>
                Create, switch, and clean up conversations without leaving the chat stage.
              </SheetDescription>
            </div>
            <Button variant="outline" size="sm" className="shrink-0" onClick={onCreate} disabled={busy}>
              <Plus className="size-4" />
              New chat
            </Button>
          </div>
        </SheetHeader>

        <ScrollArea className="h-full">
          <div className="space-y-3 px-6 py-5">
            {conversations.map((conversation) => {
              const isCurrent = conversation.key === currentConversation
              const isActive = conversation.key === activeConversation

              return (
                <div
                  key={conversation.key}
                  className={cn(
                    "workspace-soft-panel flex items-center gap-3 rounded-[24px] px-4 py-3",
                    isCurrent && "border-[color:var(--brand-teal)] bg-[color:color-mix(in_oklab,var(--brand-teal)_8%,var(--surface-soft))]"
                  )}
                >
                  <button
                    type="button"
                    disabled={busy}
                    className="flex min-w-0 flex-1 flex-col items-start text-left"
                    onClick={() => onSelect(conversation.key)}
                  >
                    <div className="flex items-center gap-2">
                      <p className="truncate font-medium">{conversation.label ?? "New chat"}</p>
                      {isCurrent ? <Badge variant="chip">Current</Badge> : null}
                      {isActive ? <Badge variant="chip">Live</Badge> : null}
                    </div>
                    <p className="mt-1 text-sm text-muted-foreground">
                      {conversation.group ?? "Workspace"}
                    </p>
                  </button>

                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button variant="quiet" size="icon-sm" className="rounded-full" disabled={busy}>
                          <MoreHorizontal className="size-4" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end" className="rounded-2xl border-border/70">
                        <DropdownMenuItem disabled={busy} onClick={() => onSelect(conversation.key)}>
                          Open
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          disabled={busy}
                          variant="destructive"
                          onClick={() => onDelete(conversation.key)}
                        >
                          <Trash2 className="size-4" />
                          Delete
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                </div>
              )
            })}
          </div>
        </ScrollArea>
      </SheetContent>
    </Sheet>
  )
}
