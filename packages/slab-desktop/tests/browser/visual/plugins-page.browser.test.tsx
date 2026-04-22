import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import PluginsPage from '@/pages/plugins';
import type { PluginInfo } from '@/lib/plugin-sdk';
import { renderDesktopScene } from '../test-utils';

const { mockIsTauri } = vi.hoisted(() => ({
  mockIsTauri: vi.fn<() => boolean>(),
}));

vi.mock('@/hooks/use-tauri', () => ({
  isTauri: mockIsTauri,
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: vi.fn<() => void>(),
  usePageHeaderControl: vi.fn<() => void>(),
}));

vi.mock('@/lib/plugin-sdk', () => ({
  pluginList: vi.fn().mockResolvedValue([]),
  pluginCall: vi.fn().mockResolvedValue({ outputText: '{}', outputBase64: '' }),
  pluginApiRequest: vi.fn().mockResolvedValue({ status: 200, headers: {}, body: '{}' }),
  pluginMountView: vi.fn().mockResolvedValue(undefined),
  pluginUnmountView: vi.fn().mockResolvedValue(undefined),
  pluginUpdateViewBounds: vi.fn().mockResolvedValue(undefined),
  pluginOnEvent: vi.fn().mockResolvedValue(() => {}),
}));

function createMockPlugin(overrides: Partial<PluginInfo> = {}): PluginInfo {
  return {
    id: 'plugin-example',
    name: 'Example Plugin',
    version: '1.0.0',
    valid: true,
    error: null,
    manifestVersion: 1,
    compatibility: {},
    networkMode: 'blocked',
    allowHosts: [],
    contributions: {
      routes: [],
      sidebar: [],
      commands: [],
      settings: [],
      agentCapabilities: [],
    },
    permissions: {
      network: {
        mode: 'blocked',
        allowHosts: [],
      },
      ui: [],
      agent: [],
      slabApi: [],
      files: {
        read: [],
        write: [],
      },
    },
    ...overrides,
  };
}

describe('PluginsPage browser visual regression', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('captures the plugins page non-Tauri fallback state', async () => {
    mockIsTauri.mockReturnValue(false);

    await renderDesktopScene(<PluginsPage />, { route: '/plugins' });

    await expect
      .element(page.getByText('Plugins require Tauri desktop runtime'))
      .toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'plugins-page-non-tauri.png',
    );
  });

  it('captures the plugins page empty state in Tauri', async () => {
    mockIsTauri.mockReturnValue(true);

    const pluginSdk = await import('@/lib/plugin-sdk');
    vi.mocked(pluginSdk.pluginList).mockResolvedValue([]);

    await renderDesktopScene(<PluginsPage />, { route: '/plugins' });

    // Wait for the initial load
    await new Promise((resolve) => setTimeout(resolve, 100));

    await expect.element(page.getByText('No plugins found')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('plugins-page-empty.png');
  });

  it('captures the plugins page with plugins loaded in Tauri', async () => {
    mockIsTauri.mockReturnValue(true);

    const mockPlugins: PluginInfo[] = [
      createMockPlugin({
        id: 'plugin-1',
        name: 'Image Enhancer',
        version: '2.1.0',
        valid: true,
      }),
      createMockPlugin({
        id: 'plugin-2',
        name: 'Code Formatter',
        version: '1.5.3',
        valid: true,
      }),
      createMockPlugin({
        id: 'plugin-3',
        name: 'Broken Plugin',
        version: '0.0.1',
        valid: false,
        error: 'Missing manifest.json',
      }),
    ];

    const pluginSdk = await import('@/lib/plugin-sdk');
    vi.mocked(pluginSdk.pluginList).mockResolvedValue(mockPlugins);

    await renderDesktopScene(<PluginsPage />, { route: '/plugins' });

    // Wait for the initial load
    await new Promise((resolve) => setTimeout(resolve, 100));

    await expect.element(page.getByText('Image Enhancer')).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('plugins-page-with-plugins.png');
  });

  it('captures the plugins page loading state in Tauri', async () => {
    mockIsTauri.mockReturnValue(true);

    const pluginSdk = await import('@/lib/plugin-sdk');
    // Create a pending promise that never resolves
    const pendingPromise = new Promise<PluginInfo[]>(() => {});
    vi.mocked(pluginSdk.pluginList).mockReturnValue(pendingPromise as any);

    await renderDesktopScene(<PluginsPage />, { route: '/plugins' });

    await expect.element(page.getByText(/refresh/i)).toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot('plugins-page-loading.png');
  });
});
