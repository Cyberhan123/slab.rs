// Keep a single assistant entry point for the page-level hooks and components.
// The heavier transport/history/runtime logic lives under `./lib/*`.

export {
  DEFAULT_CONVERSATION_KEY,
  isEphemeralConversationKey,
  type AgentResponsesClientMessage,
  type AgentResponsesServerMessage,
  type AgentStatus,
  type AgentThreadMessageResponse,
  type AgentThreadResponse,
  type AssistantMessageRecord,
  type AssistantRequestErrorInfo,
  type AssistantRequestErrorType,
  type AssistantRequestParams,
  type AssistantRuntimePresets,
  type AssistantThought,
  type AssistantUiMessage,
} from './lib/assistant-types'

export {
  getAssistantMessageTextContent,
  getContinueGenerationPrefix,
  stripTrailingAssistantTurnArtifacts,
  toAssistantRequestMessages,
} from './lib/assistant-message-utils'

export {
  AssistantTransportError,
  getAssistantErrorDescription,
  getAssistantRequestErrorMessage,
  getAssistantRequestErrorMeta,
  isAssistantRequestErrorInfo,
  isAssistantTransportError,
} from './lib/assistant-request-errors'

export { toStoredSessionAssistantMessage } from './lib/assistant-history'
