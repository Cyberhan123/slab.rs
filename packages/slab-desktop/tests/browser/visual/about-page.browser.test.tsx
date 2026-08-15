import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';

import AboutPage from '@slab/ui/pages/about';
import { renderDesktopScene } from '../test-utils';

describe('AboutPage browser visual regression', () => {
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
