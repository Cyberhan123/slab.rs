import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ChangeEvent, ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { setupSlabI18nMock } from '@slab/test-utils/mocks';

import { DecodeOptions, type DecodeOptionsProps } from '../decode-options';

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
    placeholder,
  }: {
    id?: string;
    value?: string | number;
    onChange?: (event: ChangeEvent<HTMLInputElement>) => void;
    disabled?: boolean;
    type?: string;
    placeholder?: string;
  }) => (
    <input
      id={id}
      aria-label="decode-field"
      value={value}
      onChange={onChange}
      disabled={disabled}
      type={type}
      placeholder={placeholder}
    />
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
      aria-label="decode-switch"
      checked={checked}
      onChange={(event) => onCheckedChange?.(event.target.checked)}
      disabled={disabled}
    />
  ),
}));

function baseProps(overrides: Record<string, unknown> = {}): DecodeOptionsProps {
  return {
    showDecodeOptions: true,
    setShowDecodeOptions: vi.fn<(value: boolean) => void>(),
    isTauri: true,
    isBusy: false,
    decodeOffsetMs: '',
    setDecodeOffsetMs: vi.fn<(value: string) => void>(),
    decodeDurationMs: '',
    setDecodeDurationMs: vi.fn<(value: string) => void>(),
    decodeWordThold: '',
    setDecodeWordThold: vi.fn<(value: string) => void>(),
    decodeMaxLen: '',
    setDecodeMaxLen: vi.fn<(value: string) => void>(),
    decodeMaxTokens: '',
    setDecodeMaxTokens: vi.fn<(value: string) => void>(),
    decodeTemperature: '',
    setDecodeTemperature: vi.fn<(value: string) => void>(),
    decodeTemperatureInc: '',
    setDecodeTemperatureInc: vi.fn<(value: string) => void>(),
    decodeEntropyThold: '',
    setDecodeEntropyThold: vi.fn<(value: string) => void>(),
    decodeLogprobThold: '',
    setDecodeLogprobThold: vi.fn<(value: string) => void>(),
    decodeNoSpeechThold: '',
    setDecodeNoSpeechThold: vi.fn<(value: string) => void>(),
    decodeNoContext: false,
    setDecodeNoContext: vi.fn<(value: boolean) => void>(),
    decodeNoTimestamps: false,
    setDecodeNoTimestamps: vi.fn<(value: boolean) => void>(),
    decodeTokenTimestamps: false,
    setDecodeTokenTimestamps: vi.fn<(value: boolean) => void>(),
    decodeSplitOnWord: false,
    setDecodeSplitOnWord: vi.fn<(value: boolean) => void>(),
    decodeSuppressNst: false,
    setDecodeSuppressNst: vi.fn<(value: boolean) => void>(),
    decodeTdrzEnable: false,
    setDecodeTdrzEnable: vi.fn<(value: boolean) => void>(),
    ...overrides,
  } as DecodeOptionsProps;
}

describe('DecodeOptions', () => {
  it('hides the numeric fields when collapsed', () => {
    render(<DecodeOptions {...baseProps({ showDecodeOptions: false })} />);

    expect(screen.queryAllByRole('spinbutton')).toHaveLength(0);
  });

  it('exposes the numeric fields when expanded', () => {
    render(<DecodeOptions {...baseProps()} />);

    expect(screen.getAllByRole('spinbutton').length).toBeGreaterThan(0);
  });

  it('toggles expansion via the header switch', async () => {
    const user = userEvent.setup();
    const setShowDecodeOptions = vi.fn<(value: boolean) => void>();
    render(<DecodeOptions {...baseProps({ showDecodeOptions: false, setShowDecodeOptions })} />);

    await user.click(screen.getByLabelText('pages.audio.decode.title'));

    expect(setShowDecodeOptions).toHaveBeenCalledExactlyOnceWith(true);
  });

  it('forwards a numeric field edit to its setter', async () => {
    const user = userEvent.setup();
    const setDecodeOffsetMs = vi.fn<(value: string) => void>();
    render(<DecodeOptions {...baseProps({ setDecodeOffsetMs })} />);

    await user.type(screen.getByLabelText('pages.audio.decode.fields.offset'), '5');

    expect(setDecodeOffsetMs).toHaveBeenCalledWith('5');
  });

  it('forwards a toggle field edit to its setter', async () => {
    const user = userEvent.setup();
    const setDecodeNoContext = vi.fn<(value: boolean) => void>();
    render(<DecodeOptions {...baseProps({ setDecodeNoContext })} />);

    await user.click(screen.getByLabelText('pages.audio.decode.fields.noContext'));

    expect(setDecodeNoContext).toHaveBeenCalledExactlyOnceWith(true);
  });

  it('disables the numeric inputs while busy', () => {
    render(<DecodeOptions {...baseProps({ isBusy: true })} />);

    expect(screen.getByLabelText('pages.audio.decode.fields.offset')).toBeDisabled();
  });
});
