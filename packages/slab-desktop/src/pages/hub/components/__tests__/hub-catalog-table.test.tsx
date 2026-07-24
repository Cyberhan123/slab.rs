import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { setupSlabI18nMock } from '@slab/test-utils/mocks';

import { HubCatalogTable } from '../hub-catalog-table';

vi.mock('@slab/i18n', () => setupSlabI18nMock());

vi.mock('../../hooks/use-hub-model-catalog', () => ({
  canDownloadModel: () => true,
  canRunModelLifecycleAction: () => false,
  getModelUseRoute: () => 'assistant',
}));

vi.mock('@slab/components/badge', () => ({
  Badge: ({ children }: { children: ReactNode }) => <span>{children}</span>,
}));

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

vi.mock('@slab/components/progress', () => ({
  Progress: ({ value }: { value?: number }) => <div data-testid="progress" data-value={value} />,
}))

vi.mock('@slab/components/workspace', () => ({
  StageEmptyState: ({
    title,
    description,
  }: {
    title: string;
    description: string;
    icon?: unknown;
    className?: string;
  }) => (
    <div data-testid="empty-state">
      <span>{title}</span>
      <span>{description}</span>
    </div>
  ),
}));

vi.mock('../status-badge', () => ({
  StatusBadge: ({ status }: { status: string }) => <span data-testid="status-badge">{status}</span>,
}))

function model(overrides: Record<string, unknown> = {}) {
  return {
    id: 'm1',
    display_name: 'Test Model',
    kind: 'local',
    category: 'chat',
    repo_id: 'repo/test',
    filename: 'test.gguf',
    capabilities: [],
    backend_ids: ['ggml.llama'],
    is_vad_model: false,
    status: 'ready',
    local_path: '/models/test.gguf',
    pending: false,
    runtime_state: null,
    download_task_id: null,
    download_progress: null,
    size_bytes: 1000,
    vram_risk: 'low',
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  } as never;
}

function baseProps(overrides: Record<string, unknown> = {}) {
  return {
    models: [],
    deletePending: false,
    modelActionPending: false,
    modelActionPendingId: null,
    modelActionErrors: {},
    onDownloadClick: vi.fn<(model: unknown) => void>(),
    onEnhanceClick: vi.fn<(model: unknown) => void>(),
    onDeleteClick: vi.fn<(model: unknown) => void>(),
    onLoadClick: vi.fn<(model: unknown) => void>(),
    onSwitchClick: vi.fn<(model: unknown) => void>(),
    onUnloadClick: vi.fn<(model: unknown) => void>(),
    onUseClick: vi.fn<(model: unknown, route: string) => void>(),
    ...overrides,
  };
}

describe('HubCatalogTable', () => {
  it('renders the empty state when there are no models', () => {
    render(<HubCatalogTable {...baseProps()} />);

    expect(screen.getByText('pages.hub.catalog.emptyPageTitle')).toBeInTheDocument();
  });

  it('renders a card per model', () => {
    render(<HubCatalogTable {...baseProps({ models: [model()] })} />);

    expect(screen.getByTestId('hub-model-card-m1')).toBeInTheDocument();
    expect(screen.getByText('Test Model')).toBeInTheDocument();
  });

  it('routes the use action with the resolved route', async () => {
    const user = userEvent.setup();
    const onUseClick = vi.fn<(model: unknown, route: string) => void>();
    render(<HubCatalogTable {...baseProps({ models: [model()], onUseClick })} />);

    await user.click(screen.getByTestId('hub-model-use-m1'));

    expect(onUseClick).toHaveBeenCalledOnce();
    expect(onUseClick.mock.calls[0]?.[1]).toBe('assistant');
  });

  it('fires the download action', async () => {
    const user = userEvent.setup();
    const onDownloadClick = vi.fn<(model: unknown) => void>();
    render(<HubCatalogTable {...baseProps({ models: [model()], onDownloadClick })} />);

    await user.click(screen.getByTestId('hub-model-download-m1'));

    expect(onDownloadClick).toHaveBeenCalledOnce();
  });
});
