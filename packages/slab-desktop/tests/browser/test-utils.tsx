import type { ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';
import { page, userEvent } from 'vitest/browser';
import { expect } from 'vitest';
import { render } from 'vitest-browser-react';

import { Toaster } from '@slab/components/sonner';
import { TooltipProvider } from '@slab/components/tooltip';
import { GlobalHeaderProvider } from '@/layouts/global-header-provider';

type DesktopSceneOptions = {
  route?: string;
};

export async function renderDesktopScene(
  ui: ReactNode,
  { route = '/setup' }: DesktopSceneOptions = {},
) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
      mutations: {
        retry: false,
      },
    },
  });

  return render(
    <main
      aria-label="Slab desktop scene"
      data-testid="desktop-browser-scene"
      className="min-h-screen bg-app-canvas px-6 py-8 text-foreground"
    >
      {ui}
    </main>,
    {
      wrapper: ({ children }) => (
        <MemoryRouter initialEntries={[route]}>
          <QueryClientProvider client={queryClient}>
            <TooltipProvider>
              <GlobalHeaderProvider>
                {children}
                <Toaster />
              </GlobalHeaderProvider>
            </TooltipProvider>
          </QueryClientProvider>
        </MemoryRouter>
      ),
    },
  );
}

export async function expectDesktopSceneAccessible() {
  await expect.element(page.getByRole('main', { name: 'Slab desktop scene' })).toBeVisible();
}

export async function expectDesktopSceneKeyboardReachable() {
  await userEvent.tab();

  const scene = document.querySelector('[data-testid="desktop-browser-scene"]');
  const active = document.activeElement;

  expect(active).not.toBe(document.body);
  expect(scene?.contains(active)).toBe(true);
}
