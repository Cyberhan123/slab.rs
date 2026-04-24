import { page } from "vitest/browser";
import { beforeEach, describe, expect, it, vi } from "vitest";

import PluginsPage from "@/pages/plugins";
import type { components } from "@/lib/api/v1.d.ts";
import { renderDesktopScene } from "../test-utils";

type PluginRecord = components["schemas"]["PluginResponse"];

const {
  mockApiState,
  mockIsTauri,
  mockUseMutation,
  mockUseQuery,
} = vi.hoisted(() => ({
  mockApiState: {
    plugins: [] as unknown[],
    pluginsLoading: false,
    pluginsError: null as unknown,
  },
  mockIsTauri: vi.fn<() => boolean>(),
  mockUseMutation: vi.fn<(...args: unknown[]) => unknown>(),
  mockUseQuery: vi.fn<(...args: unknown[]) => unknown>(),
}));

vi.mock("@/hooks/use-tauri", () => ({
  isTauri: mockIsTauri,
}));

vi.mock("@/hooks/use-global-header-meta", () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderControl: vi.fn<() => void>(),
  usePageHeaderSearch: vi.fn<() => void>(),
}));

vi.mock("@/lib/api", () => ({
  default: {
    useMutation: mockUseMutation,
    useQuery: mockUseQuery,
  },
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : String(error)),
}));

vi.mock("@/lib/plugin-host-bridge", () => ({
  pluginRuntimeList: vi.fn<() => Promise<unknown[]>>().mockResolvedValue([]),
  pluginCall: vi.fn<() => Promise<{ outputText: string; outputBase64: string }>>().mockResolvedValue({
    outputText: "{}",
    outputBase64: "",
  }),
  pluginApiRequest: vi.fn<() => Promise<{ status: number; headers: Record<string, string>; body: string }>>()
    .mockResolvedValue({ status: 200, headers: {}, body: "{}" }),
  pluginMountView: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  pluginUnmountView: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  pluginUpdateViewBounds: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  pluginOnEvent: vi.fn<() => Promise<() => void>>().mockResolvedValue(() => {}),
}));

function createMockPlugin(overrides: Partial<PluginRecord> = {}): PluginRecord {
  return {
    id: "plugin-example",
    name: "Example Plugin",
    version: "1.0.0",
    valid: true,
    error: null,
    manifestVersion: 1,
    compatibility: {},
    uiEntry: "ui/index.html",
    hasWasm: false,
    networkMode: "blocked",
    allowHosts: [],
    contributions: {},
    permissions: {},
    sourceKind: "market_pack",
    sourceRef: "default",
    installRoot: "C:/Slab/plugins/plugin-example",
    installedVersion: "1.0.0",
    manifestHash: "abc123",
    enabled: true,
    runtimeStatus: "stopped",
    lastError: null,
    installedAt: null,
    updatedAt: null,
    lastSeenAt: null,
    lastStartedAt: null,
    lastStoppedAt: null,
    availableVersion: null,
    updateAvailable: false,
    removable: true,
    ...overrides,
  };
}

function configurePluginQueries() {
  mockUseQuery.mockImplementation(() => {
    return {
      data: mockApiState.plugins,
      error: mockApiState.pluginsError,
      isLoading: mockApiState.pluginsLoading,
      isFetching: mockApiState.pluginsLoading,
      refetch: vi.fn<() => Promise<{ data: unknown[]; error: unknown }>>().mockResolvedValue({
        data: mockApiState.plugins,
        error: mockApiState.pluginsError,
      }),
    };
  });

  mockUseMutation.mockReturnValue({
    mutateAsync: vi.fn<() => Promise<Record<string, never>>>().mockResolvedValue({}),
  });
}

describe("PluginsPage browser visual regression", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockApiState.plugins = [];
    mockApiState.pluginsLoading = false;
    mockApiState.pluginsError = null;
    configurePluginQueries();
  });

  it("captures the plugins page non-Tauri fallback state", async () => {
    mockIsTauri.mockReturnValue(false);

    await renderDesktopScene(<PluginsPage />, { route: "/plugins" });

    await expect
      .element(page.getByText("Plugins require Tauri desktop runtime"))
      .toBeVisible();
    await expect(page.getByTestId("desktop-browser-scene")).toMatchScreenshot(
      "plugins-page-non-tauri.png",
    );
  });

  it("captures the plugins page empty state in Tauri", async () => {
    mockIsTauri.mockReturnValue(true);

    await renderDesktopScene(<PluginsPage />, { route: "/plugins" });
    await new Promise((resolve) => setTimeout(resolve, 100));

    await expect.element(page.getByText("No installed plugins found.")).toBeVisible();
    await expect(page.getByTestId("desktop-browser-scene")).toMatchScreenshot(
      "plugins-page-empty.png",
    );
  });

  it("captures the plugins page with plugins loaded in Tauri", async () => {
    mockIsTauri.mockReturnValue(true);
    mockApiState.plugins = [
      createMockPlugin({
        id: "plugin-1",
        name: "Image Enhancer",
        version: "2.1.0",
      }),
      createMockPlugin({
        id: "plugin-2",
        name: "Code Formatter",
        version: "1.5.3",
        enabled: false,
      }),
      createMockPlugin({
        id: "plugin-3",
        name: "Broken Plugin",
        version: "0.0.1",
        valid: false,
        lastError: "Missing manifest.json",
      }),
    ];

    await renderDesktopScene(<PluginsPage />, { route: "/plugins" });
    await new Promise((resolve) => setTimeout(resolve, 100));

    await expect.element(page.getByRole("heading", { name: "Image Enhancer" })).toBeVisible();
    await expect(page.getByTestId("desktop-browser-scene")).toMatchScreenshot(
      "plugins-page-with-plugins.png",
    );
  });

  it("captures the plugins page loading state in Tauri", async () => {
    mockIsTauri.mockReturnValue(true);
    mockApiState.pluginsLoading = true;

    await renderDesktopScene(<PluginsPage />, { route: "/plugins" });

    await expect.element(page.getByText(/refresh/i)).toBeVisible();
    await expect(page.getByTestId("desktop-browser-scene")).toMatchScreenshot(
      "plugins-page-loading.png",
    );
  });
});
