import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ChangeEvent, ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { setupSlabI18nMock } from '@slab/test-utils/mocks';

import { ImportPluginPackDialog } from '../import-plugin-pack-dialog';

vi.mock('@slab/i18n', () => setupSlabI18nMock());

vi.mock('@slab/components/dialog', () => ({
  Dialog: ({
    open,
    children,
  }: {
    open: boolean;
    children: ReactNode;
    onOpenChange: (open: boolean) => void;
  }) => (open ? <div>{children}</div> : null),
  DialogContent: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogDescription: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
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

vi.mock('@slab/components/input', () => ({
  Input: ({
    id,
    type,
    onChange,
    disabled,
  }: {
    id?: string;
    type?: string;
    onChange?: (event: ChangeEvent<HTMLInputElement>) => void;
    disabled?: boolean;
  }) => <input aria-label="import-input" id={id} type={type} onChange={onChange} disabled={disabled} />,
}));

vi.mock('@slab/components/label', () => ({
  Label: ({ children, htmlFor }: { children: ReactNode; htmlFor?: string }) => (
    <label htmlFor={htmlFor}>{children}</label>
  ),
}));

vi.mock('@slab/components/progress', () => ({
  Progress: ({ value }: { value?: number }) => <div data-testid="progress" data-value={value} />,
}));

vi.mock('../permission-review-list', () => ({
  PermissionReviewList: () => <div data-testid="permission-review" />,
}));

function baseProps(overrides: Record<string, unknown> = {}) {
  return {
    open: true,
    onOpenChange: vi.fn<(open: boolean) => void>(),
    selectedFileName: null,
    setImportFile: vi.fn<(file: File | null) => void>(),
    canImport: true,
    importPending: false,
    importUploadProgress: null,
    onCancelImport: vi.fn<() => void>(),
    onImport: vi.fn<() => void>(),
    importPreview: null,
    importPreviewFailed: false,
    hasReviewedPermissions: false,
    onReviewedPermissionsChange: vi.fn<(reviewed: boolean) => void>(),
    ...overrides,
  };
}

describe('ImportPluginPackDialog', () => {
  it('renders nothing when closed', () => {
    render(<ImportPluginPackDialog {...baseProps({ open: false })} />);

    expect(screen.queryByTestId('plugin-import-submit-button')).not.toBeInTheDocument();
  });

  it('uploads the selected file', async () => {
    const user = userEvent.setup();
    const setImportFile = vi.fn<(file: File | null) => void>();
    render(<ImportPluginPackDialog {...baseProps({ setImportFile })} />);

    const file = new File(['pack'], 'pack.plugin.slab');
    await user.upload(screen.getByLabelText('pages.plugins.dialogs.import.packLabel'), file);

    expect(setImportFile).toHaveBeenCalledExactlyOnceWith(file);
  });

  it('toggles the reviewed-permissions checkbox', async () => {
    const user = userEvent.setup();
    const onReviewedPermissionsChange = vi.fn<(reviewed: boolean) => void>();
    render(
      <ImportPluginPackDialog
        {...baseProps({
          selectedFileName: 'pack.plugin.slab',
          importPreview: { permissions: { slabApi: [] } } as never,
          onReviewedPermissionsChange,
        })}
      />,
    );

    await user.click(screen.getByTestId('plugin-permissions-reviewed-checkbox'));

    expect(onReviewedPermissionsChange).toHaveBeenCalledExactlyOnceWith(true);
  });

  it('submits and cancels the import', async () => {
    const user = userEvent.setup();
    const onImport = vi.fn<() => void>();
    const onCancelImport = vi.fn<() => void>();
    const { rerender } = render(<ImportPluginPackDialog {...baseProps({ onImport })} />);

    await user.click(screen.getByTestId('plugin-import-submit-button'));
    expect(onImport).toHaveBeenCalledOnce();

    rerender(<ImportPluginPackDialog {...baseProps({ importPending: true, onCancelImport })} />);
    await user.click(screen.getByTestId('plugin-import-cancel-button'));
    expect(onCancelImport).toHaveBeenCalledOnce();
  });
});
