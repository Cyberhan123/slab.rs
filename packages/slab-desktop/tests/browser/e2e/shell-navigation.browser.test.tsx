import { page, userEvent } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { createMemoryRouter, Route, Routes } from 'react-router-dom';

import Layout from '@/layouts';
import { renderDesktopScene } from '../test-utils';

vi.mock('@/pages/plugins/hooks/use-runtime-plugins', () => ({
  useRuntimePlugins: vi.fn<() => unknown>(() => ({
    data: [],
  })),
}));

function RouteMarker({ id, label }: { id: string; label: string }) {
  return <div className="p-4" data-testid={`route-marker-${id}`}>{label}</div>;
}

function CrashingRoute(): never {
  throw new Error('Synthetic page crash');
}

describe('desktop shell navigation e2e', () => {
  it('routes through the real sidebar and marks the active item', async () => {
    await renderDesktopScene(
      <Routes>
        <Route element={<Layout />} path="/">
          <Route index element={<RouteMarker id="assistant" label="Assistant route" />} />
          <Route path="workspace" element={<RouteMarker id="workspace" label="Workspace route" />} />
          <Route path="image" element={<RouteMarker id="image" label="Image route" />} />
          <Route path="settings" element={<RouteMarker id="settings" label="Settings route" />} />
        </Route>
      </Routes>,
      { route: '/' },
    );

    await expect.element(page.getByTestId('route-marker-assistant')).toBeVisible();
    await expect.element(page.getByTestId('sidebar-link-assistant')).toHaveAttribute(
      'aria-current',
      'page',
    );

    await page.getByTestId('sidebar-link-image').click();
    await expect.element(page.getByTestId('route-marker-image')).toBeVisible();
    await expect.element(page.getByTestId('sidebar-link-image')).toHaveAttribute(
      'aria-current',
      'page',
    );

    await page.getByTestId('sidebar-link-settings').click();
    await expect.element(page.getByTestId('route-marker-settings')).toBeVisible();
    await expect.element(page.getByTestId('sidebar-link-settings')).toHaveAttribute(
      'aria-current',
      'page',
    );
  });

  it('keeps sidebar navigation keyboard focusable and aria-current accurate', async () => {
    await renderDesktopScene(
      <Routes>
        <Route element={<Layout />} path="/">
          <Route index element={<RouteMarker id="assistant" label="Assistant route" />} />
          <Route path="workspace" element={<RouteMarker id="workspace" label="Workspace route" />} />
        </Route>
      </Routes>,
      { route: '/' },
    );

    const workspaceLinkElement = document.querySelector<HTMLElement>(
      '[data-testid="sidebar-link-workspace"]',
    );
    expect(workspaceLinkElement).not.toBeNull();

    workspaceLinkElement?.focus();
    expect(document.activeElement).toBe(workspaceLinkElement);

    await userEvent.keyboard('{Enter}');

    await expect.element(page.getByTestId('route-marker-workspace')).toBeVisible();
    await expect.element(page.getByTestId('sidebar-link-workspace')).toHaveAttribute('aria-current', 'page');
  });

  it('contains page crashes inside the shell and lets users go back', async () => {
    const router = createMemoryRouter(
      [
        {
          path: '/',
          element: <Layout />,
          children: [
            { index: true, element: <RouteMarker id="assistant" label="Assistant route" /> },
            { path: 'settings', element: <CrashingRoute /> },
          ],
        },
      ],
      { initialEntries: ['/'] },
    );

    await renderDesktopScene(null, { router });

    await page.getByTestId('sidebar-link-settings').click();

    await expect.element(page.getByText('Page crashed')).toBeVisible();
    expect(document.querySelector('header.shell-topbar')).not.toBeNull();
    await expect.element(page.getByTestId('sidebar-link-settings')).toBeVisible();

    const details = document.querySelector<HTMLDetailsElement>('details');
    expect(details).not.toBeNull();
    expect(details?.open).toBe(false);

    await page.getByRole('button', { name: 'Back' }).click();
    await expect.element(page.getByTestId('route-marker-assistant')).toBeVisible();
  });
});
