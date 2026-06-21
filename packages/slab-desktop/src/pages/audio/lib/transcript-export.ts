import type { components } from '@slab/api/v1';
import type { AudioTranscriptionTask } from '@/lib/media-task-api';

export type TranscriptSegment = components['schemas']['TimedTextSegmentResponse'];

export function getTranscriptSegments(task: AudioTranscriptionTask): TranscriptSegment[] {
  return task.result_data?.segments ?? task.segments ?? [];
}

export function getTranscriptText(task: AudioTranscriptionTask): string {
  return task.result_data?.text ?? task.transcript_text ?? '';
}

export function toTranscriptTxt(task: AudioTranscriptionTask): string {
  const text = getTranscriptText(task).trim();
  if (text) {
    return `${text}\n`;
  }

  return getTranscriptSegments(task)
    .map((segment) => segment.text?.trim())
    .filter((segmentText): segmentText is string => Boolean(segmentText))
    .join('\n')
    .concat('\n');
}

export function toTranscriptSrt(segments: TranscriptSegment[]): string {
  return segments
    .filter(hasSegmentText)
    .map((segment, index) => {
      const start = normalizeTimestamp(segment.start_ms);
      const end = normalizeTimestamp(segment.end_ms, start + 1000);
      return [
        String(index + 1),
        `${formatSrtTimestamp(start)} --> ${formatSrtTimestamp(Math.max(end, start + 1))}`,
        segment.text?.trim() ?? '',
      ].join('\n');
    })
    .join('\n\n')
    .concat('\n');
}

export function toTranscriptVtt(segments: TranscriptSegment[]): string {
  const cues = segments
    .filter(hasSegmentText)
    .map((segment) => {
      const start = normalizeTimestamp(segment.start_ms);
      const end = normalizeTimestamp(segment.end_ms, start + 1000);
      return [
        `${formatVttTimestamp(start)} --> ${formatVttTimestamp(Math.max(end, start + 1))}`,
        segment.text?.trim() ?? '',
      ].join('\n');
    })
    .join('\n\n');

  return `WEBVTT\n\n${cues}${cues ? '\n' : ''}`;
}

export function hasTimedSegments(segments: TranscriptSegment[]): boolean {
  return segments.some((segment) => hasSegmentText(segment) && segment.start_ms !== undefined);
}

function hasSegmentText(segment: TranscriptSegment) {
  return Boolean(segment.text?.trim());
}

function normalizeTimestamp(value: number | null | undefined, fallback = 0): number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0
    ? Math.round(value)
    : fallback;
}

function formatSrtTimestamp(value: number) {
  return formatTimestamp(value, ',');
}

function formatVttTimestamp(value: number) {
  return formatTimestamp(value, '.');
}

function formatTimestamp(value: number, millisecondSeparator: ',' | '.') {
  const milliseconds = value % 1000;
  const totalSeconds = Math.floor(value / 1000);
  const seconds = totalSeconds % 60;
  const totalMinutes = Math.floor(totalSeconds / 60);
  const minutes = totalMinutes % 60;
  const hours = Math.floor(totalMinutes / 60);

  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}${millisecondSeparator}${padMs(milliseconds)}`;
}

function pad(value: number) {
  return String(value).padStart(2, '0');
}

function padMs(value: number) {
  return String(value).padStart(3, '0');
}
