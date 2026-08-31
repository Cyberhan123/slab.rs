"use client"

import { useState } from "react"
import { MessageCircleDashedIcon } from "lucide-react"

import { useTranslation } from "@slab/i18n"
import { WorkspaceSelector } from "@slab/ui/components/workspace-selector"
import type { CommandInfo } from "@slab/api/harness"

import Sender, { type SenderSubmitOptions } from "@slab/ui/pages/assistant/components/sender.tsx"
import { useGreeting } from "../hooks/use-greeting"
import type { AssistantConversationItem } from "../hooks/use-assistant-sessions"
import type { WorkspaceSelection } from "../hooks/use-workspace-switch"

type AssistantNewChatLandingProps = {
    /** Server-side active workspace, when one is open. */
    currentWorkspace: { rootPath: string; name: string } | null
    /** Command registry snapshot forwarded into the embedded Sender. */
    commands: CommandInfo[]
    /**
     * Submit the first message. Carries the landing's own workspace selection
     * (the orchestrator applies it after creating the session).
     */
    onSubmit: (
        message: string,
        options: SenderSubmitOptions,
        selection: WorkspaceSelection,
    ) => void | Promise<void>
    submitting: boolean
    /**
     * Seed text claimed from a staged workspace handoff draft (e.g. "Explain
     * this code") — prefilled into the composer for the user to review.
     */
    initialPrompt?: string
    /** Recent conversations offered below the composer (jump back in). */
    conversations: AssistantConversationItem[]
    onSelectConversation: (key: string) => void
    conversationsBusy: boolean
}

/** How many recent conversations the landing surfaces (full list: session sheet). */
const MAX_RECENT_CONVERSATIONS = 6

/**
 * The assistant homepage ("新建聊天"): pick where the chat runs (current /
 * recent / picked folder / global) and type the first message. Submitting
 * navigates into the conversation detail (`/?session=<id>`) where the staged
 * draft auto-sends once the chat pane is ready.
 *
 * The workspace and global options always coexist: having an active workspace
 * never hides the global one (and vice versa). Both the workspace selection
 * and plan mode are LOCAL on purpose: the landing unmounts when the page
 * enters a conversation, so every visit starts from the 全局 default again.
 */
export function AssistantNewChatLanding({
    currentWorkspace,
    commands,
    onSubmit,
    submitting,
    initialPrompt,
    conversations,
    onSelectConversation,
    conversationsBusy,
}: AssistantNewChatLandingProps) {
    const { t } = useTranslation()
    const greeting = useGreeting()
    // Global (no workspace) is the DEFAULT selection: a new chat starts global
    // unless the user explicitly picks a workspace. The active workspace stays
    // one click away at the top of the list.
    const [selection, setSelection] = useState<WorkspaceSelection>({ kind: "global" })
    // Plan mode for the *new* chat — local until the session exists, then the
    // detail page's per-session state takes over on handoff.
    const [planMode, setPlanMode] = useState(false)
    const recentConversations = conversations.slice(0, MAX_RECENT_CONVERSATIONS)

    return (
        <div
            data-testid="assistant-new-chat-landing"
            className="relative flex min-h-0 flex-1 flex-col overflow-y-auto bg-card"
        >
            <div className="mx-auto flex w-full max-w-2xl flex-1 flex-col items-center justify-center gap-8 px-4 py-10">
                <div className="flex flex-col items-center gap-2 text-center">
                    <MessageCircleDashedIcon className="size-8 text-muted-foreground" />
                    <p className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
                        {t("pages.assistant.newChat.title")}
                    </p>
                    <h1 className="text-2xl font-semibold">{greeting}</h1>
                    <p className="text-sm text-muted-foreground">
                        {t("pages.assistant.newChat.description")}
                    </p>
                </div>

                <div className="flex w-full flex-col gap-3">
                    <div className="flex items-center justify-center gap-2">
                        <span className="text-muted-foreground text-xs">
                            {t("pages.assistant.newChat.workspaceSection")}
                        </span>
                        <WorkspaceSelector
                            value={selection}
                            onValueChange={setSelection}
                            currentWorkspace={currentWorkspace}
                            busy={submitting}
                        />
                    </div>
                    <Sender
                        onSubmit={(message, options) => onSubmit(message, options, selection)}
                        loading={submitting}
                        commands={commands}
                        planMode={planMode}
                        onPlanModeChange={setPlanMode}
                        initialValue={initialPrompt}
                    />
                </div>

                {recentConversations.length > 0 ? (
                    <div className="flex w-full flex-col gap-2">
                        <p className="text-muted-foreground text-xs font-medium">
                            {t("pages.assistant.newChat.recentSection")}
                        </p>
                        <div className="flex flex-col gap-1.5">
                            {recentConversations.map((conversation) => (
                                <button
                                    key={conversation.key}
                                    type="button"
                                    disabled={conversationsBusy}
                                    data-testid={`assistant-landing-session-${conversation.key}`}
                                    className="workspace-soft-panel flex min-w-0 items-center justify-between gap-3 rounded-xl px-4 py-2.5 text-left transition-colors hover:bg-glass-bg-strong disabled:pointer-events-none disabled:opacity-50"
                                    onClick={() => onSelectConversation(conversation.key)}
                                >
                                    <span className="min-w-0 truncate text-sm font-medium">
                                        {conversation.label}
                                    </span>
                                    <span className="shrink-0 truncate text-xs text-muted-foreground">
                                        {conversation.group}
                                    </span>
                                </button>
                            ))}
                        </div>
                    </div>
                ) : null}
            </div>
        </div>
    )
}
