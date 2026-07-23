import { render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { SliderField } from '../slider-field';

vi.mock('@slab/components/label', () => ({
  Label: ({ children }: { children: ReactNode }) => <label>{children}</label>,
}));

describe('SliderField', () => {
  it('renders the label, value and slider node', () => {
    render(
      <SliderField
        label="Size"
        value={512}
        slider={<input data-testid="slider" aria-label="slider" type="range" onChange={vi.fn()} />}
      />,
    );

    expect(screen.getByText('Size')).toBeInTheDocument();
    expect(screen.getByText('512')).toBeInTheDocument();
    expect(screen.getByTestId('slider')).toBeInTheDocument();
  });
});
