import { SSEFields, useXChat, XModelResponse } from '@ant-design/x-sdk'
import { useState } from 'react'
import locale from '../local'
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
  type ChatUiMessage,
} from '../chat-context'

export const useChat = (
  conversationKey: string,
  model: string,
  deepThink: boolean,
  beforeRequest?: () => Promise<void> | void
) => {
  const [activeConversation, setActiveConversation] = useState<string>()
  const resolvedConversationKey = conversationKey || DEFAULT_CONVERSATION_KEY

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
        content: getChatRequestErrorMessage(errorInfo) || error.message || locale.requestFailed,
        ...getChatRequestErrorMeta(errorInfo),
        role: 'assistant',
      }
    },
  })

  const withRequestDefaults = (
    requestParams?: Partial<ChatRequestParams>
  ): Partial<ChatRequestParams> => ({
    ...(requestParams ?? {}),
    model,
    thinking: {
      type: deepThink ? 'enabled' : 'disabled',
    },
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
        thinking: {
          type: deepThink ? 'enabled' : 'disabled',
        },
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
