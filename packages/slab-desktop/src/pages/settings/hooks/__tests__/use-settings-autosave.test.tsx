import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { renderHook } from "vitest-browser-react"
import type { ReactNode } from "react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { setupSlabI18nMock } from "@slab/test-utils/mocks"

import type { SettingResponse } from "../../types"

// Controllable mutation so tests can resolve/reject the PUT on demand.
const mutation = {
  mutateAsync: vi.fn<(arg: unknown) => Promise<unknown>>(),
}

vi.mock("@slab/i18n", () => ({
  ...setupSlabI18nMock(),
  isAppLanguagePreference: () => false,
  applyAppLanguagePreference: vi.fn(),
}))

vi.mock("@slab/api", () => ({
  default: { useMutation: () => mutation },
  getErrorMessage: (e: unknown) => (e instanceof Error ? e.message : String(e)),
  isApiError: () => false,
}))

import { useSettingsAutosave } from "../use-settings-autosave"

// Array-typed property → autoSaveDelay is 900ms and buildRequestBody returns
// { op: "set", value: draftValue } for an array draft.
const property = {
  pmid: "providers.registry",
  schema: { type: "array", default_value: [] },
  effective_value: [],
} as unknown as SettingResponse

const makePropertyMap = () => new Map([[property.pmid, property]])

function wrapper({ children }: { children: ReactNode }) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
}

describe("useSettingsAutosave", () => {
  beforeEach(() => {
    vi.useFakeTimers()
    mutation.mutateAsync.mockReset()
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  it("marks the field saved and clears the draft after a successful PUT", async () => {
    mutation.mutateAsync.mockResolvedValue(undefined)
    const refetch = vi.fn().mockResolvedValue(undefined)
    const { result, act } = await renderHook(
      () => useSettingsAutosave({ propertyMap: makePropertyMap(), refetch }),
      { wrapper },
    )

    await act(async () => {
      result.current.setDraftValue(property, [{ foo: "bar" }])
    })
    expect(result.current.drafts[property.pmid]).toEqual([{ foo: "bar" }])
    expect(result.current.fieldStatuses[property.pmid]?.tone).toBe("dirty")

    await act(async () => {
      await vi.advanceTimersByTimeAsync(900)
    })

    expect(result.current.fieldStatuses[property.pmid]?.tone).toBe("saved")
    expect(result.current.drafts[property.pmid]).toBeUndefined()
    const { dirty, saving, error } = result.current.statusSummary
    expect(dirty + saving + error).toBe(0)
    expect(mutation.mutateAsync).toHaveBeenCalledOnce()
  })

  it("keeps the newer draft and stays dirty when a second edit lands mid-flight", async () => {
    // First PUT stays pending until we resolve it; the second save resolves fast.
    let resolveFirst!: (value: unknown) => void
    mutation.mutateAsync.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveFirst = resolve
        }),
    )
    mutation.mutateAsync.mockResolvedValue(undefined)
    const refetch = vi.fn().mockResolvedValue(undefined)
    const { result, act } = await renderHook(
      () => useSettingsAutosave({ propertyMap: makePropertyMap(), refetch }),
      { wrapper },
    )

    await act(async () => {
      result.current.setDraftValue(property, [{ foo: "a" }])
    })
    // Fire the 900ms timer → saveDraft runs, the first PUT is in flight.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(900)
    })
    expect(result.current.fieldStatuses[property.pmid]?.tone).toBe("saving")

    // A newer edit lands WHILE the first PUT is still pending.
    await act(async () => {
      result.current.setDraftValue(property, [{ foo: "b" }])
    })
    expect(result.current.drafts[property.pmid]).toEqual([{ foo: "b" }])

    // First PUT resolves: the newer draft must survive and the field stay dirty.
    await act(async () => {
      resolveFirst(undefined)
      await vi.advanceTimersByTimeAsync(0)
    })

    expect(result.current.fieldStatuses[property.pmid]?.tone).toBe("dirty")
    expect(result.current.drafts[property.pmid]).toEqual([{ foo: "b" }])
  })

  it("marks the field error when the PUT rejects", async () => {
    mutation.mutateAsync.mockRejectedValue(new Error("boom"))
    const refetch = vi.fn().mockResolvedValue(undefined)
    const { result, act } = await renderHook(
      () => useSettingsAutosave({ propertyMap: makePropertyMap(), refetch }),
      { wrapper },
    )

    await act(async () => {
      result.current.setDraftValue(property, [{ foo: "a" }])
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(900)
    })

    expect(result.current.fieldStatuses[property.pmid]?.tone).toBe("error")
    expect(result.current.fieldStatuses[property.pmid]?.message).toBe("boom")
  })
})
