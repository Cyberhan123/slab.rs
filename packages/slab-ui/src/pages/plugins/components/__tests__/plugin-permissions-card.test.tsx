import { userEvent } from "vitest/browser"
import { render } from "vitest-browser-react"
import type { ReactNode } from "react"
import { beforeEach, describe, expect, it, vi } from "vitest"


import { usePluginAuthorizationStore } from "@slab/ui/store/usePluginAuthorizationStore"

import { PluginPermissionsCard } from "../plugin-permissions-card"

vi.mock("@slab/i18n", async () => {
  const { setupSlabI18nMock } = await import("@slab/test-utils/mocks")
  return setupSlabI18nMock()
})

vi.mock("@slab/plugin-sdk", () => ({
  describeSlabApiPermission: (permission: string) => ({
    title: `T:${permission}`,
    description: `D:${permission}`,
    severity: "medium",
  }),
}))

vi.mock("@slab/components/button", () => ({
  Button: ({
    children,
    onClick,
    disabled,
    title,
    ...rest
  }: {
    children: ReactNode
    onClick?: () => void
    disabled?: boolean
    title?: string
  } & Record<string, unknown>) => (
    <button type="button" onClick={onClick} disabled={disabled} title={title} {...rest}>
      {children}
    </button>
  ),
}))

const revoke = vi.hoisted(() => ({
  fn: vi.fn<(pluginId: string, permission?: string) => void>(),
}))

beforeEach(() => {
  revoke.fn = vi.fn<(pluginId: string, permission?: string) => void>()
  usePluginAuthorizationStore.setState({ grants: {}, revoke: revoke.fn })
})

describe("PluginPermissionsCard", () => {
  it("shows the empty state when there are no grants", async () => {
    const screen = await render(<PluginPermissionsCard />)

    await expect.element(
      screen.getByText("pages.plugins.permissions.management.empty"),
    ).toBeInTheDocument()
    await expect.element(
      screen.getByTestId("plugin-permissions-revoke-all"),
    ).not.toBeInTheDocument()
  })

  it("revokes every granted plugin via the revoke-all button", async () => {
    usePluginAuthorizationStore.setState({
      grants: { "plugin-a": ["perm1", "perm2"], "plugin-b": [] },
    })

    const screen = await render(<PluginPermissionsCard />)

    await userEvent.click(screen.getByTestId("plugin-permissions-revoke-all"))

    expect(revoke.fn).toHaveBeenCalledExactlyOnceWith("plugin-a")
  })

  it("revokes a single permission", async () => {
    usePluginAuthorizationStore.setState({
      grants: { "plugin-a": ["perm1"] },
    })

    const screen = await render(<PluginPermissionsCard />)

    await userEvent.click(screen.getByTestId("plugin-permissions-revoke-plugin-a-perm1"))

    expect(revoke.fn).toHaveBeenCalledExactlyOnceWith("plugin-a", "perm1")
  })
})
