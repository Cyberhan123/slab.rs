import { describe, expect, it } from 'vitest';

import {
  getTranscriptSegments,
  getTranscriptText,
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
});
