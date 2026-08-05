import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-react';

import { setupSlabI18nMock } from '@slab/test-utils/mocks';

import { StatusBadge } from '../status-badge';

vi.mock('@slab/i18n', () => setupSlabI18nMock());

vi.mock('@slab/components/workspace', () => ({
  StatusPill: ({
    children,
    status,
  }: {
    children: ReactNode;
    status?: string;
  }) => (
    <span data-testid="status-pill" data-status={status}>
      {children}
    </span>
  ),
}));

describe('StatusBadge', () => {
  it.each([
    ['ready', 'pages.hub.filters.statuses.ready'],
    ['downloading', 'pages.hub.filters.statuses.downloading'],
    ['error', 'pages.hub.filters.statuses.error'],
    ['not_downloaded', 'pages.hub.filters.statuses.not_downloaded'],
  ])('renders the %s status', async (status, label) => {
    const screen = await render(<StatusBadge status={status as never} />);
    await expect.element(screen.getByText(label)).toBeInTheDocument();
  });
});
