/**
 * Interaction contexts for the message tree. TWO contexts, split by change
 * frequency: the base context carries slowly-changing data (approval badge
 * status, per-user-message turn index, rollback action) consumed by every
 * row, while the live context carries the per-delta streaming accumulations
 * (command output / apply_patch progress) consumed ONLY by the streaming
 * tool cards. Keeping them apart means an output delta re-renders the active
 * tool card instead of every visible row.
 *
 * The approve/reject *actions* live in the separate approval banner
 * (`components/approval-banner.tsx`).
 */

import { createContext, useContext } from "react"

export type ApprovalStatus = "pending" | "approved" | "denied"

export interface MessageInteractionValue {
  /** itemId → approval status, sourced from the harness approval notifications. */
  approvalStatusByItemId: ReadonlyMap<string, ApprovalStatus>
  /** userMessage itemId → its numeric turn index (drives the rollback affordance). */
  userMessageTurnIndex: ReadonlyMap<string, number>
  /** Retract the user message with this id and everything after it. */
  rollbackToMessage: ((messageId: string) => void) | undefined
}

export interface LiveToolOutputValue {
  /** itemId → accumulated live command output (stdout/stderr deltas so far). */
  liveOutputByItemId: ReadonlyMap<string, string>
  /** itemId → live `apply_patch` progress lines (one JSON object per applied file). */
  livePatchByItemId: ReadonlyMap<string, string[]>
}

export const MessageInteractionContext = createContext<MessageInteractionValue>({
  approvalStatusByItemId: new Map<string, ApprovalStatus>(),
  userMessageTurnIndex: new Map<string, number>(),
  rollbackToMessage: undefined,
})

export const LiveToolOutputContext = createContext<LiveToolOutputValue>({
  liveOutputByItemId: new Map<string, string>(),
  livePatchByItemId: new Map<string, string[]>(),
})

export const useMessageInteraction = (): MessageInteractionValue =>
  useContext(MessageInteractionContext)

export const useLiveToolOutput = (): LiveToolOutputValue =>
  useContext(LiveToolOutputContext)
