import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { PaginationButton } from '../pagination-button';

describe('PaginationButton', () => {
  it('renders its children and fires onClick', async () => {
    const user = userEvent.setup();
    const onClick = vi.fn<() => void>();
    render(<PaginationButton onClick={onClick}>5</PaginationButton>);

    await user.click(screen.getByRole('button', { name: '5' }));

    expect(onClick).toHaveBeenCalledOnce();
  });

  it('applies the active styling when active', () => {
    render(<PaginationButton active>1</PaginationButton>);

    expect(screen.getByRole('button', { name: '1' }).className).toContain('bg-[var(--brand-teal)]');
  });

  it('forwards the disabled attribute', () => {
    render(<PaginationButton disabled>2</PaginationButton>);

    expect(screen.getByRole('button', { name: '2' })).toBeDisabled();
  });
});
