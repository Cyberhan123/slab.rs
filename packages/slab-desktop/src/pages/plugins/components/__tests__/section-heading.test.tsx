import { render, screen } from '@testing-library/react';
import { Boxes } from 'lucide-react';
import { describe, expect, it } from 'vitest';

import { SectionHeading } from '../section-heading';

describe('SectionHeading', () => {
  it('renders the icon, title and optional action', () => {
    render(
      <SectionHeading icon={Boxes} title="Installed" action={<button type="button">refresh</button>} />,
    );

    expect(screen.getByText('Installed')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'refresh' })).toBeInTheDocument();
  });

  it('renders without an action', () => {
    render(<SectionHeading icon={Boxes} title="Catalog" />);

    expect(screen.getByText('Catalog')).toBeInTheDocument();
  });
});
