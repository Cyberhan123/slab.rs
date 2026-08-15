import { useCallback, useEffect, useRef, useState, type ReactNode } from "react"
import { useTranslation } from "@slab/i18n"
import { Button } from "@slab/components/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@slab/components/dialog"

type ConfirmTone = "danger" | "default"

type ConfirmOptions = {
  /** i18n key for the descriptive message body, e.g. pages.workspace.confirm.discardUnsaved. */
  messageKey: string
  /** Optional interpolation params for the message body. */
  messageParams?: Record<string, unknown>
  /** i18n key for the confirmation button label, e.g. pages.workspace.confirm.discard. */
  confirmKey: string
  /** destructive shows the confirm button in the danger style (discard/close). */
  tone?: ConfirmTone
}

type ConfirmState = ConfirmOptions & {
  open: boolean
  resolve: ((value: boolean) => void) | null
}

const INITIAL_STATE: ConfirmState = {
  open: false,
  messageKey: "pages.workspace.confirm.discardUnsaved",
  confirmKey: "pages.workspace.confirm.discard",
  tone: "default",
  resolve: null,
}

/**
 * Imperative confirm dialog built on the app's Radix-based Dialog. Callers await
 * `confirm(...)` and proceed only when it resolves `true`, replacing the native
 * browser confirm prompts so the prompt matches the product design system. The modal
 * overlay blocks interaction while the prompt is open, so there is no race between
 * the user deciding and the file tree / tabs being clicked.
 */
export function useWorkspaceConfirmDialog() {
  const { t } = useTranslation()
  const [state, setState] = useState<ConfirmState>(INITIAL_STATE)
  const stateRef = useRef(state)
  stateRef.current = state

  const confirm = useCallback((options: ConfirmOptions) => {
    return new Promise<boolean>((resolve) => {
      setState({ ...options, open: true, resolve })
    })
  }, [])

  const settle = useCallback((value: boolean) => {
    stateRef.current.resolve?.(value)
    setState((prev) => ({ ...prev, open: false, resolve: null }))
  }, [])

  const handleOpenChange = useCallback(
    (open: boolean) => {
      // Closing the dialog any other way (overlay click, Escape) cancels.
      if (!open) {
        settle(false)
      }
    },
    [settle],
  )

  // If the owning view unmounts while a prompt is pending, deny so the awaiting
  // caller never hangs.
  useEffect(() => {
    return () => {
      stateRef.current.resolve?.(false)
    }
  }, [])

  const dialog: ReactNode = (
    <Dialog open={state.open} onOpenChange={handleOpenChange}>
      <DialogContent className="max-w-md" data-testid="workspace-confirm-dialog">
        <DialogHeader>
          <DialogTitle>{t("pages.workspace.confirm.title")}</DialogTitle>
          <DialogDescription>{t(state.messageKey, state.messageParams)}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="quiet" onClick={() => settle(false)} data-testid="workspace-confirm-cancel">
            {t("pages.workspace.confirm.cancel")}
          </Button>
          <Button
            variant={state.tone === "danger" ? "destructive" : "cta"}
            onClick={() => settle(true)}
            data-testid="workspace-confirm-accept"
          >
            {t(state.confirmKey)}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )

  return { confirm, confirmOpen: state.open, dialog }
}
