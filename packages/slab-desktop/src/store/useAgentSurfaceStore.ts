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

type WorkspaceSurfacePayload = {
  revealPath?: string
}

type ImageSurfacePayload = {
  prompt?: string
}

type ReviewSurfacePayload = {
  diff?: string
  path?: string
}

type PluginSurfacePayload = {
  payload?: unknown
  pluginId?: string
  surface?: string
}

export type AgentSurfaceInput =
  | { payload?: WorkspaceSurfacePayload; type: "workspace" }
  | { payload?: ImageSurfacePayload; type: "image" }
  | { payload?: ReviewSurfacePayload; type: "review" }
  | { payload?: PluginSurfacePayload; type: "plugin" }
  | { payload?: Record<string, never>; type: "audio" | "hub" | "video" }

export type AgentSurfaceRequest = AgentSurfaceInput & {
  createdAt: number
  id: string
  target?: "inline" | "window"
  targetRoute?: "assistant" | "workspace"
}

type PendingSurfaceOptions = {
  target?: AgentSurfaceRequest["target"]
  targetRoute?: AgentSurfaceRequest["targetRoute"]
}

type AgentSurfaceState = {
  clearDraft: () => void
  clearPendingSurface: (surfaceId?: string) => void
  consumeDraft: () => AssistantDraft | null
  consumePendingSurface: (surfaceId?: string) => AgentSurfaceRequest | null
  draft: AssistantDraft | null
  focusComposerSignal: number
  requestComposerFocus: () => void
  pendingSurface: AgentSurfaceRequest | null
  setDraft: (draft: AssistantDraft) => void
  setPendingSurface: (surface: AgentSurfaceInput, options?: PendingSurfaceOptions) => void
}

let surfaceRequestSequence = 0

function createSurfaceRequest(
  surface: AgentSurfaceInput,
  options: PendingSurfaceOptions = {}
): AgentSurfaceRequest {
  surfaceRequestSequence += 1
  const createdAt = Date.now()

  return {
    ...surface,
    createdAt,
    id: `${surface.type}:${createdAt}:${surfaceRequestSequence}`,
    target: options.target,
    targetRoute: options.targetRoute,
  }
}

export const useAgentSurfaceStore = create<AgentSurfaceState>()((set, get) => ({
  draft: null,
  focusComposerSignal: 0,
  pendingSurface: null,
  clearDraft: () => set({ draft: null }),
  clearPendingSurface: (surfaceId) =>
    set((state) => {
      if (surfaceId && state.pendingSurface?.id !== surfaceId) {
        return state
      }

      return { pendingSurface: null }
    }),
  consumeDraft: () => {
    const { draft } = get()
    set({ draft: null })
    return draft
  },
  consumePendingSurface: (surfaceId) => {
    const { pendingSurface } = get()
    if (!pendingSurface || (surfaceId && pendingSurface.id !== surfaceId)) {
      return null
    }

    set({ pendingSurface: null })
    return pendingSurface
  },
  requestComposerFocus: () =>
    set((state) => ({ focusComposerSignal: state.focusComposerSignal + 1 })),
  setDraft: (draft) => set({ draft }),
  setPendingSurface: (surface, options) => set({ pendingSurface: createSurfaceRequest(surface, options) }),
}))
