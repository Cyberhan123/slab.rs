import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { SummaryStat } from '../summary-stat';

describe('SummaryStat', () => {
  it('renders the label, value and hint', () => {
    render(<SummaryStat label="Models" value="4" hint="2 local" />);

    expect(screen.getByText('Models')).toBeInTheDocument();
    expect(screen.getByText('4')).toBeInTheDocument();
    expect(screen.getByText('2 local')).toBeInTheDocument();
  });
});
