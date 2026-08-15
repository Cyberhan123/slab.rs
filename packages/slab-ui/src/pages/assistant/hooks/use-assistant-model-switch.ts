import { useCallback, useMemo, useState } from "react"
import { toast } from "sonner"

import { useTranslation } from "@slab/i18n"

import type { ModelOption } from "../lib/assistant-page-state"

type UseAssistantModelSwitchOptions = {
  modelOptions: ModelOption[]
  selectedModelId: string
  setSelectedModelId: (value: string) => void
  isSessionBusy: boolean
  isSessionBootstrapping: boolean
  curConversation: string | undefined
  messageCount: number
  createSession: (options?: { select?: boolean; quiet?: boolean }) => Promise<{ id: string } | null>
  isCreatingSession: boolean
}

export function useAssistantModelSwitch({
  modelOptions,
  selectedModelId,
  setSelectedModelId,
  isSessionBusy,
  isSessionBootstrapping,
  curConversation,
  messageCount,
  createSession,
  isCreatingSession,
}: UseAssistantModelSwitchOptions) {
  const { t } = useTranslation()
  const [pendingModelSwitchId, setPendingModelSwitchId] = useState<string | null>(null)

  const pendingModelSwitch = useMemo(
    () => modelOptions.find((item) => item.id === pendingModelSwitchId) ?? null,
    [modelOptions, pendingModelSwitchId]
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
    const session = await createSession({ select: true })

    if (!session) {
      return
    }

    setSelectedModelId(nextModelId)
    setPendingModelSwitchId(null)
  }, [createSession, pendingModelSwitchId, setSelectedModelId])

  return {
    pendingModelSwitchId,
    pendingModelSwitch,
    handleModelPickerChange,
    closePendingModelSwitch,
    handleKeepSessionOnModelSwitch,
    handleCreateSessionOnModelSwitch,
  }
}
