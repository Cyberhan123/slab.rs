import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { h5FileDialog } from "./file-dialog-adapter"

/**
 * Minimal fake of the DOM surface the adapter touches: a hidden input whose
 * `click` the test drives by dispatching `change`/`cancel` with a files list.
 */
type FakeInput = {
  type: string
  hidden: boolean
  accept: string
  multiple: boolean
  files: File[] | null
  click: () => void
  addEventListener: (type: string, handler: () => void) => void
  removeEventListener: (type: string, handler: () => void) => void
  remove: () => void
}

function createFakeDocument() {
  const inputs: FakeInput[] = []
  const input = (): FakeInput => {
    const handlers = new Map<string, Set<() => void>>()
    return {
      type: "",
      hidden: false,
      accept: "",
      multiple: false,
      files: null,
      click: vi.fn(),
      addEventListener(type, handler) {
        if (!handlers.has(type)) handlers.set(type, new Set())
        handlers.get(type)!.add(handler)
      },
      removeEventListener(type, handler) {
        handlers.get(type)?.delete(handler)
      },
      remove: vi.fn(),
      fire(type: string) {
        for (const handler of handlers.get(type) ?? []) handler()
      },
    } as FakeInput & { fire: (type: string) => void }
  }
  return {
    inputs,
    createElement: () => {
      const node = input()
      inputs.push(node)
      return node
    },
    body: { append: (node: FakeInput) => void node },
  }
}

type FakeDocument = ReturnType<typeof createFakeDocument>

let fakeDocument: FakeDocument

function fireLast(type: "change" | "cancel") {
  const last = fakeDocument.inputs.at(-1) as (FakeInput & { fire: (t: string) => void }) | undefined
  if (!last) throw new Error("no input was created")
  last.fire(type)
}

describe("h5FileDialog", () => {
  beforeEach(() => {
    fakeDocument = createFakeDocument()
    vi.stubGlobal("document", fakeDocument)
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it("never offers a folder pick on mobile", async () => {
    await expect(h5FileDialog.pickFolder()).resolves.toBeNull()
  })

  it("pickFile resolves with the first picked file and cleans up the input", async () => {
    const file = new File(["x"], "photo.png", { type: "image/png" })
    const promise = h5FileDialog.pickFile()
    const input = fakeDocument.inputs[0]
    expect(input.type).toBe("file")
    expect(input.hidden).toBe(true)
    expect(input.accept).toBe("")

    input.files = [file]
    fireLast("change")
    await expect(promise).resolves.toEqual({ file, name: "photo.png" })
    expect(input.remove).toHaveBeenCalled()
  })

  it("pickFile maps filters to an accept list", async () => {
    const promise = h5FileDialog.pickFile({
      filters: [{ name: "Images", extensions: ["png", "jpg"] }],
    })
    expect(fakeDocument.inputs[0].accept).toBe(".png,.jpg")
    fireLast("cancel")
    await expect(promise).resolves.toBeNull()
  })

  it("pickFiles resolves with every picked file (multiple)", async () => {
    const a = new File(["1"], "a.png")
    const b = new File(["2"], "b.png")
    const promise = h5FileDialog.pickFiles({ multiple: true })
    const input = fakeDocument.inputs[0]
    expect(input.multiple).toBe(true)
    input.files = [a, b]
    fireLast("change")
    await expect(promise).resolves.toEqual([
      { file: a, name: "a.png" },
      { file: b, name: "b.png" },
    ])
  })

  it("resolves empty when the user cancels without the cancel event", async () => {
    const promise = h5FileDialog.pickFile()
    fireLast("change") // change with no files = dismissal on some WebViews
    await expect(promise).resolves.toBeNull()
  })
})
