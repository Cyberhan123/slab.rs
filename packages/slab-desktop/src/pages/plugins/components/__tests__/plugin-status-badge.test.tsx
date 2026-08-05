import { render } from 'vitest-browser-react';
import { describe, expect, it, vi } from 'vitest';

import { setupSlabI18nMock } from '@slab/test-utils/mocks';

import { PluginStatusBadge } from '../plugin-status-badge';

vi.mock('@slab/i18n', () => setupSlabI18nMock());

describe('PluginStatusBadge', () => {
  it('renders the translated status label', async () => {
    const screen = await render(<PluginStatusBadge status="running" />);

    await expect.element(screen.getByText('pages.plugins.status.running')).toBeInTheDocument();
  });

  it('forces the working status while busy', async () => {
    const screen = await render(<PluginStatusBadge status="idle" busy />);

    await expect.element(screen.getByText('pages.plugins.status.working')).toBeInTheDocument();
    await expect.element(screen.getByText('pages.plugins.status.idle')).not.toBeInTheDocument();
  });

  it('applies the invalid styling for an invalid status', async () => {
    const screen = await render(<PluginStatusBadge status="invalid" />);

    expect(screen.getByText('pages.plugins.status.invalid').element().className).toContain(
      'text-destructive',
    );
  });
});
