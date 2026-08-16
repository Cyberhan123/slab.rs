import { render } from 'vitest-browser-react';
import { Boxes } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';

import { EmptyPanel } from '../empty-panel';

vi.mock('@slab/components/state-surface', () => ({
  StateSurface: ({
    title,
    description,
    variant,
    size,
  }: {
    title: string;
    description: string;
    variant?: string;
    size?: string;
    icon?: unknown;
  }) => (
    <div data-testid="state-surface" data-variant={variant} data-size={size}>
      <span>{title}</span>
      <span>{description}</span>
    </div>
  ),
}));

describe('EmptyPanel', () => {
  it('forwards title, description, variant and size to StateSurface', async () => {
    const screen = await render(
      <EmptyPanel icon={Boxes} title="No plugins" description="Install one to begin" />,
    );

    const surface = screen.getByTestId('state-surface');
    await expect.element(surface).toHaveAttribute('data-variant', 'empty');
    await expect.element(surface).toHaveAttribute('data-size', 'compact');
    await expect.element(screen.getByText('No plugins')).toBeInTheDocument();
    await expect.element(screen.getByText('Install one to begin')).toBeInTheDocument();
  });
});
