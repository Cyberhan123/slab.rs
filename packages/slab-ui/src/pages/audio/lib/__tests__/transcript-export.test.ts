import { describe, expect, it } from 'vitest';

import {
  getTranscriptSegments,
  getTranscriptText,
  hasTimedSegments,
  toTranscriptSrt,
  toTranscriptTxt,
  toTranscriptVtt,
} from '../transcript-export';

describe('transcript export helpers', () => {
  const task = {
    result_data: {
      segments: [
        { end_ms: 1200, start_ms: 0, text: 'Hello world' },
        { end_ms: 2500, start_ms: 1200, text: 'Second line' },
      ],
      text: 'Hello world\nSecond line',
    },
    segments: null,
    transcript_text: 'legacy transcript',
  } as never;

  it('prefers result data text and segments', () => {
    expect(getTranscriptText(task)).toBe('Hello world\nSecond line');
    expect(getTranscriptSegments(task)).toHaveLength(2);
  });

  it('exports TXT, SRT, and VTT payloads', () => {
    expect(toTranscriptTxt(task)).toBe('Hello world\nSecond line\n');
    expect(toTranscriptSrt(getTranscriptSegments(task))).toContain('1\n00:00:00,000 --> 00:00:01,200\nHello world');
    expect(toTranscriptVtt(getTranscriptSegments(task))).toContain('WEBVTT');
    expect(toTranscriptVtt(getTranscriptSegments(task))).toContain('00:00:00.000 --> 00:00:01.200');
  });

  it('reports timed segments only when text and start_ms are present', () => {
    expect(hasTimedSegments([{ start_ms: 0, end_ms: 100, text: 'hi' }])).toBe(true);
    expect(hasTimedSegments([{ start_ms: 0, end_ms: 100, text: '   ' }])).toBe(false);
    expect(hasTimedSegments([{ start_ms: undefined, end_ms: 100, text: 'hi' }])).toBe(false);
    expect(hasTimedSegments([])).toBe(false);
  });

  it('falls back to segment text and legacy fields when result data is absent', () => {
    const legacyTask = {
      transcript_text: 'legacy transcript',
      segments: [{ start_ms: 0, text: 'seg' }],
    } as never;
    expect(getTranscriptText(legacyTask)).toBe('legacy transcript');
    expect(getTranscriptSegments(legacyTask)).toEqual([{ start_ms: 0, text: 'seg' }]);

    const noTextTask = {
      result_data: { segments: [{ text: 'one' }, { text: '' }, { text: 'two' }] },
    } as never;
    expect(toTranscriptTxt(noTextTask)).toBe('one\ntwo\n');

    // No text and no text-bearing segments collapse to a single newline.
    expect(toTranscriptTxt({ result_data: { segments: [] } } as never)).toBe('\n');
    // Whitespace-only text is treated as empty and falls through to segments.
    expect(toTranscriptTxt({ result_data: { text: '   ', segments: [{ text: 'kept' }] } } as never)).toBe(
      'kept\n',
    );
  });

  it('normalizes missing and out-of-order timestamps when formatting cues', () => {
    const segments = [
      { start_ms: null, end_ms: null, text: 'a' },
      { start_ms: 1000, end_ms: 500, text: 'c' },
      { start_ms: 1234.6, end_ms: 2000, text: 'd' },
    ] as never;

    const srt = toTranscriptSrt(segments);
    // start_ms:null -> 0; end_ms:null -> start+1000.
    expect(srt).toContain('00:00:00,000 --> 00:00:01,000');
    // end < start is clamped up to start+1.
    expect(srt).toContain('00:00:01,000 --> 00:00:01,001');
    // start is rounded (1234.6 -> 1235).
    expect(srt).toContain('00:00:01,235 --> 00:00:02,000');
  });

  it('emits an empty WEBVTT header when no cues carry text', () => {
    expect(toTranscriptVtt([])).toBe('WEBVTT\n\n');
    expect(toTranscriptVtt([{ start_ms: 0, text: '   ' }])).toBe('WEBVTT\n\n');
  });

  it('skips blank-text segments and renumbers the remaining cues', () => {
    const srt = toTranscriptSrt([
      { start_ms: 0, end_ms: 100, text: 'first' },
      { start_ms: 200, end_ms: 300, text: '' },
      { start_ms: 400, end_ms: 500, text: 'third' },
    ] as never);
    expect(srt).toContain('1\n00:00:00,000 --> 00:00:00,100\nfirst');
    expect(srt).toContain('2\n00:00:00,400 --> 00:00:00,500\nthird');
    expect(srt).not.toContain('\n3\n');
  });
});
