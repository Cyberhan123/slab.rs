"use client"

import { useCallback, useEffect, useState } from "react"
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
    } = useAssistantSessions()

    const {
        modelOptions,
        selectedModel,
        selectedModelId,
        setSelectedModelId,
        modelLoading,
        isPreparingModel,
        ensureAssistantModelReady,
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
    } = useHarnessConversation(curConversation, selectedModelId || "slab-llama")

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

    useAssistantHeader({
        modelOptions,
        selectedModelId,
        modelLoading,
        isSessionBusy,
        isSessionBootstrapping,
        pendingModelSwitchId,
        onModelPickerChange: handleModelPickerChange,
        onOpenSessionSheet: openSessionSheet,
    })

    useEffect(() => {
        if (!harnessError) {
            return
        }

        toast.error(t("pages.assistant.toast.failedToLoadSession"), {
            description: getAssistantErrorDescription(
                new Error(harnessError),
                t("pages.assistant.toast.unknownError"),
                t
            ),
        })
    }, [harnessError, t])

    const handleBeforeSubmit = useCallback(
        async (value: string) => {
            if (!curConversation || isSessionBusy || isSessionBootstrapping) {
                toast.info(t("pages.assistant.toast.sessionSyncing"))
                throw new Error("Assistant session is not ready.")
            }

            await ensureAssistantModelReady()
            void setConversationLabelIfNeeded(curConversation, value)
        },
        [
            curConversation,
            ensureAssistantModelReady,
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
