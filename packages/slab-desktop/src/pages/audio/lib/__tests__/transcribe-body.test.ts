import { describe, expect, it } from 'vitest';

import type { TranscribeOptions } from '../../hooks/use-transcribe';
import { buildTranscriptionBody } from '../transcribe-body';

const t = (key: string) => key;

describe('buildTranscriptionBody', () => {
  it('throws when running outside the desktop host', () => {
    expect(() => buildTranscriptionBody('/path', undefined, false, t)).toThrow(
      'pages.audio.error.webUploadNotImplemented',
    );
  });

  it('throws for non-string or blank file paths', () => {
    const file = new File(['x'], 'a.mp3');
    expect(() => buildTranscriptionBody(file, undefined, true, t)).toThrow(
      'pages.audio.error.invalidDesktopFilePath',
    );
    expect(() => buildTranscriptionBody('   ', undefined, true, t)).toThrow(
      'pages.audio.error.invalidDesktopFilePath',
    );
  });

  it('builds a path-only body when no options are supplied', () => {
    expect(buildTranscriptionBody('/songs/a.mp3', undefined, true, t)).toEqual({
      path: '/songs/a.mp3',
    });
  });

  it('trims and grafts optional fields onto the body', () => {
    const options: TranscribeOptions = {
      model_id: '  whisper-1  ',
      language: '  en  ',
      prompt: '  hello  ',
      detect_language: true,
    };

    expect(buildTranscriptionBody('/p', options, true, t)).toEqual({
      path: '/p',
      model_id: 'whisper-1',
      language: 'en',
      prompt: 'hello',
      detect_language: true,
    });
  });

  it('omits blank optional string fields', () => {
    expect(
      buildTranscriptionBody('/p', { model_id: '   ', language: '', prompt: '  ' }, true, t),
    ).toEqual({ path: '/p' });
  });

  it('passes vad and decode settings through by reference', () => {
    const vad: TranscribeOptions['vad'] = { enabled: true, model_path: '/vad.bin' };
    const decode: TranscribeOptions['decode'] = { offset_ms: 10 };
    const body = buildTranscriptionBody('/p', { vad, decode }, true, t) as {
      path: string;
      vad?: unknown;
      decode?: unknown;
    };

    expect(body.vad).toBe(vad);
    expect(body.decode).toBe(decode);
  });
});
