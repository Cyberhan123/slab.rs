"use client"

import { useChat } from "@ai-sdk/react"
import type { UIMessage } from "ai"
import { MessageCircleDashedIcon } from "lucide-react"
import { useCallback, useEffect, useMemo, useState } from "react"
import { toast } from "sonner"

import api from "@slab/api"
import {
    DEFAULT_ASSISTANT_LABELS,
    LEGACY_DEFAULT_CHAT_LABELS,
    getResolvedAppLanguage,
    useTranslation,
} from "@slab/i18n"
import {
    Card,
    CardContent,
    CardFooter,
} from "@slab/components/card"
import {
    Empty,
    EmptyDescription,
    EmptyHeader,
    EmptyMedia,
    EmptyTitle,
} from "@slab/components/empty"
import {
    MessageScrollerProvider,
} from "@slab/components/message-scroller"

import { useAiModel } from "@/hooks/use-ai-model"
import { useHeader } from "@/hooks/use-header"
import { HEADER_SELECT_KEYS } from "@/layouts/header"

import MessageList from "@/pages/assistant/components/message/index.tsx"
import Sender from "@/pages/assistant/components/sender.tsx"
import {
    getAssistantErrorDescription,
    getAssistantMessageTextContent,
    type AgentHistoryResponse,
} from "./assistant-context"
import { AssistantModelSwitchDialog } from "./components/assistant-model-switch-dialog"
import { AssistantSessionSheet } from "./components/assistant-session-sheet"
import { useGreeting } from "./hooks/use-greeting"
import { useAssistantSessions } from "./hooks/use-assistant-sessions"
import { projectRestoreSession } from "./lib/openai-responses"
import {
    createConversationLabel,
    getSelectedModelStatusLabel,
    resolveAssistantModelCapabilities,
    type ModelOption,
    type ModelRuntimeStatus,
} from "./lib/assistant-page-state"
import { createChat } from "./lib/message-provider"

type AssistantChatPaneProps = {
    disabled: boolean
    initialMessages: UIMessage[]
    isHistoryLoading: boolean
    modelStatusLabel: string
    onBeforeSubmit: (value: string) => Promise<void>
    onBusyChange: (busy: boolean) => void
    onMessageCountChange: (count: number) => void
    transport: ReturnType<ReturnType<typeof createChat>["transport"]>
}

function toChatMessages(response: AgentHistoryResponse): UIMessage[] {
    return projectRestoreSession(response.messages, response.responses)
        .map((record): UIMessage | null => {
            const text = getAssistantMessageTextContent(record.message).trim()

            if (!text) {
                return null
            }

            return {
                id: String(record.id),
                parts: [{ text, type: "text" }],
                role: record.message.role === "assistant" ? "assistant" : "user",
            } satisfies UIMessage
        })
        .filter((message): message is UIMessage => Boolean(message))
}

function AssistantChatPane({
    disabled,
    initialMessages,
    isHistoryLoading,
    modelStatusLabel,
    onBeforeSubmit,
    onBusyChange,
    onMessageCountChange,
    transport,
}: AssistantChatPaneProps) {
    const { t } = useTranslation()
    const { messages, sendMessage, status } = useChat({
        messages: initialMessages,
        transport,
    })
    const isBusy = status === "submitted" || status === "streaming"
    const greeting = useGreeting()

    useEffect(() => {
        onBusyChange(isBusy)
    }, [isBusy, onBusyChange])

    useEffect(() => {
        onMessageCountChange(messages.length)
    }, [messages.length, onMessageCountChange])

    return (
        <MessageScrollerProvider>
            <div className="relative flex min-h-0 flex-1 flex-col bg-[var(--shell-card)]">
                <Card className="h-full w-full gap-0 border-none shadow-none">
                    <CardContent className="flex-1 overflow-hidden p-0">
                        {isHistoryLoading && messages.length === 0 ? (
                            <Empty className="h-full" data-testid="assistant-loading-state">
                                <EmptyHeader>
                                    <EmptyMedia variant="icon">
                                        <MessageCircleDashedIcon />
                                    </EmptyMedia>
                                    <EmptyTitle>{t("pages.assistant.loading.title")}</EmptyTitle>
                                    <EmptyDescription>
                                        {t("pages.assistant.loading.description")}
                                    </EmptyDescription>
                                </EmptyHeader>
                            </Empty>
                        ) : messages.length === 0 ? (
                            <Empty className="h-full" data-testid="assistant-empty-state">
                                <EmptyHeader>
                                    <EmptyMedia variant="icon">
                                        <MessageCircleDashedIcon />
                                    </EmptyMedia>
                                    <EmptyTitle>{greeting}</EmptyTitle>
                                    <EmptyDescription>
                                        {t("pages.assistant.hero.description")}
                                    </EmptyDescription>
                                </EmptyHeader>
                            </Empty>
                        ) : (
                            <MessageList messages={messages} isBusy={isBusy} />
                        )}
                    </CardContent>
                    <CardFooter className="flex-col gap-2">
                        <Sender
                            onSubmit={async (value) => {
                                await onBeforeSubmit(value)
                                sendMessage({ text: value })
                            }}
                            loading={disabled || isBusy}
                        />
                        <p
                            className="w-full truncate text-xs text-muted-foreground"
                            data-testid="assistant-model-status"
                        >
                            {modelStatusLabel}
                        </p>
                    </CardFooter>
                </Card>
            </div>
        </MessageScrollerProvider>
    )
}

