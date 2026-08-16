import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import { Boxes } from 'lucide-react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';


import { InstalledPluginCard } from '../installed-plugin-card';

vi.mock('@slab/i18n', async () => {
  const { setupSlabI18nMock } = await import('@slab/test-utils/mocks')
  return setupSlabI18nMock()
});

vi.mock('@slab/components/button', () => ({
  Button: ({
    children,
    onClick,
    disabled,
    ...rest
  }: {
    children: ReactNode;
    onClick?: () => void;
    disabled?: boolean;
  } & Record<string, unknown>) => (
    <button type="button" onClick={onClick} disabled={disabled} {...rest}>
      {children}
    </button>
  ),
}));

vi.mock('../plugin-status-badge', () => ({
  PluginStatusBadge: ({ status }: { status: string }) => (
    <span data-testid="status-badge">{status}</span>
  ),
}));

vi.mock('@slab/ui/components/error-data-detail', () => ({
  ErrorDataDetail: () => null,
}));

function plugin(overrides: Record<string, unknown> = {}) {
  return {
    id: 'p1',
    name: 'My Plugin',
    version: '1.0.0',
    enabled: true,
    valid: true,
    runtimeStatus: 'idle',
    lastError: null,
    error: null,
    updateAvailable: false,
    removable: true,
    hasWasm: false,
    uiEntry: null,
    ...overrides,
  } as never;
}

describe('InstalledPluginCard', () => {
  it('offers the launch action for an enabled, idle plugin', async () => {
    const screen = await render(
      <InstalledPluginCard
        plugin={plugin()}
        icon={Boxes}
        tone="teal"
        busy={false}
        actionError={null}
        onPrimaryAction={vi.fn<() => void>()}
        onToggleEnabled={vi.fn<() => void>()}
        onUpdate={vi.fn<() => void>()}
        onDelete={vi.fn<() => void>()}
      />,
    );

    await expect.element(screen.getByTestId('plugin-primary-action-p1')).toHaveTextContent(
      'pages.plugins.actions.launch',
    );
  });

  it('offers the enable action for a disabled plugin', async () => {
    const screen = await render(
      <InstalledPluginCard
        plugin={plugin({ enabled: false })}
        icon={Boxes}
        tone="teal"
        busy={false}
        actionError={null}
        onPrimaryAction={vi.fn<() => void>()}
        onToggleEnabled={vi.fn<() => void>()}
        onUpdate={vi.fn<() => void>()}
        onDelete={vi.fn<() => void>()}
      />,
    );

    await expect.element(screen.getByTestId('plugin-primary-action-p1')).toHaveTextContent(
      'pages.plugins.actions.enable',
    );
  });

  it('offers the stop action for a running plugin', async () => {
    const screen = await render(
      <InstalledPluginCard
        plugin={plugin({ runtimeStatus: 'running' })}
        icon={Boxes}
        tone="teal"
        busy={false}
        actionError={null}
        onPrimaryAction={vi.fn<() => void>()}
        onToggleEnabled={vi.fn<() => void>()}
        onUpdate={vi.fn<() => void>()}
        onDelete={vi.fn<() => void>()}
      />,
    );

    await expect.element(screen.getByTestId('plugin-primary-action-p1')).toHaveTextContent(
      'pages.plugins.actions.stop',
    );
  });

  it('fires the primary, update, toggle and delete callbacks', async () => {
    const onPrimaryAction = vi.fn<() => void>();
    const onUpdate = vi.fn<() => void>();
    const onToggleEnabled = vi.fn<() => void>();
    const onDelete = vi.fn<() => void>();
    const screen = await render(
      <InstalledPluginCard
        plugin={plugin({ updateAvailable: true })}
        icon={Boxes}
        tone="teal"
        busy={false}
        actionError={null}
        onPrimaryAction={onPrimaryAction}
        onToggleEnabled={onToggleEnabled}
        onUpdate={onUpdate}
        onDelete={onDelete}
      />,
    );

    await userEvent.click(screen.getByTestId('plugin-primary-action-p1'));
    await userEvent.click(screen.getByTestId('plugin-update-p1'));
    await userEvent.click(screen.getByTestId('plugin-toggle-enabled-p1'));
    await userEvent.click(screen.getByTestId('plugin-delete-p1'));

    expect(onPrimaryAction).toHaveBeenCalledOnce();
    expect(onUpdate).toHaveBeenCalledOnce();
    expect(onToggleEnabled).toHaveBeenCalledOnce();
    expect(onDelete).toHaveBeenCalledOnce();
  });

  it('shows an action error block when present', async () => {
    const screen = await render(
      <InstalledPluginCard
        plugin={plugin()}
        icon={Boxes}
        tone="teal"
        busy={false}
        actionError={{ message: 'failed to start', error: new Error('x') }}
        onPrimaryAction={vi.fn<() => void>()}
        onToggleEnabled={vi.fn<() => void>()}
        onUpdate={vi.fn<() => void>()}
        onDelete={vi.fn<() => void>()}
      />,
    );

    await expect.element(screen.getByText('failed to start')).toBeInTheDocument();
  });
});
