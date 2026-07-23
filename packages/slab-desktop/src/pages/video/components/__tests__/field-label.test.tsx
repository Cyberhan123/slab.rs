import { render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { FieldLabel } from '../field-label';

vi.mock('@slab/components/label', () => ({
  Label: ({
    children,
    className,
    ...rest
  }: { children: ReactNode; className?: string } & Record<string, unknown>) => (
    <label data-class={className} {...rest}>
      {children}
    </label>
  ),
}));

describe('FieldLabel', () => {
  it('renders its children with the merged className', () => {
    render(<FieldLabel className="extra-cls">Height</FieldLabel>);

    const label = screen.getByText('Height');
    const className = label.getAttribute('data-class') ?? '';
    expect(className).toContain('uppercase');
    expect(className).toContain('extra-cls');
  });
});
