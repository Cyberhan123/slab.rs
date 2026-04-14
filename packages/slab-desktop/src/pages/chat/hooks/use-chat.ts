import { SSEFields, useXChat, XModelResponse } from '@ant-design/x-sdk'
import { useState } from 'react'
import { useChatLocale } from '@slab/i18n'
import {
  DEFAULT_CONVERSATION_KEY,
  getContinueGenerationPrefix,
  getChatMessageTextContent,
  getChatRequestErrorMessage,
  getChatRequestErrorMeta,
  historyMessageFactory,
  providerFactory,
  toChatRequestMessages,
  type ChatRequestParams,
  type ChatRuntimePresets,
  type ChatUiMessage,
} from '../chat-context'

export const useChat = (
  conversationKey: string,
  model: string,
  deepThink: boolean,
  supportsReasoningControls: boolean,
  runtimePresets?: ChatRuntimePresets | null,
  beforeRequest?: () => Promise<void> | void
) => {
  const [activeConversation, setActiveConversation] = useState<string>()
  const resolvedConversationKey = conversationKey || DEFAULT_CONVERSATION_KEY
  const locale = useChatLocale()

  const {
    onRequest,
    messages,
    isRequesting,
    abort,
    isDefaultMessagesRequesting,
    onReload: rawOnReload,
  } = useXChat<
    ChatUiMessage,
    ChatUiMessage,
    ChatRequestParams,
    Partial<Record<SSEFields, XModelResponse>>
  >({
    provider: providerFactory(resolvedConversationKey, model),
    conversationKey: resolvedConversationKey,
    defaultMessages: historyMessageFactory,
    requestPlaceholder: (requestParams) => {
      const continuationPrefix = requestParams?.continue_generation
        ? getContinueGenerationPrefix(requestParams.messages)
        : ''

      return {
        content: continuationPrefix || locale.noData,
        role: 'assistant',
      }
    },
    requestFallback: (_, { error, errorInfo, messageInfo }) => {
      if (error.name === 'AbortError') {
        const abortedContent = getChatMessageTextContent(messageInfo?.message)

        return {
          content:
            abortedContent && abortedContent !== locale.noData
              ? abortedContent
              : locale.requestAborted,
          role: 'assistant',
        }
      }

      return {
        content:
          getChatRequestErrorMessage(error) ||
          getChatRequestErrorMessage(errorInfo) ||
          error.message ||
          locale.requestFailed,
        ...getChatRequestErrorMeta(error),
        ...getChatRequestErrorMeta(errorInfo),
        role: 'assistant',
      }
    },
  })

  const buildThinkingParams = (): Partial<Pick<ChatRequestParams, 'thinking'>> =>
    supportsReasoningControls
      ? {
          thinking: {
            type: deepThink ? 'enabled' : 'disabled',
          },
        }
      : {}

  const buildRuntimePresetParams = (): Partial<
    Pick<ChatRequestParams, 'temperature' | 'top_p'>
  > => ({
    ...(typeof runtimePresets?.temperature === 'number'
      ? { temperature: runtimePresets.temperature }
      : {}),
    ...(typeof runtimePresets?.top_p === 'number'
      ? { top_p: runtimePresets.top_p }
      : {}),
  })

  const withRequestDefaults = (
    requestParams?: Partial<ChatRequestParams>
  ): Partial<ChatRequestParams> => ({
    ...(requestParams ?? {}),
    model,
    ...buildThinkingParams(),
    ...buildRuntimePresetParams(),
  })

  const runWithPreparedModel = (callback: () => void) => {
    Promise.resolve(beforeRequest?.())
      .then(() => {
        callback()
      })
      .catch(() => {
        // beforeRequest should handle its own user-facing errors.
      })
  }

  const handleSubmit = async (val: string) => {
    if (!val) return
    try {
      await beforeRequest?.()
      onRequest({
        model,
        messages: [{ role: 'user', content: val }],
        ...buildThinkingParams(),
        ...buildRuntimePresetParams(),
      })
      setActiveConversation(resolvedConversationKey)
    } catch (_e) {
      // beforeRequest should handle its own user-facing errors.
    }
  }

  const onReload = (id: string | number, requestParams?: Partial<ChatRequestParams>, opts?: any) => {
    runWithPreparedModel(() => {
      rawOnReload(id, withRequestDefaults(requestParams), opts)
    })
  }

  const onContinue = (
    id: string | number,
    requestParams?: Partial<ChatRequestParams>,
    opts?: any
  ) => {
    const targetIndex = messages.findIndex((item) => item.id === id)
    if (targetIndex < 0) {
      return
    }

    const snapshot = toChatRequestMessages(messages.slice(0, targetIndex + 1).map((item) => item.message))
    if (!getContinueGenerationPrefix(snapshot)) {
      onReload(id, requestParams, opts)
      return
    }

    runWithPreparedModel(() => {
      rawOnReload(
        id,
        withRequestDefaults({
          ...(requestParams ?? {}),
          continue_generation: true,
          messages: snapshot,
          userAction: requestParams?.userAction ?? 'continue',
        }),
        opts
      )
    })
  }

  return {
    messages,
    isRequesting,
    isHistoryLoading: isDefaultMessagesRequesting,
    abort,
    onReload,
    onContinue,
    activeConversation,
    setActiveConversation,
    handleSubmit,
  }
}