function Assistant() {
    const { t } = useTranslation()
    const [isSessionSheetOpen, setIsSessionSheetOpen] = useState(false)
    const [pendingModelSwitchId, setPendingModelSwitchId] = useState<string | null>(null)
    const [loadedModelStatus, setLoadedModelStatus] = useState<ModelRuntimeStatus | null>(null)
    const [restoredMessages, setRestoredMessages] = useState<UIMessage[]>([])
    const [restoredThreadId, setRestoredThreadId] = useState<string | null>(null)
    const [restoreVersion, setRestoreVersion] = useState(0)
    const [isHistoryLoading, setIsHistoryLoading] = useState(false)
    const [activeConversation, setActiveConversation] = useState<string>()
    const [isChatBusy, setIsChatBusy] = useState(false)
    const [messageCount, setMessageCount] = useState(0)
    const resolvedLanguage = getResolvedAppLanguage()

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
        data: restoredSession,
        error: restoreSessionError,
        isLoading: isRestoreSessionLoading,
    } = api.useQuery(
        "get",
        "/v1/sessions/{id}/agent-history",
        {
            params: {
                path: { id: curConversation ?? "" },
            },
        },
        {
            enabled: Boolean(curConversation),
            meta: {
                skipGlobalErrorToast: true,
            },
        }
    )

    const assistantModels = useAiModel({
        capability: "chat_generation",
        storageKey: HEADER_SELECT_KEYS.assistantModel,
        includeCloud: true,
    })

    const modelOptions = useMemo<ModelOption[]>(
        () =>
            assistantModels.models.map((model) => {
                const downloaded =
                    model.kind === "cloud" ||
                    (model.status === "ready" &&
                        typeof model.local_path === "string" &&
                        model.local_path.length > 0)

                return {
                    capabilities: resolveAssistantModelCapabilities(model),
                    contextWindow: model.spec.context_window ?? null,
                    downloaded,
                    id: model.id,
                    label: model.display_name,
                    pending: model.pending,
                    runtimePresets: model.runtime_presets ?? null,
                    source: model.kind,
                }
            }),
        [assistantModels.models]
    )
    const selectedModelId = assistantModels.selectedId
    const setSelectedModelId = assistantModels.setSelectedId
    const selectedModel = useMemo(
        () => modelOptions.find((item) => item.id === selectedModelId),
        [modelOptions, selectedModelId]
    )
    const pendingModelSwitch = useMemo(
        () => modelOptions.find((item) => item.id === pendingModelSwitchId) ?? null,
        [modelOptions, pendingModelSwitchId]
    )

    const modelLoading = assistantModels.loading
    const isPreparingModel = assistantModels.status.busy
    const isSessionBootstrapping = (sessionsLoading || isCreatingSession) && conversationList.length === 0
    const isSessionBusy = isChatBusy || isPreparingModel || isHistoryLoading || isSessionMutating
    const selectedRuntimeContextLength = loadedModelStatus?.context_length ?? null
    const currentConversationLabel =
        conversationList.find((item) => item.key === curConversation)?.label?.trim() ||
        t("pages.assistant.sessionSummary.currentSession")

    useEffect(() => {
        setIsHistoryLoading(isRestoreSessionLoading)
    }, [isRestoreSessionLoading])

    useEffect(() => {
        if (!restoreSessionError) {
            return
        }

        toast.error(t("pages.assistant.toast.failedToLoadSession"), {
            description: getAssistantErrorDescription(
                restoreSessionError,
                t("pages.assistant.toast.unknownError"),
                t
            ),
        })
    }, [restoreSessionError, t])

    useEffect(() => {
        if (!curConversation) {
            setRestoredMessages([])
            setRestoredThreadId(null)
            setActiveConversation(undefined)
            setMessageCount(0)
            setRestoreVersion((value) => value + 1)
            return
        }

        if (!restoredSession || restoredSession.session_id !== curConversation) {
            setRestoredMessages([])
            setRestoredThreadId(null)
            setActiveConversation(undefined)
            setMessageCount(0)
            return
        }

        const nextMessages = toChatMessages(restoredSession)
        setActiveConversation(restoredSession.session_id)
        setRestoredMessages(nextMessages)
        setRestoredThreadId(restoredSession.thread?.id ?? null)
        setMessageCount(nextMessages.length)
        setRestoreVersion((value) => value + 1)
    }, [curConversation, restoredSession])

    const prepareSelectedModel = useCallback(async () => {
        if (!selectedModelId) {
            throw new Error(t("pages.assistant.error.selectModelFirst"))
        }

        const selectedOption = modelOptions.find((item) => item.id === selectedModelId)
        if (!selectedOption) {
            throw new Error(t("pages.assistant.error.selectedModelUnavailable"))
        }

        if (selectedOption.source === "cloud") {
            setLoadedModelStatus(null)
            return
        }

        const selectedLocal = assistantModels.localModels.find((item) => item.id === selectedModelId)
        const { downloadedNow } = await assistantModels.ensureDownloaded(selectedModelId)

        if (downloadedNow) {
            toast.success(
                t("pages.assistant.toast.downloaded", {
                    model: selectedLocal?.display_name ?? selectedModelId,
                })
            )
        }

        try {
            const status = await assistantModels.ensureLoaded(selectedModelId)
            if (status.runtimeStatus) {
                setLoadedModelStatus(status.runtimeStatus)
            }
        } catch (firstLoadError) {
            if (downloadedNow) {
                throw firstLoadError
            }

            toast.message(t("pages.assistant.toast.modelLoadRetry"))

            const retry = await assistantModels.ensureDownloaded(selectedModelId, { forceDownload: true })
            if (retry.downloadedNow) {
                toast.success(
                    t("pages.assistant.toast.downloaded", {
                        model: selectedLocal?.display_name ?? selectedModelId,
                    })
                )
            }

            const status = await assistantModels.ensureLoaded(selectedModelId)
            if (status.runtimeStatus) {
                setLoadedModelStatus(status.runtimeStatus)
            }
        }
    }, [assistantModels, modelOptions, selectedModelId, t])

    const ensureAssistantModelReady = useCallback(async () => {
        try {
            await prepareSelectedModel()
        } catch (error) {
            toast.error(t("pages.assistant.toast.failedToPrepareModel"), {
                description: getAssistantErrorDescription(error, t("pages.assistant.toast.unknownError"), t),
            })
            throw error
        }
    }, [prepareSelectedModel, t])

    const sortedConversations = useMemo(() => {
        const currentConversation = conversationList.find((item) => item.key === curConversation)
        const remainingConversations = conversationList.filter((item) => item.key !== curConversation)

        return currentConversation
            ? [currentConversation, ...remainingConversations]
            : remainingConversations
    }, [conversationList, curConversation])

    const setConversationLabelIfNeeded = useCallback(
        async (conversationKey: string, prompt: string) => {
            const conversation = conversationList.find((item) => item.key === conversationKey)
            const label = conversation?.label ?? t("pages.assistant.runtime.newChat")
            const defaultLabels = new Set([
                t("pages.assistant.runtime.newChat"),
                t("pages.assistant.runtime.newConversation"),
                ...DEFAULT_ASSISTANT_LABELS,
                ...LEGACY_DEFAULT_CHAT_LABELS,
            ])

            if (!defaultLabels.has(label)) {
                return
            }

            const nextLabel = createConversationLabel(prompt, t("pages.assistant.runtime.newChat"))
            if (nextLabel) {
                await updateSessionLabel(conversationKey, nextLabel)
            }
        },
        [conversationList, t, updateSessionLabel]
    )

    const handleModelPickerChange = useCallback(
        (nextModelId: string) => {
            if (!nextModelId || nextModelId === selectedModelId) {
                return
            }

            if (isSessionBusy || isSessionBootstrapping) {
                toast.info(t("pages.assistant.toast.waitBeforeSwitchingModels"))
                return
            }

            if (!curConversation || messageCount === 0) {
                setSelectedModelId(nextModelId)
                return
            }

            setPendingModelSwitchId(nextModelId)
        },
        [
            curConversation,
            isSessionBootstrapping,
            isSessionBusy,
            messageCount,
            selectedModelId,
            setSelectedModelId,
            t,
        ]
    )

    const headerModelPicker = useMemo(
        () => ({
            disabled:
                modelLoading ||
                isSessionBusy ||
                isSessionBootstrapping ||
                Boolean(pendingModelSwitchId) ||
                modelOptions.length === 0,
            emptyLabel: t("pages.assistant.modelPicker.emptyLabel"),
            groupLabel: t("pages.assistant.modelPicker.groupLabel"),
            loading: modelLoading,
            onChange: handleModelPickerChange,
            options: modelOptions.map((model) => ({
                id: model.id,
                label: model.label,
            })),
            placeholder: t("pages.assistant.modelPicker.placeholder"),
            value: selectedModelId,
        }),
        [
            handleModelPickerChange,
            isSessionBootstrapping,
            isSessionBusy,
            modelLoading,
            modelOptions,
            pendingModelSwitchId,
            selectedModelId,
            t,
        ]
    )

    const headerHistoryButton = useMemo(
        () => ({
            ariaLabel: t("pages.assistant.sessionSheet.title"),
            disabled: isSessionBootstrapping,
            onClick: () => setIsSessionSheetOpen(true),
            title: t("pages.assistant.sessionSheet.title"),
        }),
        [isSessionBootstrapping, t]
    )

    useHeader({ history: headerHistoryButton, select: headerModelPicker })

    const selectedModelStatusLabel = useMemo(
        () =>
            getSelectedModelStatusLabel({
                curConversation,
                eventsConnected: Boolean(restoredThreadId) || !isHistoryLoading,
                isCreatingSession,
                isDeletingSession,
                isHistoryLoading,
                isPreparingModel,
                isSessionBootstrapping,
                modelLoading,
                resolvedLanguage,
                selectedModel,
                selectedRuntimeContextLength,
                t,
            }),
        [
            curConversation,
            isCreatingSession,
            isDeletingSession,
            isHistoryLoading,
            isPreparingModel,
            isSessionBootstrapping,
            modelLoading,
            resolvedLanguage,
            restoredThreadId,
            selectedModel,
            selectedRuntimeContextLength,
            t,
        ]
    )

    const transport = useMemo(
        () =>
            createChat().transport({
                model: selectedModelId || "slab-llama",
                sessionId: curConversation || undefined,
                threadId: restoredThreadId,
            }),
        [curConversation, restoredThreadId, selectedModelId]
    )

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

    const handleDeleteConversation = useCallback(
        async (conversationKey: string) => {
            if (isSessionBusy) {
                toast.info(t("pages.assistant.toast.waitBeforeDeletingSessions"))
                return
            }

            await deleteConversationSession(conversationKey)
        },
        [deleteConversationSession, isSessionBusy, t]
    )

    const handleSelectConversation = useCallback(
        (conversationKey: string) => {
            if (conversationKey === curConversation) {
                setIsSessionSheetOpen(false)
                return
            }

            if (isSessionBusy || isSessionBootstrapping) {
                toast.info(t("pages.assistant.toast.sessionSyncing"))
                return
            }

            setCurConversation(conversationKey)
            setIsSessionSheetOpen(false)
        },
        [curConversation, isSessionBootstrapping, isSessionBusy, setCurConversation, t]
    )

    const closePendingModelSwitch = useCallback(() => {
        if (isCreatingSession) {
            return
        }

        setPendingModelSwitchId(null)
    }, [isCreatingSession])

    const handleKeepSessionOnModelSwitch = useCallback(() => {
        if (!pendingModelSwitchId) {
            return
        }

        setSelectedModelId(pendingModelSwitchId)
        setPendingModelSwitchId(null)
    }, [pendingModelSwitchId, setSelectedModelId])

    const handleCreateSessionOnModelSwitch = useCallback(async () => {
        if (!pendingModelSwitchId) {
            return
        }

        const nextModelId = pendingModelSwitchId
        const session = await createEmptySession({ select: true })

        if (!session) {
            return
        }

        setSelectedModelId(nextModelId)
        setPendingModelSwitchId(null)
    }, [createEmptySession, pendingModelSwitchId, setSelectedModelId])

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
