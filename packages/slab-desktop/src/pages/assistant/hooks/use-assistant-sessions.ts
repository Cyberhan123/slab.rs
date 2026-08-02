import { useCallback, useEffect, useMemo, useRef } from "react"
import { toast } from "sonner"

import type { components } from "@slab/api/v1"
import { useTranslation } from "@slab/i18n"
import api from "@slab/api"
import { useAssistantUiStore } from "@/store/useAssistantUiStore"
import { GUARDRAIL_PMIDS, useGuardrailFlag } from "@/lib/guardrail-flags"

import { getAssistantErrorDescription } from "../lib/assistant-request-errors"

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

export function useAssistantSessions({ lockedSessionId }: { lockedSessionId?: string } = {}) {
  const trimmedLock = lockedSessionId?.trim() || undefined
  const { t } = useTranslation()
  const assistantErrorEnvelopeRenderingEnabled = useGuardrailFlag(
    GUARDRAIL_PMIDS.assistantErrorEnvelopeRendering
  )
  const hasHydrated = useAssistantUiStore((state) => state.hasHydrated)
  const storeCurrentSessionId = useAssistantUiStore((state) => state.currentSessionId)
  const setStoreCurrentSessionId = useAssistantUiStore((state) => state.setCurrentSessionId)
  // When locked to a `?session=` override, surface it as the current session and
  // make setCurrentSessionId a no-op so the shared global `zustand:assistant-ui`
  // store is never mutated (concurrent e2e browsers each bind their own session).
  // The override also gates the bootstrap/fallback effects below.
  const currentSessionId = trimmedLock ?? storeCurrentSessionId
  const setCurrentSessionId = useCallback(
    (next: string) => {
      if (trimmedLock) {
        return
      }
      setStoreCurrentSessionId(next)
    },
    [setStoreCurrentSessionId, trimmedLock]
  )
  const sessionLabels = useAssistantUiStore((state) => state.sessionLabels)
  const setSessionLabel = useAssistantUiStore((state) => state.setSessionLabel)
  const removeSessionLabel = useAssistantUiStore((state) => state.removeSessionLabel)
  const hasBootstrappedSessions = useRef(false)

  const { data: sessionData, isLoading: isSessionsLoading, refetch: refetchSessions } = api.useQuery(
    "get",
    "/v1/sessions"
  )
  const createSessionMutation = api.useMutation("post", "/v1/sessions", {
    meta: {
      skipGlobalErrorToast: true,
    },
  })
  const updateSessionMutation = api.useMutation("put", "/v1/sessions/{id}", {
    meta: {
      skipGlobalErrorToast: true,
    },
  })
  const deleteSessionMutation = api.useMutation("delete", "/v1/sessions/{id}", {
    meta: {
      skipGlobalErrorToast: true,
    },
  })

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
            description: getAssistantErrorDescription(error, t("pages.assistant.toast.unknownError"), t, {
              preferServerEnvelope: assistantErrorEnvelopeRenderingEnabled,
            }),
          })
        }

        return null
      }
    },
    [
      assistantErrorEnvelopeRenderingEnabled,
      createSessionMutation,
      refetchSessions,
      setCurrentSessionId,
      t,
    ]
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
          description: getAssistantErrorDescription(error, t("pages.assistant.toast.unknownError"), t, {
            preferServerEnvelope: assistantErrorEnvelopeRenderingEnabled,
          }),
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
      assistantErrorEnvelopeRenderingEnabled,
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
          description: getAssistantErrorDescription(error, t("pages.assistant.toast.unknownError"), t, {
            preferServerEnvelope: assistantErrorEnvelopeRenderingEnabled,
          }),
        })
        return false
      }
    },
    [
      assistantErrorEnvelopeRenderingEnabled,
      refetchSessions,
      setSessionLabel,
      t,
      updateSessionMutation,
    ]
  )

  useEffect(() => {
    if (trimmedLock) {
      return
    }

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
  }, [createSession, isSessionsLoading, sessionRecords.length, trimmedLock])

  useEffect(() => {
    if (trimmedLock) {
      return
    }

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
    trimmedLock,
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
