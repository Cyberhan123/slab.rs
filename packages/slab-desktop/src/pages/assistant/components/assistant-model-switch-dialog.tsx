import { Loader2 } from "lucide-react"

import { Button } from "@slab/components/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@slab/components/dialog"
import { Trans, useTranslation } from "@slab/i18n"

type AssistantModelSwitchDialogProps = {
  conversationLabel: string
  isCreatingSession: boolean
  messageCount: number
  onCreateSession: () => void
  onKeepSession: () => void
  onOpenChange: (open: boolean) => void
  pendingModelId: string | null
  pendingModelLabel: string | null | undefined
  selectedModelLabel: string | null | undefined
}

export function AssistantModelSwitchDialog({
  conversationLabel,
  isCreatingSession,
  messageCount,
  onCreateSession,
  onKeepSession,
  onOpenChange,
  pendingModelId,
  pendingModelLabel,
  selectedModelLabel,
}: AssistantModelSwitchDialogProps) {
  const { t } = useTranslation()
  const placeholder = t("pages.assistant.modelPicker.placeholder")

  return (
    <Dialog open={Boolean(pendingModelId)} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl" showCloseButton={!isCreatingSession}>
        <DialogHeader className="space-y-3 text-left">
          <DialogTitle>{t("pages.assistant.dialog.title")}</DialogTitle>
          <DialogDescription>
            {t("pages.assistant.dialog.description")}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-2 text-sm leading-6 text-muted-foreground">
          <p>
            <Trans
              i18nKey="pages.assistant.dialog.switchingSummary"
              values={{
                from: selectedModelLabel ?? placeholder,
                to: pendingModelLabel ?? pendingModelId ?? placeholder,
              }}
              components={{ strong: <strong /> }}
            />
          </p>
          <p>
            <Trans
              i18nKey="pages.assistant.dialog.sessionSummary"
              count={messageCount}
              values={{
                count: messageCount,
                label: conversationLabel,
              }}
              components={{ strong: <strong /> }}
            />
          </p>
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          <div className="rounded-2xl border border-border/70 bg-[var(--surface-1)] px-4 py-3">
            <p className="text-sm font-medium text-foreground">
              {t("pages.assistant.dialog.keepTitle")}
            </p>
            <p className="mt-1 text-sm leading-6 text-muted-foreground">
              {t("pages.assistant.dialog.keepDescription")}
            </p>
          </div>
          <div className="rounded-2xl border border-border/70 bg-[var(--surface-1)] px-4 py-3">
            <p className="text-sm font-medium text-foreground">
              {t("pages.assistant.dialog.createTitle")}
            </p>
            <p className="mt-1 text-sm leading-6 text-muted-foreground">
              {t("pages.assistant.dialog.createDescription")}
            </p>
          </div>
        </div>

        <DialogFooter className="gap-2">
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={isCreatingSession}>
            {t("pages.assistant.dialog.cancel")}
          </Button>
          <Button variant="secondary" onClick={onKeepSession} disabled={isCreatingSession}>
            {t("pages.assistant.dialog.keepTitle")}
          </Button>
          <Button onClick={onCreateSession} disabled={isCreatingSession}>
            {isCreatingSession ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : null}
            {t("pages.assistant.dialog.createTitle")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
