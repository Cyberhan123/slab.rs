import { userEvent } from 'vitest/browser';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-react';

import { PaginationButton } from '../pagination-button';

describe('PaginationButton', () => {
  it('renders its children and fires onClick', async () => {
    const onClick = vi.fn<() => void>();
    const screen = await render(<PaginationButton onClick={onClick}>5</PaginationButton>);

    await userEvent.click(screen.getByRole('button', { name: '5' }));

    expect(onClick).toHaveBeenCalledOnce();
  });

  it('applies the active styling when active', async () => {
    const screen = await render(<PaginationButton active>1</PaginationButton>);

    expect(screen.getByRole('button', { name: '1' }).element()?.className).toContain('bg-[var(--brand-teal)]');
  });

  it('forwards the disabled attribute', async () => {
    const screen = await render(<PaginationButton disabled>2</PaginationButton>);

    await expect.element(screen.getByRole('button', { name: '2' })).toBeDisabled();
  });
});
