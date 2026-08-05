import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-react';

import { SliderField } from '../slider-field';

vi.mock('@slab/components/label', () => ({
  Label: ({ children }: { children: ReactNode }) => <label>{children}</label>,
}));

describe('SliderField', () => {
  it('renders the label, value and slider node', async () => {
    const screen = await render(
      <SliderField
        label="Size"
        value={512}
        slider={<input data-testid="slider" aria-label="slider" type="range" onChange={vi.fn()} />}
      />,
    );

    await expect.element(screen.getByText('Size')).toBeInTheDocument();
    await expect.element(screen.getByText('512')).toBeInTheDocument();
    await expect.element(screen.getByTestId('slider')).toBeInTheDocument();
  });
});
