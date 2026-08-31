import { render } from "vitest-browser-react"
import { userEvent } from "vitest/browser"
import { beforeEach, describe, expect, it, vi } from "vitest"

const platform = vi.hoisted(() => ({
    desktop: true,
    pickFolder: vi.fn(),
}))

const storeState = vi.hoisted(() => ({
    recentWorkspaces: [
        { rootPath: "C:\\recent-a", name: "recent-a", lastOpenedAt: 1 },
        { rootPath: "C:\\recent-b", name: "recent-b", lastOpenedAt: 2 },
    ],
}))

vi.mock("@slab/ui/provider/slab-provider", () => ({
    useSlab: () => ({
        ports: {
            platformInfo: { desktop: platform.desktop, mobile: false, os: "unknown" },
            fileDialog: { pickFolder: platform.pickFolder },
        },
    }),
}))

vi.mock("@slab/i18n", () => ({
    useTranslation: () => ({ t: (key: string) => key }),
}))

vi.mock("@slab/ui/store/useWorkspaceUiStore", () => ({
    useWorkspaceUiStore: (selector: (s: typeof storeState) => unknown) => selector(storeState),
}))

import { WorkspaceSelector } from "../workspace-selector"

function baseProps(overrides: Record<string, unknown> = {}) {
    return {
        value: { kind: "global" as const },
        onValueChange: vi.fn(),
        currentWorkspace: null,
        ...overrides,
    }
}

async function openDropdown(screen: Awaited<ReturnType<typeof render>>) {
    await userEvent.click(screen.getByTestId("workspace-selector").getByRole("button"))
}

describe("WorkspaceSelector", () => {
    beforeEach(() => {
        vi.clearAllMocks()
        platform.desktop = true
    })

    it("always offers the global option, even with an active workspace", async () => {
        const onValueChange = vi.fn()
        const screen = await render(
            <WorkspaceSelector
                {...baseProps({
                    value: { kind: "root", rootPath: "C:\\cur", name: "cur" },
                    onValueChange,
                    currentWorkspace: { rootPath: "C:\\cur", name: "cur" },
                })}
            />,
        )
        await openDropdown(screen)

        const global = screen.getByTestId("workspace-selector-item-global")
        await expect.element(global).toBeInTheDocument()
        await userEvent.click(global)
        expect(onValueChange).toHaveBeenCalledWith({ kind: "global" })
    })

    it("lists the current workspace and recents (deduped against current)", async () => {
        const screen = await render(
            <WorkspaceSelector
                {...baseProps({
                    value: { kind: "root", rootPath: "C:\\recent-b", name: "recent-b" },
                    currentWorkspace: { rootPath: "C:\\recent-b", name: "recent-b" },
                })}
            />,
        )
        await openDropdown(screen)

        // Current workspace section renders.
        await expect.element(screen.getByTestId("workspace-selector-item-current")).toBeInTheDocument()
        // recent-b is deduped against current; only recent-a remains in recents.
        const recents = screen.container.querySelectorAll(
            '[data-testid^="workspace-selector-item-recent"]',
        )
        expect(recents).toHaveLength(1)
        expect(recents[0]?.textContent).toContain("recent-a")
    })

    it("emits a root selection with name for a recent workspace", async () => {
        const onValueChange = vi.fn()
        const screen = await render(<WorkspaceSelector {...baseProps({ onValueChange })} />)
        await openDropdown(screen)

        await userEvent.click(screen.getByTestId("workspace-selector-item-recent-C:\\recent-a"))
        expect(onValueChange).toHaveBeenCalledWith({
            kind: "root",
            rootPath: "C:\\recent-a",
            name: "recent-a",
        })
    })

    it("picks a folder through the native dialog when on desktop", async () => {
        platform.pickFolder.mockResolvedValue("C:\\picked")
        const onValueChange = vi.fn()
        const screen = await render(<WorkspaceSelector {...baseProps({ onValueChange })} />)
        await openDropdown(screen)

        await userEvent.click(screen.getByTestId("workspace-selector-choose-folder"))
        expect(platform.pickFolder).toHaveBeenCalled()
        expect(onValueChange).toHaveBeenCalledWith({ kind: "root", rootPath: "C:\\picked" })
    })

    it("hides the folder picker on web (no native dialog)", async () => {
        platform.desktop = false
        const screen = await render(<WorkspaceSelector {...baseProps()} />)
        await openDropdown(screen)

        expect(screen.getByTestId("workspace-selector-choose-folder").query()).toBeNull()
    })
})
