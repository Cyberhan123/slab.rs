import { render, screen } from '@testing-library/react';
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
  it('forwards title, description, variant and size to StateSurface', () => {
    render(<EmptyPanel icon={Boxes} title="No plugins" description="Install one to begin" />);

    const surface = screen.getByTestId('state-surface');
    expect(surface).toHaveAttribute('data-variant', 'empty');
    expect(surface).toHaveAttribute('data-size', 'compact');
    expect(screen.getByText('No plugins')).toBeInTheDocument();
    expect(screen.getByText('Install one to begin')).toBeInTheDocument();
  });
});
