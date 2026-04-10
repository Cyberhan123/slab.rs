import { useCallback, useEffect, useMemo, useRef } from "react"
import { toast } from "sonner"

import type { components } from "@/lib/api/v1.d.ts"
import api from "@/lib/api"
import { useChatUiStore } from "@/store/useChatUiStore"

import { clearConversationCache } from "../chat-context"

type SessionRecord = components["schemas"]["SessionResponse"]

export type ChatConversationItem = {
  key: string
  label: string
  group: string
}

type CreateSessionOptions = {
  quiet?: boolean
  select?: boolean
}

function getErrorDescription(error: unknown) {
  if (error instanceof Error && error.message.trim()) {
    return error.message
  }

  if (typeof error === "object" && error !== null) {
    const message = (error as { error?: unknown; message?: unknown }).message
    if (typeof message === "string" && message.trim()) {
      return message
    }

    const rawError = (error as { error?: unknown }).error
    if (typeof rawError === "string" && rawError.trim()) {
      return rawError
    }
  }

  return "Unknown error"
}

function toConversationItem(
  session: SessionRecord,
  sessionLabels: Record<string, string>
): ChatConversationItem {
  const storedLabel = sessionLabels[session.id]?.trim() || null
  const backendLabel = session.name.trim()

  return {
    key: session.id,
    label: storedLabel ?? (backendLabel || "New chat"),
    group: "Workspace",
  }
}

function toSessionRecords(data: SessionRecord[] | undefined): SessionRecord[] {
  return Array.isArray(data) ? data : []
}

export function useChatSessions() {
  const hasHydrated = useChatUiStore((state) => state.hasHydrated)
  const currentSessionId = useChatUiStore((state) => state.currentSessionId)
  const setCurrentSessionId = useChatUiStore((state) => state.setCurrentSessionId)
  const sessionLabels = useChatUiStore((state) => state.sessionLabels)
  const setSessionLabel = useChatUiStore((state) => state.setSessionLabel)
  const removeSessionLabel = useChatUiStore((state) => state.removeSessionLabel)
  const hasBootstrappedSessions = useRef(false)

  const { data: sessionData, isLoading: isSessionsLoading, refetch: refetchSessions } = api.useQuery(
    "get",
    "/v1/sessions"
  )
  const createSessionMutation = api.useMutation("post", "/v1/sessions")
  // Generated types currently drop the DELETE path param for this endpoint.
  const deleteSessionMutation = api.useMutation("delete", "/v1/sessions/{id}") as unknown as {
    isPending: boolean
    mutateAsync: (options: { params: { path: { id: string } } }) => Promise<unknown>
  }

  const sessionRecords = useMemo(() => toSessionRecords(sessionData), [sessionData])
  const conversationList = useMemo(
    () => sessionRecords.map((session) => toConversationItem(session, sessionLabels)),
    [sessionLabels, sessionRecords]
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
          toast.error("Failed to create chat session.", {
            description: getErrorDescription(error),
          })
        }

        return null
      }
    },
    [createSessionMutation, refetchSessions, setCurrentSessionId]
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
        toast.error("Failed to delete chat session.", {
          description: getErrorDescription(error),
        })
        return false
      }

      removeSessionLabel(sessionId)
      clearConversationCache(sessionId)

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
    ]
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
    deleteSession,
  }
}
