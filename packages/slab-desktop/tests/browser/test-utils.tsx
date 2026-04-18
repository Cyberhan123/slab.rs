import type { ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';
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
    <div
      data-testid="desktop-browser-scene"
      className="min-h-screen bg-app-canvas px-6 py-8 text-foreground"
    >
      {ui}
    </div>,
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
