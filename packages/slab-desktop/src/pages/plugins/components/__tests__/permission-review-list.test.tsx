import { render } from "vitest-browser-react"
import { describe, expect, it, vi } from "vitest"

import { setupSlabI18nMock } from "@slab/test-utils/mocks"

import { PermissionReviewList } from "../permission-review-list"

vi.mock("@slab/i18n", () => setupSlabI18nMock())

vi.mock("@slab/plugin-sdk", () => ({
  describeSlabApiPermission: (permission: string) => ({
    title: `T:${permission}`,
    description: `D:${permission}`,
    severity: "medium",
  }),
  isKnownSlabApiPermission: (permission: string) => permission !== "unknown.perm",
}))

const preview = (overrides: Record<string, unknown> = {}) =>
  ({
    id: "p1",
    name: "Plugin",
    version: "1.0.0",
    parseError: null,
    permissions: {
      slabApi: ["known.perm", "unknown.perm"],
      filesRead: ["/read"],
      filesWrite: ["/write"],
      networkMode: "restricted",
      networkHosts: ["example.com"],
      agent: ["agent.tool"],
      lsp: ["rust-analyzer"],
    },
    ...overrides,
  }) as never

describe("PermissionReviewList", () => {
  it("shows the none message when the manifest requests no permissions", async () => {
    const screen = await render(
      <PermissionReviewList
        preview={
          {
            id: "p1",
            name: "Plugin",
            version: "1.0.0",
            parseError: null,
            permissions: {
              slabApi: [],
              filesRead: [],
              filesWrite: [],
              networkMode: null,
              networkHosts: [],
              agent: [],
              lsp: [],
            },
          } as never
        }
      />,
    )

    await expect.element(screen.getByText("pages.plugins.permissions.none")).toBeInTheDocument()
  })

  it("lists slab API permissions and warns about unknown ones", async () => {
    const screen = await render(<PermissionReviewList preview={preview()} />)

    await expect.element(screen.getByTestId("plugin-permission-known.perm")).toBeInTheDocument()
    await expect.element(screen.getByTestId("plugin-permission-unknown.perm")).toBeInTheDocument()
    await expect.element(
      screen.getByText("pages.plugins.permissions.unknownWarning"),
    ).toBeInTheDocument()
  })

  it("renders file and network permission chips", async () => {
    const screen = await render(<PermissionReviewList preview={preview()} />)

    await expect.element(screen.getByText("read: /read")).toBeInTheDocument()
    await expect.element(screen.getByText("write: /write")).toBeInTheDocument()
    await expect.element(screen.getByText("host: example.com")).toBeInTheDocument()
  })
})
