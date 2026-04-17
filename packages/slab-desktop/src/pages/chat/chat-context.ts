// Keep a single chat entry point for the page-level hooks and components.
// The heavier transport/history/provider logic now lives under `./lib/*`.

export {
  DEFAULT_CONVERSATION_KEY,
  type ChatMessageRecord,
  type ChatRequestErrorInfo,
  type ChatRequestErrorType,
  type ChatRequestParams,
  type ChatRuntimePresets,
  type ChatUiMessage,
} from './lib/chat-types'

export {
  getChatMessageTextContent,
  getContinueGenerationPrefix,
  stripTrailingAssistantTurnArtifacts,
  toChatRequestMessages,
} from './lib/chat-message-utils'

export {
  ChatTransportError,
  getChatRequestErrorMessage,
  getChatRequestErrorMeta,
  isChatRequestErrorInfo,
  isChatTransportError,
} from './lib/chat-request-errors'

export { historyMessageFactory } from './lib/chat-history'

export { clearConversationCache, providerCaches, providerFactory } from './lib/chat-provider'
