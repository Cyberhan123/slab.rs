import type { ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createMemoryRouter, RouterProvider } from 'react-router-dom';
import { page, userEvent } from 'vitest/browser';
import { expect } from 'vitest';
import { render } from 'vitest-browser-react';

import { Toaster } from '@slab/components/sonner';
import { TooltipProvider } from '@slab/components/tooltip';
import { GlobalHeaderProvider } from '@/layouts/global-header-provider';

type DesktopSceneOptions = {
  route?: string;
  router?: Parameters<typeof RouterProvider>[0]['router'];
};

export async function renderDesktopScene(
  ui: ReactNode,
  { route = '/setup', router }: DesktopSceneOptions = {},
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
  if (router) {
    return render(
      <main
        aria-label="Slab desktop scene"
        data-testid="desktop-browser-scene"
        className="min-h-screen bg-app-canvas px-6 py-8 text-foreground"
      >
        <QueryClientProvider client={queryClient}>
          <TooltipProvider>
            <GlobalHeaderProvider>
              <RouterProvider router={router} />
              <Toaster />
            </GlobalHeaderProvider>
          </TooltipProvider>
        </QueryClientProvider>
      </main>,
    );
  }

  const memoryRouter = createMemoryRouter(
    [
      {
        path: '*',
        element: (
          <main
            aria-label="Slab desktop scene"
            data-testid="desktop-browser-scene"
            className="min-h-screen bg-app-canvas px-6 py-8 text-foreground"
          >
            <QueryClientProvider client={queryClient}>
              <TooltipProvider>
                <GlobalHeaderProvider>
                  {ui}
                  <Toaster />
                </GlobalHeaderProvider>
              </TooltipProvider>
            </QueryClientProvider>
          </main>
        ),
      },
    ],
    { initialEntries: [route] },
  );

  return render(<RouterProvider router={memoryRouter} />);
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
