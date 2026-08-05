import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-react';

import { SummaryStat } from '../summary-stat';

describe('SummaryStat', () => {
  it('renders the label, value and hint', async () => {
    const screen = await render(<SummaryStat label="Models" value="4" hint="2 local" />);

    await expect.element(screen.getByText('Models')).toBeInTheDocument();
    await expect.element(screen.getByText('4')).toBeInTheDocument();
    await expect.element(screen.getByText('2 local')).toBeInTheDocument();
  });
});
