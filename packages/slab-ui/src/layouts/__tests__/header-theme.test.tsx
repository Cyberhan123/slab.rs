import { type ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';

import { applyAppLanguagePreference } from '@slab/i18n';
import Header from '@slab/ui/layouts/header';
import { HeaderProvider } from '@slab/ui/layouts/header-provider';
import { SlabProvider } from '@slab/ui/provider/slab-provider';
import { createTestSlabPorts } from '@slab/ui/provider/test-ports';
import { setThemePreference, THEME_STORAGE_KEY } from '@slab/ui/lib/theme';

function withProviders(children: ReactNode) {
  const queryClient = new QueryClient();
  return (
    <QueryClientProvider client={queryClient}>
      <SlabProvider deps={{ ports: createTestSlabPorts(), queryClient }}>
        <HeaderProvider>{children}</HeaderProvider>
      </SlabProvider>
    </QueryClientProvider>
  );
}

describe('header theme control', () => {
  beforeEach(async () => {
    // The browser locale follows the host OS (zh-CN on this machine); pin the
    // runtime language so the accessible menu names are deterministic.
    await applyAppLanguagePreference('en-US');
  });

  afterEach(() => {
    setThemePreference('system');
    window.localStorage.removeItem(THEME_STORAGE_KEY);
  });

  it('opens a three-option picker and applies the dark class on select', async () => {
    const screen = await render(withProviders(<Header />));

    const trigger = screen.getByTestId('header-theme-control');
    await userEvent.click(trigger);

    const darkItem = screen.getByRole('menuitem', { name: 'Dark' });
    await expect.element(screen.getByRole('menuitem', { name: 'Light' })).toBeInTheDocument();
    await expect.element(screen.getByRole('menuitem', { name: 'System' })).toBeInTheDocument();

    await userEvent.click(darkItem);
    expect(document.documentElement.classList.contains('dark')).toBe(true);
    expect(window.localStorage.getItem(THEME_STORAGE_KEY)).toBe('dark');
    expect(document.documentElement.style.colorScheme).toBe('dark');

    // Switching back to System releases the forced class (browser default is light).
    await userEvent.click(trigger);
    await userEvent.click(screen.getByRole('menuitem', { name: 'System' }));
    expect(window.localStorage.getItem(THEME_STORAGE_KEY)).toBe('system');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });
});
