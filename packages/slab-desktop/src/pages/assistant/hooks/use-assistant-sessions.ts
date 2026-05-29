import { useCallback, useEffect, useMemo, useRef } from "react"
import { toast } from "sonner"

import type { components } from "@slab/api/v1"
import { useTranslation } from "@slab/i18n"
import api from "@slab/api"
import { useAssistantUiStore } from "@/store/useAssistantUiStore"

import { getAssistantErrorDescription } from "../assistant-context"

type SessionRecord = components["schemas"]["SessionResponse"]

export type AssistantConversationItem = {
  key: string
  label: string
  group: string
}

type CreateSessionOptions = {
  quiet?: boolean
  select?: boolean
}

function toConversationItem(
  session: SessionRecord,
  sessionLabels: Record<string, string>,
  defaults: {
    newChat: string
    workspace: string
  }
): AssistantConversationItem {
  const storedLabel = sessionLabels[session.id]?.trim() || null
  const backendLabel = session.name.trim()

  return {
    key: session.id,
    label: backendLabel || storedLabel || defaults.newChat,
    group: defaults.workspace,
  }
}

function toSessionRecords(data: SessionRecord[] | undefined): SessionRecord[] {
  return Array.isArray(data) ? data : []
}

export function useAssistantSessions() {
  const { t } = useTranslation()
  const hasHydrated = useAssistantUiStore((state) => state.hasHydrated)
  const currentSessionId = useAssistantUiStore((state) => state.currentSessionId)
  const setCurrentSessionId = useAssistantUiStore((state) => state.setCurrentSessionId)
  const sessionLabels = useAssistantUiStore((state) => state.sessionLabels)
  const setSessionLabel = useAssistantUiStore((state) => state.setSessionLabel)
  const removeSessionLabel = useAssistantUiStore((state) => state.removeSessionLabel)
  const hasBootstrappedSessions = useRef(false)

  const { data: sessionData, isLoading: isSessionsLoading, refetch: refetchSessions } = api.useQuery(
    "get",
    "/v1/sessions"
  )
  const createSessionMutation = api.useMutation("post", "/v1/sessions")
  const updateSessionMutation = api.useMutation("put", "/v1/sessions/{id}")
  const deleteSessionMutation = api.useMutation("delete", "/v1/sessions/{id}")

  const sessionRecords = useMemo(() => toSessionRecords(sessionData), [sessionData])
  const localizedDefaults = useMemo(
    () => ({
      newChat: t("pages.assistant.runtime.newChat"),
      workspace: t("pages.assistant.runtime.workspace"),
    }),
    [t]
  )
  const conversationList = useMemo(
    () => sessionRecords.map((session) => toConversationItem(session, sessionLabels, localizedDefaults)),
    [localizedDefaults, sessionLabels, sessionRecords]
  )

  const createSession = useCallback(
    async (options?: CreateSessionOptions) => {
      try {
        const session = await createSessionMutation.mutateAsync({
          body: {},
        })

        await refetchSessions()

        if (options?.select ?? true) {
          setCurrentSessionId(session.id)
        }

        return session
      } catch (error) {
        if (!options?.quiet) {
          toast.error(t("pages.assistant.toast.failedToCreateSession"), {
            description: getAssistantErrorDescription(error, t("pages.assistant.toast.unknownError")),
          })
        }

        return null
      }
    },
    [createSessionMutation, refetchSessions, setCurrentSessionId, t]
  )

  const deleteSession = useCallback(
    async (sessionId: string) => {
      try {
        await deleteSessionMutation.mutateAsync({
          params: {
            path: { id: sessionId },
          },
        })
      } catch (error) {
        toast.error(t("pages.assistant.toast.failedToDeleteSession"), {
          description: getAssistantErrorDescription(error, t("pages.assistant.toast.unknownError")),
        })
        return false
      }

      removeSessionLabel(sessionId)

      const refreshed = await refetchSessions()
      const nextSessions = toSessionRecords(refreshed.data)

      if (nextSessions.length === 0) {
        return Boolean(await createSession({ quiet: true, select: true }))
      }

      if (sessionId === currentSessionId) {
        setCurrentSessionId(nextSessions[0]?.id ?? "")
      }

      return true
    },
    [
      createSession,
      currentSessionId,
      deleteSessionMutation,
      refetchSessions,
      removeSessionLabel,
      setCurrentSessionId,
      t,
    ]
  )

  const updateSessionLabel = useCallback(
    async (sessionId: string, label: string) => {
      const trimmedSessionId = sessionId.trim()
      const trimmedLabel = label.trim()

      if (!trimmedSessionId || !trimmedLabel) {
        return false
      }

      setSessionLabel(trimmedSessionId, trimmedLabel)

      try {
        await updateSessionMutation.mutateAsync({
          params: {
            path: { id: trimmedSessionId },
          },
          body: {
            name: trimmedLabel,
          },
        })
        await refetchSessions()
        return true
      } catch (error) {
        toast.error(t("pages.assistant.toast.failedToUpdateSession"), {
          description: getAssistantErrorDescription(error, t("pages.assistant.toast.unknownError")),
        })
        return false
      }
    },
    [refetchSessions, setSessionLabel, t, updateSessionMutation]
  )

  useEffect(() => {
    if (isSessionsLoading) {
      return
    }

    if (sessionRecords.length > 0) {
      hasBootstrappedSessions.current = false
      return
    }

    if (hasBootstrappedSessions.current) {
      return
    }

    hasBootstrappedSessions.current = true
    void createSession({ quiet: true, select: true })
  }, [createSession, isSessionsLoading, sessionRecords.length])

  useEffect(() => {
    if (!hasHydrated || isSessionsLoading || conversationList.length === 0) {
      return
    }

    if (conversationList.some((item) => item.key === currentSessionId)) {
      return
    }

    const nextConversationKey = conversationList[0]?.key ?? ""

    if (nextConversationKey && nextConversationKey !== currentSessionId) {
      setCurrentSessionId(nextConversationKey)
    }
  }, [
    conversationList,
    currentSessionId,
    hasHydrated,
    isSessionsLoading,
    setCurrentSessionId,
  ])

  return {
    conversationList,
    createSession,
    currentSessionId,
    isCreatingSession: createSessionMutation.isPending,
    isDeletingSession: deleteSessionMutation.isPending,
    isSessionMutating: createSessionMutation.isPending || deleteSessionMutation.isPending,
    isSessionsLoading,
    setCurrentSessionId,
    setSessionLabel,
    updateSessionLabel,
    deleteSession,
  }
}
