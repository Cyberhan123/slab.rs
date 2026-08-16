import { render } from 'vitest-browser-react';
import { Boxes } from 'lucide-react';
import { describe, expect, it } from 'vitest';

import { SectionHeading } from '../section-heading';

describe('SectionHeading', () => {
  it('renders the icon, title and optional action', async () => {
    const screen = await render(
      <SectionHeading icon={Boxes} title="Installed" action={<button type="button">refresh</button>} />,
    );

    await expect.element(screen.getByText('Installed')).toBeInTheDocument();
    await expect.element(screen.getByRole('button', { name: 'refresh' })).toBeInTheDocument();
  });

  it('renders without an action', async () => {
    const screen = await render(<SectionHeading icon={Boxes} title="Catalog" />);

    await expect.element(screen.getByText('Catalog')).toBeInTheDocument();
  });
});
