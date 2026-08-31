import { create } from "zustand"
import type { FileUIPart } from "ai"

import type { PermissionMode, ReasoningEffort } from "@slab/api/harness"

type AssistantDraftSource = {
  label: string
  path?: string
}

type AssistantDraft = {
  autoSubmit: boolean
  prompt: string
  source?: AssistantDraftSource
  /**
   * When set, the draft only auto-sends once the assistant is showing this
   * session (the new-chat dialog creates the session before stashing the
   * draft). Absent = send into whatever session is current.
   */
  sessionId?: string
  /** Attachment parts for the first message (data URLs — memory-only store). */
  files?: FileUIPart[]
  effort?: ReasoningEffort
  permissionMode?: PermissionMode
  agentType?: "plan"
}

type WorkspaceRevealPayload = {
  revealPath?: string
}

export type WorkspaceRevealInput = {
  payload?: WorkspaceRevealPayload
  type: "workspace"
}

export type WorkspaceRevealRequest = WorkspaceRevealInput & {
  createdAt: number
  id: string
}

type WorkspaceHandoffState = {
  clearDraft: () => void
  clearPendingWorkspaceReveal: (id?: string) => void
  consumeDraft: () => AssistantDraft | null
  consumePendingWorkspaceReveal: (id?: string) => WorkspaceRevealRequest | null
  draft: AssistantDraft | null
  pendingWorkspaceReveal: WorkspaceRevealRequest | null
  setDraft: (draft: AssistantDraft) => void
  setPendingWorkspaceReveal: (reveal: WorkspaceRevealInput) => void
}

let revealRequestSequence = 0

function createRevealRequest(reveal: WorkspaceRevealInput): WorkspaceRevealRequest {
  revealRequestSequence += 1
  const createdAt = Date.now()

  return {
    ...reveal,
    createdAt,
    id: `${reveal.type}:${createdAt}:${revealRequestSequence}`,
  }
}

export const useWorkspaceHandoffStore = create<WorkspaceHandoffState>()((set, get) => ({
  draft: null,
  pendingWorkspaceReveal: null,
  clearDraft: () => set({ draft: null }),
  clearPendingWorkspaceReveal: (id) =>
    set((state) => {
      if (id && state.pendingWorkspaceReveal?.id !== id) {
        return state
      }

      return { pendingWorkspaceReveal: null }
    }),
  consumeDraft: () => {
    const { draft } = get()
    set({ draft: null })
    return draft
  },
  consumePendingWorkspaceReveal: (id) => {
    const { pendingWorkspaceReveal } = get()
    if (!pendingWorkspaceReveal || (id && pendingWorkspaceReveal.id !== id)) {
      return null
    }

    set({ pendingWorkspaceReveal: null })
    return pendingWorkspaceReveal
  },
  setDraft: (draft) => set({ draft }),
  setPendingWorkspaceReveal: (reveal) =>
    set({ pendingWorkspaceReveal: createRevealRequest(reveal) }),
}))
