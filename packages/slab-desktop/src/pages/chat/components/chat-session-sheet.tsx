import { MoreHorizontal, Trash2 } from "lucide-react"

import { Badge } from "@slab/components/badge"
import { Button } from "@slab/components/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@slab/components/dropdown-menu"
import { ScrollArea } from "@slab/components/scroll-area"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@slab/components/sheet"
import { useTranslation } from "@slab/i18n"
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
  onDelete,
}: ChatSessionSheetProps) {
  const { t } = useTranslation()

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="right"
        className="w-full overflow-hidden border-l border-border/60 bg-[var(--surface-1)] p-0 sm:max-w-xl"
      >
          <SheetHeader className="shrink-0 border-b border-border/60 px-6 py-5 pr-14">
            <div className="space-y-1">
              <SheetTitle className="text-xl">{t("pages.chat.sessionSheet.title")}</SheetTitle>
              <SheetDescription>
                {t("pages.chat.sessionSheet.description")}
              </SheetDescription>
            </div>
          </SheetHeader>

        <ScrollArea className="min-h-0 flex-1 overflow-hidden">
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
                      <p className="truncate font-medium">
                        {conversation.label ?? t("pages.chat.runtime.newChat")}
                      </p>
                      {isCurrent ? (
                        <Badge variant="chip">{t("pages.chat.sessionSheet.current")}</Badge>
                      ) : null}
                      {isActive ? <Badge variant="chip">{t("pages.chat.sessionSheet.live")}</Badge> : null}
                    </div>
                    <p className="mt-1 text-sm text-muted-foreground">
                      {conversation.group ?? t("pages.chat.runtime.workspace")}
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
                          {t("pages.chat.sessionSheet.open")}
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          disabled={busy}
                          variant="destructive"
                          onClick={() => onDelete(conversation.key)}
                        >
                          <Trash2 className="size-4" />
                          {t("pages.chat.sessionSheet.delete")}
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
