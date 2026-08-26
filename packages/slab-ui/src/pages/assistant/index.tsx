"use client"

import { useCallback, useEffect, useRef, useState } from "react"
import { useSearchParams } from "react-router-dom"
import { toast } from "sonner"

import { useTranslation } from "@slab/i18n"

import { AssistantChatPane } from "./components/assistant-chat-pane"
import { AssistantModelSwitchDialog } from "./components/assistant-model-switch-dialog"
import { AssistantSessionSheet } from "./components/assistant-session-sheet"
import { getAssistantErrorDescription } from "./lib/assistant-request-errors"
import { useAssistantConversationList } from "./hooks/use-assistant-conversation-list"
import { useAssistantHeader } from "./hooks/use-assistant-header"
import { useAssistantModel } from "./hooks/use-assistant-model"
import { useAssistantModelStatusLabel } from "./hooks/use-assistant-model-status-label"
import { useAssistantModelSwitch } from "./hooks/use-assistant-model-switch"
import { useAssistantSessions } from "./hooks/use-assistant-sessions"
import { useHarnessConversation } from "./hooks/use-harness-conversation"

function Assistant() {
    const { t } = useTranslation()
    const [isSessionSheetOpen, setIsSessionSheetOpen] = useState(false)
    const [isChatBusy, setIsChatBusy] = useState(false)
    const [messageCount, setMessageCount] = useState(0)
    // `?session=<id>` deep link pins this page to a specific session, bypassing
    // the shared `zustand:assistant-ui` "current session" (which is global per
    // server and would race across concurrent e2e browsers). Absent the param,
    // behavior is unchanged.
    const [searchParams] = useSearchParams()
    const sessionOverride = searchParams.get("session")

    const {
        conversationList,
        createSession: createEmptySession,
        currentSessionId: curConversation,
        deleteSession: deleteConversationSession,
        isCreatingSession,
        isDeletingSession,
        isSessionMutating,
        isSessionsLoading: sessionsLoading,
        setCurrentSessionId: setCurConversation,
        updateSessionLabel,
    } = useAssistantSessions({ lockedSessionId: sessionOverride ?? undefined })

    const {
        modelOptions,
        selectedModel,
        selectedModelId,
        setSelectedModelId,
        modelLoading,
        isPreparingModel,
        loadedModelStatus,
    } = useAssistantModel()

    const {
        transport,
        restoredMessages,
        restoredThreadId,
        activeConversation,
        restoreVersion,
        isHistoryLoading,
        error: harnessError,
        actionError: harnessActionError,
        approvals,
        approvalStatusByItemId,
        liveOutputByItemId,
        livePatchByItemId,
        modelLoad,
        turnUsage,
        historyCreatedAt,
        commands,
        compactionMarkers,
        isCompacting,
        isForking,
        resolveApproval,
        compactThread,
        forkThread,
        userMessageTurnIndex,
        rollbackFromTurn,
        planMode,
        setPlanMode,
        threadStatus,
        abortReason,
        queuedCount,
        sendSteering,
    } = useHarnessConversation(curConversation, selectedModelId || "slab-llama")

    // Context window for the usage consumption bar: prefer the runtime's
    // post-load value, fall back to the catalog-declared context window.
    const usageContextWindow =
        loadedModelStatus?.context_length ?? selectedModel?.contextWindow ?? null

    const isSessionBootstrapping = (sessionsLoading || isCreatingSession) && conversationList.length === 0
    const isSessionBusy = isChatBusy || isPreparingModel || isHistoryLoading || isSessionMutating

    const {
        pendingModelSwitchId,
        pendingModelSwitch,
        handleModelPickerChange,
        closePendingModelSwitch,
        handleKeepSessionOnModelSwitch,
        handleCreateSessionOnModelSwitch,
    } = useAssistantModelSwitch({
        modelOptions,
        selectedModelId,
        setSelectedModelId,
        isSessionBusy,
        isSessionBootstrapping,
        curConversation,
        messageCount,
        createSession: createEmptySession,
        isCreatingSession,
    })

    const {
        sortedConversations,
        currentConversationLabel,
        setConversationLabelIfNeeded,
        handleSelectConversation,
        handleDeleteConversation,
    } = useAssistantConversationList({
        conversationList,
        curConversation,
        setCurConversation,
        deleteSession: deleteConversationSession,
        updateSessionLabel,
        isSessionBusy,
        isSessionBootstrapping,
        setIsSessionSheetOpen,
    })

    const { statusLabel: selectedModelStatusLabel } = useAssistantModelStatusLabel({
        curConversation,
        isCreatingSession,
        isDeletingSession,
        isHistoryLoading,
        restoredThreadId,
        isPreparingModel,
        isSessionBootstrapping,
        modelLoading,
        selectedModel,
        loadedModelStatus,
    })

    const openSessionSheet = useCallback(() => setIsSessionSheetOpen(true), [])
    const handleNewSession = useCallback(() => {
        void createEmptySession()
    }, [createEmptySession])

    useAssistantHeader({
        modelOptions,
        selectedModelId,
        modelLoading,
        isSessionBusy,
        isSessionBootstrapping,
        pendingModelSwitchId,
        onModelPickerChange: handleModelPickerChange,
        onOpenSessionSheet: openSessionSheet,
        onNewSession: handleNewSession,
    })

    // Toast a restore failure once per distinct error. `t` is read for the label
    // but intentionally omitted from the deps so a post-mount language change
    // (AppLanguageSync) doesn't re-fire the toast for the same error.
    const lastToastedRestoreErrorRef = useRef<string | null>(null)
    useEffect(() => {
        if (!harnessError) {
            lastToastedRestoreErrorRef.current = null
            return
        }
        if (lastToastedRestoreErrorRef.current === harnessError) return
        lastToastedRestoreErrorRef.current = harnessError
        toast.error(t("pages.assistant.toast.failedToLoadSession"), {
            description: getAssistantErrorDescription(
                new Error(harnessError),
                t("common.toasts.unknownError"),
                // eslint-disable-next-line react-hooks/exhaustive-deps
                t
            ),
        })
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [harnessError])

    // Surface a failed `/compact` or `/fork` action (distinct from a restore
    // failure) so a backend refusal doesn't look like "nothing happened".
    const lastToastedActionRef = useRef<{ kind: string; message: string } | null>(null)
    useEffect(() => {
        if (!harnessActionError) {
            lastToastedActionRef.current = null
            return
        }
        const last = lastToastedActionRef.current
        if (last?.kind === harnessActionError.kind && last.message === harnessActionError.message) {
            return
        }
        lastToastedActionRef.current = {
            kind: harnessActionError.kind,
            message: harnessActionError.message,
        }
        toast.error(
            t(
                harnessActionError.kind === "compact"
                    ? "pages.assistant.toast.compactFailed"
                    : "pages.assistant.toast.forkFailed",
            ),
            { description: harnessActionError.message },
        )
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [harnessActionError])

    const handleBeforeSubmit = useCallback(
        async (value: string) => {
            if (!curConversation || isSessionBusy || isSessionBootstrapping) {
                toast.info(t("pages.assistant.toast.sessionSyncing"))
                throw new Error("Assistant session is not ready.")
            }

            // NOTE: model loading is now server-driven inside `turn/start`
            // (streaming `model/load/*` notifications rendered by the in-stream
            // model-load Marker), so there is no HTTP pre-flight here.
            void setConversationLabelIfNeeded(curConversation, value)
        },
        [
            curConversation,
            isSessionBootstrapping,
            isSessionBusy,
            setConversationLabelIfNeeded,
            t,
        ]
    )

    return (
        <>
            <AssistantChatPane
                key={`${curConversation ?? "none"}:${restoreVersion}`}
                disabled={isSessionBootstrapping || isHistoryLoading || isSessionMutating || !curConversation}
                initialMessages={restoredMessages}
                isHistoryLoading={isHistoryLoading}
                modelStatusLabel={selectedModelStatusLabel}
                onBeforeSubmit={handleBeforeSubmit}
                onBusyChange={setIsChatBusy}
                onMessageCountChange={setMessageCount}
                transport={transport}
                approvals={approvals}
                approvalStatusByItemId={approvalStatusByItemId}
                liveOutputByItemId={liveOutputByItemId}
                livePatchByItemId={livePatchByItemId}
                modelLoad={modelLoad}
                turnUsage={turnUsage}
                contextWindow={usageContextWindow}
                resolveApproval={resolveApproval}
                onCompact={() => compactThread()}
                onFork={() => forkThread()}
                historyCreatedAt={historyCreatedAt}
                commands={commands}
                compactionMarkers={compactionMarkers}
                isCompacting={isCompacting}
                isForking={isForking}
                userMessageTurnIndex={userMessageTurnIndex}
                onRollbackFromTurn={rollbackFromTurn}
                planMode={planMode}
                onPlanModeChange={setPlanMode}
                threadStatus={threadStatus}
                abortReason={abortReason}
                queuedCount={queuedCount}
                onSteerSubmit={(text, options) =>
                    sendSteering(
                        {
                            id: `steer-${Date.now()}`,
                            role: "user",
                            parts: [{ type: "text", text }],
                        },
                        options,
                    )
                }
            />

            <AssistantSessionSheet
                open={isSessionSheetOpen}
                onOpenChange={setIsSessionSheetOpen}
                conversations={sortedConversations}
                currentConversation={curConversation}
                activeConversation={activeConversation}
                busy={isSessionBusy || isSessionBootstrapping}
                onSelect={handleSelectConversation}
                onDelete={handleDeleteConversation}
            />

            <AssistantModelSwitchDialog
                conversationLabel={currentConversationLabel}
                isCreatingSession={isCreatingSession}
                messageCount={messageCount}
                onCreateSession={() => void handleCreateSessionOnModelSwitch()}
                onKeepSession={handleKeepSessionOnModelSwitch}
                onOpenChange={(open) => {
                    if (!open) {
                        closePendingModelSwitch()
                    }
                }}
                pendingModelId={pendingModelSwitchId}
                pendingModelLabel={pendingModelSwitch?.label}
                selectedModelLabel={selectedModel?.label}
            />
        </>
    )
}

export default Assistant
