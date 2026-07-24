import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { setupSlabI18nMock } from '@slab/test-utils/mocks';

import { PluginStatusBadge } from '../plugin-status-badge';

vi.mock('@slab/i18n', () => setupSlabI18nMock());

describe('PluginStatusBadge', () => {
  it('renders the translated status label', () => {
    render(<PluginStatusBadge status="running" />);

    expect(screen.getByText('pages.plugins.status.running')).toBeInTheDocument();
  });

  it('forces the working status while busy', () => {
    render(<PluginStatusBadge status="idle" busy />);

    expect(screen.getByText('pages.plugins.status.working')).toBeInTheDocument();
    expect(screen.queryByText('pages.plugins.status.idle')).not.toBeInTheDocument();
  });

  it('applies the invalid styling for an invalid status', () => {
    render(<PluginStatusBadge status="invalid" />);

    expect(screen.getByText('pages.plugins.status.invalid').className).toContain('text-destructive');
  });
});
