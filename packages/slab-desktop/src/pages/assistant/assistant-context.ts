// Keep a single assistant entry point for the page-level hooks and components.
// The heavier transport/history/runtime logic lives under `./lib/*`.

export {
  DEFAULT_CONVERSATION_KEY,
  isEphemeralConversationKey,
  type AgentApprovalResolveRequest,
  type AgentControlResponse,
  type AgentHistoryResponse,
  type AgentStatus,
  type AgentThreadControlRequest,
  type AssistantArtifactRef,
  type AgentThreadMessageResponse,
  type AgentThreadResponse,
  type AssistantMessageRecord,
  type AssistantMessageStatus,
  type AssistantRequestErrorInfo,
  type AssistantRequestErrorType,
  type AssistantRequestParams,
  type AssistantRuntimePresets,
  type AssistantThought,
  type AssistantThoughtStatus,
  type AssistantUiMessage,
  type OpenAICreateRequest,
} from './lib/assistant-types'

export {
  getAssistantMessageTextContent,
  getContinueGenerationPrefix,
  stripThinkTags,
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
