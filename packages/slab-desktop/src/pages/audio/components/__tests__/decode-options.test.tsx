import { userEvent } from 'vitest/browser';
import type { ChangeEvent, ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-react';

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
    // oxlint-disable-next-line jsx-a11y/control-has-associated-label -- labeled via the component's sibling <Label htmlFor>
    <input
      id={id}
      title="decode-field"
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
    // oxlint-disable-next-line jsx-a11y/control-has-associated-label -- labeled via the component's sibling <Label htmlFor>
    <input
      type="checkbox"
      id={id}
      title="decode-switch"
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
  it('hides the numeric fields when collapsed', async () => {
    const screen = await render(<DecodeOptions {...baseProps({ showDecodeOptions: false })} />);

    expect(screen.getByRole('spinbutton').elements()).toHaveLength(0);
  });

  it('exposes the numeric fields when expanded', async () => {
    const screen = await render(<DecodeOptions {...baseProps()} />);

    expect(screen.getByRole('spinbutton').length).toBeGreaterThan(0);
  });

  it('toggles expansion via the header switch', async () => {
    const setShowDecodeOptions = vi.fn<(value: boolean) => void>();
    const screen = await render(<DecodeOptions {...baseProps({ showDecodeOptions: false, setShowDecodeOptions })} />);

    await userEvent.click(screen.getByLabelText('pages.audio.decode.title'));

    expect(setShowDecodeOptions).toHaveBeenCalledExactlyOnceWith(true);
  });

  it('forwards a numeric field edit to its setter', async () => {
    const setDecodeOffsetMs = vi.fn<(value: string) => void>();
    const screen = await render(<DecodeOptions {...baseProps({ setDecodeOffsetMs })} />);

    await userEvent.type(screen.getByLabelText('pages.audio.decode.fields.offset'), '5');

    expect(setDecodeOffsetMs).toHaveBeenCalledWith('5');
  });

  it('forwards a toggle field edit to its setter', async () => {
    const setDecodeNoContext = vi.fn<(value: boolean) => void>();
    const screen = await render(<DecodeOptions {...baseProps({ setDecodeNoContext })} />);

    await userEvent.click(screen.getByLabelText('pages.audio.decode.fields.noContext'));

    expect(setDecodeNoContext).toHaveBeenCalledExactlyOnceWith(true);
  });

  it('disables the numeric inputs while busy', async () => {
    const screen = await render(<DecodeOptions {...baseProps({ isBusy: true })} />);

    await expect.element(screen.getByLabelText('pages.audio.decode.fields.offset')).toBeDisabled();
  });
});
