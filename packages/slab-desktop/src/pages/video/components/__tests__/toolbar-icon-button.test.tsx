import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { ToolbarIconButton } from '../toolbar-icon-button';

const icon = ({ className }: { className?: string }): ReactNode => (
  <svg data-testid="ico" className={className} />
);

describe('ToolbarIconButton', () => {
  it('renders the icon, exposes the label and fires onClick', async () => {
    const user = userEvent.setup();
    const onClick = vi.fn<() => void>();
    render(<ToolbarIconButton icon={icon} label="Play" onClick={onClick} />);

    await user.click(screen.getByRole('button', { name: 'Play' }));

    expect(onClick).toHaveBeenCalledOnce();
    expect(screen.getByTestId('ico')).toBeInTheDocument();
  });

  it('applies the active styling when active', () => {
    render(<ToolbarIconButton icon={icon} label="Play" active onClick={vi.fn<() => void>()} />);

    expect(screen.getByRole('button', { name: 'Play' }).className).toContain('shadow-elevation-2');
  });

  it('disables the button when disabled is set', () => {
    render(<ToolbarIconButton icon={icon} label="Play" disabled onClick={vi.fn<() => void>()} />);

    expect(screen.getByRole('button', { name: 'Play' })).toBeDisabled();
  });
});
