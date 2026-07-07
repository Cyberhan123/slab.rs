/**
 * OpenAI Responses protocol module for the slab-desktop assistant.
 *
 * Canonical home for everything that speaks the OpenAI Responses protocol:
 * types (sourced from the `openai` SDK), the streaming event parser/converter,
 * and the restore projector. The chat transport (`SlabChatTransport`) drives
 * `client.responses.create({stream:true})` directly and feeds events through
 * `convertEvent`.
 */

export { projectResponses } from "./project"
export { projectRestoreSession } from "./project"

export {
  convertEvent,
  createStreamState,
  parseStreamEvent,
} from "./stream"
export type { StreamState } from "./stream"

export type {
  Response,
  ResponseCustomToolCall,
  ResponseFunctionToolCall,
  ResponseOutputItem,
  ResponseOutputMessage,
  ResponseOutputMessageContent,
  ResponseOutputText,
  ResponseReasoningItem,
  ResponseReasoningSummaryPart,
  ResponseStreamEvent,
} from "./types"
