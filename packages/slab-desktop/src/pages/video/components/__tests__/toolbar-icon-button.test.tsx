import { userEvent } from 'vitest/browser';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-react';

import { ToolbarIconButton } from '../toolbar-icon-button';

const icon = ({ className }: { className?: string }): ReactNode => (
  <svg data-testid="ico" className={className} />
);

describe('ToolbarIconButton', () => {
  it('renders the icon, exposes the label and fires onClick', async () => {
    const onClick = vi.fn<() => void>();
    const screen = await render(<ToolbarIconButton icon={icon} label="Play" onClick={onClick} />);

    await userEvent.click(screen.getByRole('button', { name: 'Play' }));

    expect(onClick).toHaveBeenCalledOnce();
    await expect.element(screen.getByTestId('ico')).toBeInTheDocument();
  });

  it('applies the active styling when active', async () => {
    const screen = await render(<ToolbarIconButton icon={icon} label="Play" active onClick={vi.fn<() => void>()} />);

    expect(screen.getByRole('button', { name: 'Play' }).element().className).toContain('shadow-elevation-2');
  });

  it('disables the button when disabled is set', async () => {
    const screen = await render(
      <ToolbarIconButton icon={icon} label="Play" disabled onClick={vi.fn<() => void>()} />,
    );

    await expect.element(screen.getByRole('button', { name: 'Play' })).toBeDisabled();
  });
});
