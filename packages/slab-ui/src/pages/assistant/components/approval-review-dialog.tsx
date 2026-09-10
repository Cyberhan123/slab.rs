"use client"

import { useState } from "react"

import { Button } from "@slab/components/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@slab/components/dialog"
import { Label } from "@slab/components/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@slab/components/select"
import { Spinner } from "@slab/components/spinner"
import { Textarea } from "@slab/components/textarea"
import { useTranslation } from "@slab/i18n"
import { useAiModel } from "@slab/ui/hooks/use-ai-model"
import { useAssistantUiStore } from "@slab/ui/store/useAssistantUiStore"

type ApprovalReviewDialogProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
  /**
   * Saved with a real change (reviewer model and/or policy prompt) — carries
   * the pre/post values so the parent can record an in-stream marker. Fires
   * only when something actually changed.
   */
  onSaved?: (change: {
    fromModel: string | null
    toModel: string | null
    promptChanged: boolean
  }) => void
}

/**
 * "Approve for me" review configuration: pick the model that reviews approval
 * requests on the user's behalf and optionally append a custom policy prompt.
 * Persisted in the assistant UI store and re-applied on every turn via
 * `turn/start` `approvalModel` / `approvalPrompt`.
 *
 * The form body mounts only while the dialog is open, so the draft seeds from
 * the store on mount and abandoned edits are discarded on close.
 */
export function ApprovalReviewDialog({ open, onOpenChange, onSaved }: ApprovalReviewDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {open ? <ApprovalReviewForm onOpenChange={onOpenChange} onSaved={onSaved} /> : null}
    </Dialog>
  )
}

function ApprovalReviewForm({
  onOpenChange,
  onSaved,
}: {
  onOpenChange: (open: boolean) => void
  onSaved?: ApprovalReviewDialogProps["onSaved"]
}) {
  const { t } = useTranslation()
  const approvalReviewModel = useAssistantUiStore((state) => state.approvalReviewModel)
  const approvalReviewPrompt = useAssistantUiStore((state) => state.approvalReviewPrompt)
  const setApprovalReviewModel = useAssistantUiStore((state) => state.setApprovalReviewModel)
  const setApprovalReviewPrompt = useAssistantUiStore((state) => state.setApprovalReviewPrompt)

  const { options, loading } = useAiModel({ capability: "chat_generation", includeCloud: true })

  const [model, setModel] = useState(approvalReviewModel)
  const [prompt, setPrompt] = useState(approvalReviewPrompt)

  const save = () => {
    // Report the change BEFORE the store write (the pre-save store values are
    // the "from" side of the marker); skipped when nothing actually changed.
    if (model !== approvalReviewModel || prompt !== approvalReviewPrompt) {
      onSaved?.({
        fromModel: approvalReviewModel || null,
        toModel: model || null,
        promptChanged: prompt !== approvalReviewPrompt,
      })
    }
    setApprovalReviewModel(model)
    setApprovalReviewPrompt(prompt)
    onOpenChange(false)
  }

  return (
    <DialogContent className="max-w-lg">
      <DialogHeader className="space-y-2 text-left">
        <DialogTitle>{t("pages.assistant.approvalReview.dialogTitle")}</DialogTitle>
        <DialogDescription>
          {t("pages.assistant.approvalReview.dialogDescription")}
        </DialogDescription>
      </DialogHeader>

      <div className="space-y-4">
        <div className="space-y-2">
          <Label htmlFor="approval-review-model">
            {t("pages.assistant.approvalReview.modelField")}
          </Label>
          <Select value={model} onValueChange={setModel}>
            <SelectTrigger
              id="approval-review-model"
              data-testid="approval-review-model-select"
              className="w-full"
            >
              <SelectValue placeholder={t("pages.assistant.approvalReview.modelPlaceholder")} />
            </SelectTrigger>
            <SelectContent>
              {loading && options.length === 0 ? (
                <div className="flex items-center gap-2 px-2 py-1.5 text-sm text-muted-foreground">
                  <Spinner className="size-3" />
                  {t("common.status.loading")}
                </div>
              ) : null}
              {options.map((option) => (
                <SelectItem key={option.id} value={option.id} disabled={option.disabled}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <p className="text-xs leading-5 text-muted-foreground">
            {t("pages.assistant.approvalReview.modelHint")}
          </p>
        </div>

        <div className="space-y-2">
          <Label htmlFor="approval-review-prompt">
            {t("pages.assistant.approvalReview.promptField")}
          </Label>
          <Textarea
            id="approval-review-prompt"
            data-testid="approval-review-prompt-input"
            className="min-h-24"
            placeholder={t("pages.assistant.approvalReview.promptPlaceholder")}
            value={prompt}
            onChange={(event) => setPrompt(event.target.value)}
          />
          <p className="text-xs leading-5 text-muted-foreground">
            {t("pages.assistant.approvalReview.promptHint")}
          </p>
        </div>
      </div>

      <DialogFooter className="gap-2">
        <Button
          variant="outline"
          data-testid="approval-review-cancel-button"
          onClick={() => onOpenChange(false)}
        >
          {t("common.actions.cancel")}
        </Button>
        <Button data-testid="approval-review-save-button" disabled={!model} onClick={save}>
          {t("common.actions.save")}
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}
