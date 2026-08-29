import { renderHook } from "vitest-browser-react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const { useHeaderMock } = vi.hoisted(() => ({
  useHeaderMock: vi.fn<(registration: unknown) => void>(),
}))

vi.mock("@slab/ui/hooks/use-header", () => ({ useHeader: useHeaderMock }))
vi.mock("@slab/i18n", () => ({
  default: { t: (key: string) => key },
  useTranslation: () => ({ t: (key: string) => key }),
}))

import type { ModelOption } from "../../lib/assistant-page-state"
import { useAssistantHeader } from "../use-assistant-header"

function modelOption(overrides: Partial<ModelOption> = {}): ModelOption {
  return {
    capabilities: { raw_gbnf: true, reasoning_controls: false, structured_output: true },
    contextWindow: null,
    downloaded: true,
    id: "model-1",
    label: "Model",
    pending: false,
    source: "local",
    ...overrides,
  }
}

async function renderAssistantHeader(modelOptions: ModelOption[]) {
  return renderHook(() =>
    useAssistantHeader({
      isSessionBootstrapping: false,
      isSessionBusy: false,
      modelLoading: false,
      modelOptions,
      onModelPickerChange: vi.fn(),
      onNewSession: vi.fn(),
      onOpenSessionSheet: vi.fn(),
      pendingModelSwitchId: null,
      selectedModelId: "model-1",
    }),
  )
}

function lastSelectOptions() {
  const registration = useHeaderMock.mock.calls.at(-1)?.[0] as
    | { select?: { options?: unknown[] } }
    | undefined
  return registration?.select?.options ?? []
}

describe("useAssistantHeader model picker groups", () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it("groups models local-first regardless of catalog order", async () => {
    await renderAssistantHeader([
      modelOption({ id: "glm-4.5", label: "GLM-4.5", source: "cloud" }),
      modelOption({ id: "qwen", label: "Qwen3.5", source: "local" }),
      modelOption({ id: "glm-4.5-flash", label: "GLM-4.5 flash", source: "cloud" }),
    ])

    const options = lastSelectOptions() as Array<{
      id: string
      children: { groupLabel: string; options: Array<{ id: string; label: string }> }
    }>
    expect(options).toHaveLength(2)
    const [localGroup, cloudGroup] = options
    expect(localGroup.id).toBe("local")
    expect(localGroup.children.groupLabel).toBe("pages.assistant.modelPicker.localGroupLabel")
    expect(localGroup.children.options.map((option) => option.id)).toEqual(["qwen"])
    expect(cloudGroup.id).toBe("cloud")
    expect(cloudGroup.children.groupLabel).toBe("pages.assistant.modelPicker.cloudGroupLabel")
    expect(cloudGroup.children.options.map((option) => option.id)).toEqual([
      "glm-4.5",
      "glm-4.5-flash",
    ])
  })

  it("emits only the cloud group when no local models exist", async () => {
    await renderAssistantHeader([
      modelOption({ id: "glm-4.5", label: "GLM-4.5", source: "cloud" }),
    ])

    const options = lastSelectOptions() as Array<{ id: string }>
    expect(options).toHaveLength(1)
    expect(options[0]?.id).toBe("cloud")
  })

  it("emits no options for an empty catalog", async () => {
    await renderAssistantHeader([])

    expect(lastSelectOptions()).toHaveLength(0)
  })
})
