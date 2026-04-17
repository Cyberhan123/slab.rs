import {
  DeepSeekChatProvider,
  SSEFields,
  XModelResponse,
  XRequest,
} from '@ant-design/x-sdk'
import { chatMessagesStoreHelper } from '@ant-design/x-sdk/es/x-chat/store'

import { SERVER_BASE_URL } from '@/lib/config'

import {
  extractSseDeltaTextField,
  getChatMessageTextContent,
  getContinueGenerationPrefix,
  mergeContinuationContent,
  stripTrailingAssistantTurnArtifacts,
  toChatRequestMessages,
  withChatMessageReasoningContent,
  withChatMessageTextContent,
} from './chat-message-utils'
import {
  ChatTransportError,
  adaptChatTransportResponse,
  extractStreamChunkError,
  getResponseRequestId,
} from './chat-request-errors'
import { isEphemeralConversationKey, type ChatRequestParams, type ChatUiMessage } from './chat-types'

type ProviderTransformParamsOptions = Parameters<
  DeepSeekChatProvider<
    ChatUiMessage,
    ChatRequestParams,
    Partial<Record<SSEFields, XModelResponse>>
  >['transformParams']
>[1]

type ProviderTransformMessageInfo = Parameters<
  DeepSeekChatProvider<
    ChatUiMessage,
    ChatRequestParams,
    Partial<Record<SSEFields, XModelResponse>>
  >['transformMessage']
>[0]

const CHAT_COMPLETIONS_URL = `${SERVER_BASE_URL}/v1/chat/completions`

// Our backend speaks OpenAI-compatible chat completions, but streamed
// reasoning arrives through `delta.reasoning_content`, so the DeepSeek
// provider remains the closest built-in base. The customization below keeps
// the slab-specific continuation and history semantics on top of that base.
class SlabChatProvider extends DeepSeekChatProvider<
  ChatUiMessage,
  ChatRequestParams,
  Partial<Record<SSEFields, XModelResponse>>
> {
  private pendingContinuationPrefix = ''
  private pendingReasoningContent = ''

  override transformParams(
    requestParams: Partial<ChatRequestParams>,
    options: ProviderTransformParamsOptions
  ): ChatRequestParams {
    const requestMessages =
      requestParams.continue_generation && requestParams.messages?.length
        ? requestParams.messages
        : this.getMessages()

    this.pendingContinuationPrefix = requestParams.continue_generation
      ? getContinueGenerationPrefix(requestParams.messages)
      : ''
    this.pendingReasoningContent = ''

    return {
      ...(options?.params || {}),
      ...requestParams,
      messages: toChatRequestMessages(requestMessages),
    }
  }

  override transformLocalMessage(requestParams: Partial<ChatRequestParams>): ChatUiMessage[] {
    if (requestParams?.continue_generation) {
      return []
    }

    return super.transformLocalMessage(requestParams)
  }

  captureStreamChunk(chunk: Partial<Record<SSEFields, XModelResponse>> | undefined) {
    const reasoningChunk = extractSseDeltaTextField(chunk, 'reasoning_content')
    if (!reasoningChunk) {
      return
    }

    this.pendingReasoningContent = mergeContinuationContent(
      this.pendingReasoningContent,
      reasoningChunk
    )
  }

  override transformMessage(info: ProviderTransformMessageInfo): ChatUiMessage {
    let message = super.transformMessage(info)
    if (message.role === 'assistant') {
      const content = stripTrailingAssistantTurnArtifacts(getChatMessageTextContent(message))
      if (content !== getChatMessageTextContent(message)) {
        message = withChatMessageTextContent(message, content)
      }
      message = withChatMessageReasoningContent(message, this.pendingReasoningContent)
    }

    if (!this.pendingContinuationPrefix) {
      return message
    }

    const prefix = this.pendingContinuationPrefix
    this.pendingContinuationPrefix = ''

    return withChatMessageTextContent(
      message,
      mergeContinuationContent(prefix, getChatMessageTextContent(message))
    )
  }
}

export const providerCaches = new Map<string, SlabChatProvider>()

export const clearConversationCache = (conversationKey: string) => {
  if (isEphemeralConversationKey(conversationKey)) {
    return
  }

  const providerCachePrefix = `${conversationKey}::`
  Array.from(providerCaches.keys()).forEach((cacheKey) => {
    if (cacheKey.startsWith(providerCachePrefix)) {
      providerCaches.delete(cacheKey)
    }
  })

  chatMessagesStoreHelper.delete(conversationKey)
}

export const providerFactory = (conversationKey: string, model: string) => {
  const cacheKey = `${conversationKey}::${model}`
  const cachedProvider = providerCaches.get(cacheKey)
  if (cachedProvider) {
    return cachedProvider
  }

  let provider: SlabChatProvider | undefined
  const request = XRequest<ChatRequestParams, Partial<Record<SSEFields, XModelResponse>>, ChatUiMessage>(
    CHAT_COMPLETIONS_URL,
    {
      manual: true,
      callbacks: {
        onUpdate: (chunk, responseHeaders) => {
          provider?.captureStreamChunk(chunk)

          // Local runtime errors can arrive as in-band SSE chunks with no `choices`.
          const error = extractStreamChunkError(chunk)
          if (!error) {
            return
          }

          const request_id = getResponseRequestId(responseHeaders)
          throw new ChatTransportError({
            message: error.message,
            transport_status: 200,
            code: error.code ?? undefined,
            param: error.param ?? undefined,
            request_id,
            error_type: error.type,
          })
        },
        onSuccess: () => {},
        onError: () => {},
      },
      middlewares: {
        onResponse: adaptChatTransportResponse,
      },
      params: {
        stream: true,
        model,
        ...(isEphemeralConversationKey(conversationKey) ? {} : { id: conversationKey }),
      },
    }
  )

  provider = new SlabChatProvider({ request })
  providerCaches.set(cacheKey, provider)
  return provider
}
