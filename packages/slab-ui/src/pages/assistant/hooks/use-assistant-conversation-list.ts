import { useCallback, useMemo } from "react"
import { toast } from "sonner"

import {
  DEFAULT_ASSISTANT_LABELS,
  LEGACY_DEFAULT_CHAT_LABELS,
  useTranslation,
} from "@slab/i18n"

import { createConversationLabel } from "../lib/assistant-page-state"
import type { AssistantConversationItem } from "./use-assistant-sessions"

type UseAssistantConversationListOptions = {
  conversationList: AssistantConversationItem[]
  curConversation: string | undefined
  setCurConversation: (id: string) => void
  deleteSession: (sessionId: string) => Promise<boolean>
  updateSessionLabel: (sessionId: string, label: string) => Promise<boolean>
  isSessionBusy: boolean
  isSessionBootstrapping: boolean
  setIsSessionSheetOpen: (open: boolean) => void
  /**
   * Invoked after the CURRENT conversation was deleted (the page navigates —
   * e.g. back to the new-chat landing when the detail's session is gone).
   */
  onCurrentConversationDeleted?: () => void
}

export function useAssistantConversationList({
  conversationList,
  curConversation,
  setCurConversation,
  deleteSession,
  updateSessionLabel,
  isSessionBusy,
  isSessionBootstrapping,
  setIsSessionSheetOpen,
  onCurrentConversationDeleted,
}: UseAssistantConversationListOptions) {
  const { t } = useTranslation()

  const sortedConversations = useMemo(() => {
    const currentConversation = conversationList.find((item) => item.key === curConversation)
    const remainingConversations = conversationList.filter((item) => item.key !== curConversation)

    return currentConversation
      ? [currentConversation, ...remainingConversations]
      : remainingConversations
  }, [conversationList, curConversation])

  const currentConversationLabel = useMemo(
    () =>
      conversationList.find((item) => item.key === curConversation)?.label?.trim() ||
      t("pages.assistant.sessionSummary.currentSession"),
    [conversationList, curConversation, t]
  )

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

  const handleDeleteConversation = useCallback(
    async (conversationKey: string) => {
      if (isSessionBusy) {
        toast.info(t("pages.assistant.toast.waitBeforeDeletingSessions"))
        return
      }

      const deleted = await deleteSession(conversationKey)
      if (deleted && conversationKey === curConversation) {
        onCurrentConversationDeleted?.()
      }
    },
    [curConversation, deleteSession, isSessionBusy, onCurrentConversationDeleted, t]
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
    [
      curConversation,
      isSessionBootstrapping,
      isSessionBusy,
      setCurConversation,
      setIsSessionSheetOpen,
      t,
    ]
  )

  return {
    sortedConversations,
    currentConversationLabel,
    setConversationLabelIfNeeded,
    handleSelectConversation,
    handleDeleteConversation,
  }
}
