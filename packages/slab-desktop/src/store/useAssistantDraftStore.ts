import { create } from "zustand"

type AssistantDraftSource = {
  label: string
  path?: string
}

type AssistantDraft = {
  autoSubmit: boolean
  prompt: string
  source?: AssistantDraftSource
}

type AssistantDraftState = {
  draft: AssistantDraft | null
  clearDraft: () => void
  setDraft: (draft: AssistantDraft) => void
}

export const useAssistantDraftStore = create<AssistantDraftState>()((set) => ({
  draft: null,
  clearDraft: () => set({ draft: null }),
  setDraft: (draft) => set({ draft }),
}))
