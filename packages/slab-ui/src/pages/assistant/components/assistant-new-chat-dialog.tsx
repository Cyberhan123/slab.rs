"use client"

import { useState } from "react"

import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@slab/components/dialog"
import { useTranslation } from "@slab/i18n"
import { WorkspaceSelector } from "@slab/ui/components/workspace-selector"
import type { CommandInfo } from "@slab/core/harness"

import Sender, { type SenderSubmitOptions } from "@slab/ui/pages/assistant/components/sender.tsx"
import type { WorkspaceSelection } from "../hooks/use-workspace-switch"

type AssistantNewChatDialogProps = {
    open: boolean
    onOpenChange: (open: boolean) => void
    /** Controlled workspace selection (owned by the orchestrating hook). */
    selection: WorkspaceSelection
    onSelectionChange: (selection: WorkspaceSelection) => void
    /** Server-side active workspace, when one is open. */
    currentWorkspace: { rootPath: string; name: string } | null
    /** Command registry snapshot forwarded into the embedded Sender. */
    commands: CommandInfo[]
    onSubmit: (message: string, options: SenderSubmitOptions) => void | Promise<void>
    submitting: boolean
}

/**
 * New-chat entry: pick where the chat runs (current / recent / picked folder /
 * global) and type the first message in an embedded {@link Sender}. Submitting
 * hands off to the assistant page — the session is created, the workspace is
 * switched if needed, and the draft auto-sends once the chat pane is ready.
 *
 * The workspace and global options always coexist: having an active workspace
 * never hides the global one (and vice versa).
 */
export function AssistantNewChatDialog({
    open,
    onOpenChange,
    selection,
    onSelectionChange,
    currentWorkspace,
    commands,
    onSubmit,
    submitting,
}: AssistantNewChatDialogProps) {
    const { t } = useTranslation()
    // Plan mode for the *new* chat — local until the session exists, then the
    // page's per-session state takes over on handoff.
    const [planMode, setPlanMode] = useState(false)

    return (
        <Dialog open={open} onOpenChange={onOpenChange}>
            <DialogContent data-testid="assistant-new-chat-dialog" className="sm:max-w-xl">
                <DialogHeader>
                    <DialogTitle>{t("pages.assistant.newChat.title")}</DialogTitle>
                    <DialogDescription>{t("pages.assistant.newChat.description")}</DialogDescription>
                </DialogHeader>
                <div className="flex flex-col gap-3">
                    <div className="flex items-center gap-2">
                        <span className="text-muted-foreground text-xs">
                            {t("pages.assistant.newChat.workspaceSection")}
                        </span>
                        <WorkspaceSelector
                            value={selection}
                            onValueChange={onSelectionChange}
                            currentWorkspace={currentWorkspace}
                            busy={submitting}
                        />
                    </div>
                    <Sender
                        onSubmit={onSubmit}
                        loading={submitting}
                        commands={commands}
                        planMode={planMode}
                        onPlanModeChange={setPlanMode}
                    />
                </div>
            </DialogContent>
        </Dialog>
    )
}
