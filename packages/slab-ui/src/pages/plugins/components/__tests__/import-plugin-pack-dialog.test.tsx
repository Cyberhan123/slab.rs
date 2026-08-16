import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-react';
import type { ChangeEvent, ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';


import { ImportPluginPackDialog } from '../import-plugin-pack-dialog';

vi.mock('@slab/i18n', async () => {
  const { setupSlabI18nMock } = await import('@slab/test-utils/mocks')
  return setupSlabI18nMock()
});

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
    ...rest
  }: {
    id?: string;
    type?: string;
    onChange?: (event: ChangeEvent<HTMLInputElement>) => void;
    disabled?: boolean;
  } & Record<string, unknown>) => (
    // Forward all extra props (incl. data-testid / accept) so the real file
    // <input type="file"> stays actionable for Playwright's setInputFiles. Do
    // NOT set aria-label here: it would override the <Label htmlFor> accessible
    // name and break getByLabelText queries that rely on the label association.
    <input id={id} type={type} onChange={onChange} disabled={disabled} {...rest} />
  ),
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
  it('renders nothing when closed', async () => {
    const screen = await render(<ImportPluginPackDialog {...baseProps({ open: false })} />);

    await expect.element(screen.getByTestId('plugin-import-submit-button')).not.toBeInTheDocument();
  });

  it('uploads the selected file', async () => {
    const setImportFile = vi.fn<(file: File | null) => void>();
    const screen = await render(<ImportPluginPackDialog {...baseProps({ setImportFile })} />);

    const file = new File(['pack'], 'pack.plugin.slab');
    await userEvent.upload(screen.getByLabelText('pages.plugins.dialogs.import.packLabel'), file);

    expect(setImportFile).toHaveBeenCalledExactlyOnceWith(file);
  });

  it('toggles the reviewed-permissions checkbox', async () => {
    const onReviewedPermissionsChange = vi.fn<(reviewed: boolean) => void>();
    const screen = await render(
      <ImportPluginPackDialog
        {...baseProps({
          selectedFileName: 'pack.plugin.slab',
          importPreview: { permissions: { slabApi: [] } } as never,
          onReviewedPermissionsChange,
        })}
      />,
    );

    await userEvent.click(screen.getByTestId('plugin-permissions-reviewed-checkbox'));

    expect(onReviewedPermissionsChange).toHaveBeenCalledExactlyOnceWith(true);
  });

  it('submits and cancels the import', async () => {
    const onImport = vi.fn<() => void>();
    const onCancelImport = vi.fn<() => void>();
    const screen = await render(<ImportPluginPackDialog {...baseProps({ onImport })} />);

    await userEvent.click(screen.getByTestId('plugin-import-submit-button'));
    expect(onImport).toHaveBeenCalledOnce();

    await screen.rerender(<ImportPluginPackDialog {...baseProps({ importPending: true, onCancelImport })} />);
    await userEvent.click(screen.getByTestId('plugin-import-cancel-button'));
    expect(onCancelImport).toHaveBeenCalledOnce();
  });
});
