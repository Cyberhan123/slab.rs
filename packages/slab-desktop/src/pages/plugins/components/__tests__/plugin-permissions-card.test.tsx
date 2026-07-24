import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import type { ReactNode } from "react"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { setupSlabI18nMock } from "@slab/test-utils/mocks"

import { usePluginAuthorizationStore } from "@/store/usePluginAuthorizationStore"

import { PluginPermissionsCard } from "../plugin-permissions-card"

vi.mock("@slab/i18n", () => setupSlabI18nMock())

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
  it("shows the empty state when there are no grants", () => {
    render(<PluginPermissionsCard />)

    expect(screen.getByText("pages.plugins.permissions.management.empty")).toBeInTheDocument()
    expect(screen.queryByTestId("plugin-permissions-revoke-all")).not.toBeInTheDocument()
  })

  it("revokes every granted plugin via the revoke-all button", async () => {
    const user = userEvent.setup()
    usePluginAuthorizationStore.setState({
      grants: { "plugin-a": ["perm1", "perm2"], "plugin-b": [] },
    })

    render(<PluginPermissionsCard />)

    await user.click(screen.getByTestId("plugin-permissions-revoke-all"))

    expect(revoke.fn).toHaveBeenCalledExactlyOnceWith("plugin-a")
  })

  it("revokes a single permission", async () => {
    const user = userEvent.setup()
    usePluginAuthorizationStore.setState({
      grants: { "plugin-a": ["perm1"] },
    })

    render(<PluginPermissionsCard />)

    await user.click(screen.getByTestId("plugin-permissions-revoke-plugin-a-perm1"))

    expect(revoke.fn).toHaveBeenCalledExactlyOnceWith("plugin-a", "perm1")
  })
})
