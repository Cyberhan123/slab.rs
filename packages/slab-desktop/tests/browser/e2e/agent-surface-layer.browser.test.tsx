import { page, userEvent } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useEffect } from 'react';
import { Route, Routes } from 'react-router-dom';

import Layout from '@/layouts';
import { useAgentSurfaceStore } from '@/store/useAgentSurfaceStore';
import { renderDesktopScene } from '../test-utils';

vi.mock('@/pages/plugins/hooks/use-runtime-plugins', () => ({
  useRuntimePlugins: vi.fn<() => unknown>(() => ({
    data: [],
  })),
}));

function RouteMarker({ id, label }: { id: string; label: string }) {
  useEffect(() => {
    const key = `route-mounted:${id}`;
    window.sessionStorage.setItem(key, String(Number(window.sessionStorage.getItem(key) ?? '0') + 1));
  }, [id]);

  return <div className="p-4" data-testid={`route-marker-${id}`}>{label}</div>;
}

async function renderSurfaceShell() {
  await renderDesktopScene(
    <Routes>
      <Route element={<Layout />} path="/">
        <Route index element={<RouteMarker id="assistant" label="Assistant route" />} />
        <Route path="workspace" element={<RouteMarker id="workspace" label="Workspace route" />} />
        <Route path="image" element={<RouteMarker id="image" label="Image route" />} />
        <Route path="hub" element={<RouteMarker id="hub" label="Hub route" />} />
        <Route path="plugins" element={<RouteMarker id="plugins" label="Plugins route" />} />
      </Route>
    </Routes>,
    { route: '/' },
  );
}

describe('layout agent surface layer e2e', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.sessionStorage.clear();
    useAgentSurfaceStore.setState({
      draft: null,
      focusComposerSignal: 0,
      pendingSurface: null,
    });
  });

  it('routes a2u surfaces through the layout shell without unmounting the assistant route', async () => {
    useAgentSurfaceStore.getState().setPendingSurface({
      type: 'workspace',
      payload: {
        revealPath: 'src/main.rs',
      },
    });

    await renderSurfaceShell();

    await expect.element(page.getByTestId('agent-surface-layer')).toBeVisible();
    await expect.element(page.getByTestId('agent-surface-live-region')).toHaveTextContent(
      'Agent surface opened.',
    );
    await expect.element(page.getByTestId('a2u-workspace-surface')).toHaveTextContent(
      'src/main.rs',
    );
    expect(useAgentSurfaceStore.getState().pendingSurface).toBeNull();
    expect(window.sessionStorage.getItem('route-mounted:assistant')).toBe('1');

    await page.getByTestId('agent-surface-close').click();

    await vi.waitFor(() => {
      expect(document.querySelector('[data-testid="agent-surface-layer"]')).toBeNull();
    });
    await expect.element(page.getByTestId('route-marker-assistant')).toBeVisible();
    expect(window.sessionStorage.getItem('route-mounted:assistant')).toBe('1');
  });

  it('preserves pin collapse and Escape behavior at the layout level', async () => {
    useAgentSurfaceStore.getState().setPendingSurface({
      type: 'workspace',
      payload: {
        revealPath: 'src/main.rs',
      },
    });

    await renderSurfaceShell();

    await page.getByTestId('agent-surface-collapse').click();
    await expect.element(page.getByTestId('agent-surface-collapsed')).toHaveTextContent(
      'Agent surface collapsed.',
    );
    expect(document.querySelector('[data-testid="a2u-workspace-surface"]')).toBeNull();
    await expect.element(page.getByTestId('agent-surface-collapse')).toHaveAttribute(
      'aria-expanded',
      'false',
    );

    await page.getByTestId('agent-surface-collapse').click();
    await expect.element(page.getByTestId('a2u-workspace-surface')).toHaveTextContent(
      'src/main.rs',
    );

    await page.getByTestId('agent-surface-pin').click();
    await expect.element(page.getByTestId('agent-surface-pinned-indicator')).toHaveTextContent(
      'Pinned',
    );
    useAgentSurfaceStore.getState().setPendingSurface({
      type: 'image',
      payload: {
        prompt: 'Generate a compact app icon',
      },
    });
    await vi.waitFor(() => {
      expect(useAgentSurfaceStore.getState().pendingSurface).toMatchObject({
        type: 'image',
      });
    });
    expect(document.querySelector('[data-testid="a2u-image-surface"]')).toBeNull();

    await page.getByTestId('agent-surface-pin').click();
    await expect.element(page.getByTestId('a2u-image-surface')).toHaveTextContent(
      'Generate a compact app icon',
    );

    await userEvent.keyboard('{Escape}');
    await vi.waitFor(() => {
      expect(document.querySelector('[data-testid="agent-surface-layer"]')).toBeNull();
    });
    await expect.element(page.getByTestId('agent-surface-live-region')).toHaveTextContent(
      'Agent surface closed.',
    );
  });

  it('leaves workspace reveal surfaces pending for the workspace route consumer', async () => {
    useAgentSurfaceStore.getState().setPendingSurface(
      {
        type: 'workspace',
        payload: {
          revealPath: 'src/lib.rs',
        },
      },
      { targetRoute: 'workspace' },
    );

    await renderSurfaceShell();

    expect(document.querySelector('[data-testid="agent-surface-layer"]')).toBeNull();
    expect(useAgentSurfaceStore.getState().pendingSurface).toMatchObject({
      type: 'workspace',
      targetRoute: 'workspace',
      payload: {
        revealPath: 'src/lib.rs',
      },
    });
  });

  it('re-queues workspace surfaces for the full workspace route', async () => {
    useAgentSurfaceStore.getState().setPendingSurface({
      type: 'workspace',
      payload: {
        revealPath: 'src/main.rs',
      },
    });

    await renderSurfaceShell();
    await page.getByTestId('agent-surface-open-workspace').click();

    await expect.element(page.getByTestId('route-marker-workspace')).toBeVisible();
    await vi.waitFor(() => {
      expect(useAgentSurfaceStore.getState().pendingSurface).toMatchObject({
        type: 'workspace',
        targetRoute: 'workspace',
        payload: {
          revealPath: 'src/main.rs',
        },
      });
    });
  });

  it('shows plugin.launch ask-risk surfaces as preview before explicit navigation', async () => {
    useAgentSurfaceStore.getState().setPendingSurface({
      type: 'plugin',
      payload: {
        pluginId: 'demo-plugin',
        surface: 'panel',
        payload: {
          taskId: 'task-1',
        },
      },
    });

    await renderSurfaceShell();

    await expect.element(page.getByTestId('a2u-plugin-surface')).toHaveTextContent(
      'demo-plugin',
    );
    await expect.element(page.getByTestId('a2u-plugin-surface')).toHaveTextContent('panel');
    await expect.element(page.getByTestId('agent-surface-open-plugins')).toBeVisible();
    expect(document.querySelector('[data-testid="route-marker-plugins"]')).toBeNull();

    await page.getByTestId('agent-surface-open-plugins').click();

    await expect.element(page.getByTestId('route-marker-plugins')).toBeVisible();
  });
});
