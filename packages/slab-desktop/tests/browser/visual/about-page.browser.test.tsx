import { page } from 'vitest/browser';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import AboutPage from '@/pages/about';
import { renderDesktopScene } from '../test-utils';

// Mock the global header meta hook
const { mockUsePageHeader } = vi.hoisted(() => ({
  mockUsePageHeader: vi.fn<() => void>(),
}));

vi.mock('@/hooks/use-global-header-meta', () => ({
  usePageHeader: mockUsePageHeader,
  usePageHeaderControl: vi.fn<() => void>(),
}));

describe('AboutPage browser visual regression', () => {
  beforeEach(() => {
    mockUsePageHeader.mockReset();
  });

  it('captures the about page layout', async () => {
    await renderDesktopScene(<AboutPage />, { route: '/about' });

    await expect
      .element(page.getByRole('heading', { name: 'About Slab App' }))
      .toBeVisible();
    await expect(page.getByTestId('desktop-browser-scene')).toMatchScreenshot(
      'about-page.png',
    );
  });
});
