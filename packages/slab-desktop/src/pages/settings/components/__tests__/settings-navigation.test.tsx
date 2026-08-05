import { userEvent } from 'vitest/browser';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-react';

import { SettingsNavigation } from '../settings-navigation';

vi.mock('@slab/i18n', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { resolvedLanguage: 'en-US', language: 'en-US' },
  }),
  translateServerField: (_i18n: unknown, _field: unknown, fallback: string) => fallback,
}));

vi.mock('@slab/components/badge', () => ({
  Badge: ({ children }: { children: ReactNode }) => <span data-testid="count-badge">{children}</span>,
}));

const sections = [
  { id: 'general', title: 'General', i18n: null, subsections: [] },
  { id: 'models', title: 'Models', i18n: null, subsections: [] },
] as never;

describe('SettingsNavigation', () => {
  it('renders a nav entry per section', async () => {
    const screen = await render(
      <SettingsNavigation activeSectionId="general" sections={sections} onSelectSection={vi.fn<(id: string) => void>()} />,
    );

    await expect.element(screen.getByRole('button', { name: /General/ })).toBeInTheDocument();
    await expect.element(screen.getByRole('button', { name: /^Models$/ })).toBeInTheDocument();
  });

  it('reports the selected section on click', async () => {
    const onSelectSection = vi.fn<(id: string) => void>();
    const screen = await render(
      <SettingsNavigation activeSectionId="general" sections={sections} onSelectSection={onSelectSection} />,
    );

    await userEvent.click(screen.getByRole('button', { name: /^Models$/ }));

    expect(onSelectSection).toHaveBeenCalledExactlyOnceWith('models');
  });

  it('shows the property-count badge only for the active section', async () => {
    const screen = await render(
      <SettingsNavigation activeSectionId="models" sections={sections} onSelectSection={vi.fn<(id: string) => void>()} />,
    );

    const activeButton = screen.getByRole('button', { name: /Models/ }).element();
    const inactiveButton = screen.getByRole('button', { name: /General/ }).element();
    expect(activeButton?.querySelector('[data-testid="count-badge"]')).not.toBeNull();
    expect(inactiveButton?.querySelector('[data-testid="count-badge"]')).toBeNull();
  });
});
