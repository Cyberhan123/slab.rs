import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ChangeEvent, ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { setupSlabI18nMock } from '@slab/test-utils/mocks';

import { VadSettings, type VadSettingsProps } from '../vad-settings';

vi.mock('@slab/i18n', () => setupSlabI18nMock());

vi.mock('@slab/components/label', () => ({
  Label: ({ children, htmlFor }: { children: ReactNode; htmlFor?: string }) => (
    <label htmlFor={htmlFor}>{children}</label>
  ),
}));

vi.mock('@slab/components/input', () => ({
  Input: ({
    id,
    value,
    onChange,
    disabled,
    type,
  }: {
    id?: string;
    value?: string | number;
    onChange?: (event: ChangeEvent<HTMLInputElement>) => void;
    disabled?: boolean;
    type?: string;
  }) => (
    <input id={id} aria-label="vad-field" value={value} onChange={onChange} disabled={disabled} type={type} />
  ),
}));

vi.mock('@slab/components/switch', () => ({
  Switch: ({
    id,
    checked,
    onCheckedChange,
    disabled,
  }: {
    id?: string;
    checked?: boolean;
    onCheckedChange?: (checked: boolean) => void;
    disabled?: boolean;
  }) => (
    <input
      type="checkbox"
      id={id}
      aria-label="vad-switch"
      checked={checked}
      onChange={(event) => onCheckedChange?.(event.target.checked)}
      disabled={disabled}
    />
  ),
}));

vi.mock('@slab/components/select', () => ({
  Select: ({
    value,
    onValueChange,
    disabled,
    children,
  }: {
    value?: string;
    onValueChange?: (value: string) => void;
    disabled?: boolean;
    children: ReactNode;
  }) => (
    <select
      data-testid="vad-model-select"
      aria-label="vad-model"
      value={value}
      disabled={disabled}
      onChange={(event) => onValueChange?.(event.target.value)}
    >
      {children}
    </select>
  ),
  SelectTrigger: ({ children }: { children: ReactNode }) => <>{children}</>,
  SelectContent: ({ children }: { children: ReactNode }) => <>{children}</>,
  SelectItem: ({
    value,
    children,
  }: {
    value: string;
    children: ReactNode;
  }) => (
    <option value={value}>{children}</option>
  ),
  SelectValue: () => null,
}));

function baseProps(overrides: Record<string, unknown> = {}): VadSettingsProps {
  return {
    bundledVadLabel: 'Bundled',
    enableVad: true,
    hasBundledVad: true,
    setEnableVad: vi.fn<(value: boolean) => void>(),
    isTauri: true,
    isBusy: false,
    isUsingBundledVad: false,
    selectedVadModelId: 'bundled',
    setSelectedVadModelId: vi.fn<(value: string) => void>(),
    catalogModelsLoading: false,
    whisperVadModels: [],
    selectedVadModel: undefined,
    vadThreshold: '',
    setVadThreshold: vi.fn<(value: string) => void>(),
    vadMinSpeechDurationMs: '',
    setVadMinSpeechDurationMs: vi.fn<(value: string) => void>(),
    vadMinSilenceDurationMs: '',
    setVadMinSilenceDurationMs: vi.fn<(value: string) => void>(),
    vadMaxSpeechDurationS: '',
    setVadMaxSpeechDurationS: vi.fn<(value: string) => void>(),
    vadSpeechPadMs: '',
    setVadSpeechPadMs: vi.fn<(value: string) => void>(),
    vadSamplesOverlap: '',
    setVadSamplesOverlap: vi.fn<(value: string) => void>(),
    ...overrides,
  } as VadSettingsProps;
}

describe('VadSettings', () => {
  it('hides the numeric fields while VAD is disabled', () => {
    render(<VadSettings {...baseProps({ enableVad: false })} />);

    expect(screen.queryByLabelText('pages.audio.vad.fields.threshold')).not.toBeInTheDocument();
  });

  it('toggles VAD via the header switch', async () => {
    const user = userEvent.setup();
    const setEnableVad = vi.fn<(value: boolean) => void>();
    render(<VadSettings {...baseProps({ enableVad: false, setEnableVad })} />);

    await user.click(screen.getByLabelText('pages.audio.vad.title'));

    expect(setEnableVad).toHaveBeenCalledExactlyOnceWith(true);
  });

  it('forwards a numeric field edit to its setter', async () => {
    const user = userEvent.setup();
    const setVadThreshold = vi.fn<(value: string) => void>();
    render(<VadSettings {...baseProps({ setVadThreshold })} />);

    await user.type(screen.getByLabelText('pages.audio.vad.fields.threshold'), '5');

    expect(setVadThreshold).toHaveBeenCalledWith('5');
  });

  it('renders dedicated VAD models and reports selection', async () => {
    const user = userEvent.setup();
    const setSelectedVadModelId = vi.fn<(value: string) => void>();
    render(
      <VadSettings
        {...baseProps({
          hasBundledVad: false,
          whisperVadModels: [{ id: 'm1', display_name: 'Silero VAD', local_path: '/p' }] as never,
          setSelectedVadModelId,
        })}
      />,
    );

    await user.selectOptions(screen.getByTestId('vad-model-select'), 'm1');

    expect(setSelectedVadModelId).toHaveBeenCalledExactlyOnceWith('m1');
  });
});
