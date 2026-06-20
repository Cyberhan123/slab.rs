import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it, vi } from "vitest"

import { useWorkspaceConfirmDialog } from "../use-workspace-confirm"

vi.mock("@slab/i18n", () => ({
  useTranslation: () => ({
    // Passthrough so the rendered dialog text is the i18n key, which lets the test
    // assert which message/confirm key was passed without loading locale data.
    t: (key: string, params?: Record<string, unknown>) => {
      if (!params) return key
      return `${key}:${JSON.stringify(params)}`
    },
    i18n: { resolvedLanguage: "en", language: "en" },
  }),
}))

type ConfirmOptions = Parameters<ReturnType<typeof useWorkspaceConfirmDialog>["confirm"]>[0]

function setupConfirm() {
  let confirmRef: ((options: ConfirmOptions) => Promise<boolean>) | null = null
  function Harness() {
    const { confirm, dialog } = useWorkspaceConfirmDialog()
    confirmRef = confirm
    return <>{dialog}</>
  }

  render(<Harness />)
  return {
    confirm: (options: ConfirmOptions) => {
      if (!confirmRef) throw new Error("harness not mounted")
      return confirmRef(options)
    },
  }
}

describe("useWorkspaceConfirmDialog", () => {
  it("opens a themed modal with the requested message and resolves true on accept", async () => {
    const { confirm } = setupConfirm()
    const pending = confirm({
      messageKey: "pages.workspace.confirm.discardUnsaved",
      confirmKey: "pages.workspace.confirm.discard",
      tone: "danger",
    })

    expect(await screen.findByTestId("workspace-confirm-dialog")).toBeInTheDocument()
    expect(screen.getByText("pages.workspace.confirm.discardUnsaved")).toBeInTheDocument()

    await userEvent.click(screen.getByTestId("workspace-confirm-accept"))

    expect(await pending).toBe(true)
    expect(screen.queryByTestId("workspace-confirm-dialog")).not.toBeInTheDocument()
  })

  it("resolves false on cancel so callers keep the unsaved content", async () => {
    const { confirm } = setupConfirm()
    const pending = confirm({
      messageKey: "pages.workspace.confirm.closeUnsaved",
      confirmKey: "pages.workspace.confirm.closeAnyway",
      tone: "danger",
    })

    await screen.findByTestId("workspace-confirm-dialog")
    await userEvent.click(screen.getByTestId("workspace-confirm-cancel"))

    expect(await pending).toBe(false)
    expect(screen.queryByTestId("workspace-confirm-dialog")).not.toBeInTheDocument()
  })

  it("resolves false when the dialog is dismissed without accepting", async () => {
    const { confirm } = setupConfirm()
    const pending = confirm({
      messageKey: "pages.workspace.confirm.discardGitChange",
      messageParams: { path: "src/app.ts" },
      confirmKey: "pages.workspace.confirm.discard",
      tone: "danger",
    })

    await screen.findByTestId("workspace-confirm-dialog")
    // Interpolated message params are forwarded to the translator.
    expect(screen.getByText(/pages.workspace.confirm.discardGitChange:.*src\/app.ts/)).toBeInTheDocument()

    // Simulate the user closing the dialog via the root onOpenChange (overlay/escape).
    await userEvent.keyboard("{Escape}")

    expect(await pending).toBe(false)
  })
})
