import { page } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { Route, Routes } from 'react-router-dom';

import Layout from '@/layouts';
import { renderDesktopScene } from '../test-utils';

vi.mock('@/pages/plugins/hooks/use-runtime-plugins', () => ({
  useRuntimePlugins: vi.fn<() => unknown>(() => ({
    data: [],
  })),
}));

function RouteMarker({ label }: { label: string }) {
  return <div className="p-4">{label}</div>;
}

describe('desktop shell navigation e2e', () => {
  it('routes through the real sidebar and marks the active item', async () => {
    await renderDesktopScene(
      <Routes>
        <Route element={<Layout />} path="/">
          <Route index element={<RouteMarker label="Assistant route" />} />
          <Route path="workspace" element={<RouteMarker label="Workspace route" />} />
          <Route path="image" element={<RouteMarker label="Image route" />} />
          <Route path="settings" element={<RouteMarker label="Settings route" />} />
        </Route>
      </Routes>,
      { route: '/' },
    );

    await expect.element(page.getByText('Assistant route')).toBeVisible();
    await expect.element(page.getByRole('link', { name: 'Assistant' })).toHaveAttribute(
      'aria-current',
      'page',
    );

    await page.getByRole('link', { name: 'Image' }).click();
    await expect.element(page.getByText('Image route')).toBeVisible();
    await expect.element(page.getByRole('link', { name: 'Image' })).toHaveAttribute(
      'aria-current',
      'page',
    );

    await page.getByRole('link', { name: 'Settings' }).click();
    await expect.element(page.getByText('Settings route')).toBeVisible();
    await expect.element(page.getByRole('link', { name: 'Settings' })).toHaveAttribute(
      'aria-current',
      'page',
    );
  });
});
