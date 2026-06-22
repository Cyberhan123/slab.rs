import { useMemo, useRef } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { Copy, Download } from 'lucide-react';
import { toast } from 'sonner';

import { Button } from '@slab/components/button';
import { useTranslation } from '@slab/i18n';
import type { AudioTranscriptionTask } from '@/lib/media-task-api';
import {
  getTranscriptSegments,
  getTranscriptText,
  hasTimedSegments,
  toTranscriptSrt,
  toTranscriptTxt,
  toTranscriptVtt,
  type TranscriptSegment,
} from '../lib/transcript-export';

type AudioTranscriptDetailProps = {
  isTauri: boolean;
  task: AudioTranscriptionTask;
};

export function AudioTranscriptDetail({ isTauri, task }: AudioTranscriptDetailProps) {
  const { t } = useTranslation();
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const segments = getTranscriptSegments(task);
  const transcriptText = getTranscriptText(task);
  const timedSegments = hasTimedSegments(segments);
  const sourceUrl = useMemo(
    () => resolveAudioSource(task.source_path, isTauri),
    [isTauri, task.source_path],
  );

  const copyTranscript = async () => {
    await navigator.clipboard.writeText(toTranscriptTxt(task));
    toast.success(t('pages.audio.history.actions.copied'));
  };

  const exportFile = (kind: 'txt' | 'srt' | 'vtt') => {
    const baseName = task.task_id.replace(/[^a-z0-9_-]/gi, '').slice(0, 24) || 'transcript';
    const content =
      kind === 'txt'
        ? toTranscriptTxt(task)
        : kind === 'srt'
          ? toTranscriptSrt(segments)
          : toTranscriptVtt(segments);

    downloadText(`${baseName}.${kind}`, content, kind === 'txt' ? 'text/plain' : 'text/vtt');
  };

  const seekToSegment = (segment: TranscriptSegment) => {
    const player = audioRef.current;
    if (!player || typeof segment.start_ms !== 'number') {
      return;
    }

    player.currentTime = Math.max(segment.start_ms / 1000, 0);
    void player.play();
  };

  return (
    <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_280px]">
      <div className="space-y-4">
        {sourceUrl ? (
          // eslint-disable-next-line jsx-a11y/media-has-caption
          <audio
            ref={audioRef}
            src={sourceUrl}
            controls
            aria-label={t('pages.audio.history.actions.playbackAria')}
            className="w-full rounded-[18px] border border-border/60 bg-[var(--surface-soft)] px-3 py-2"
          />
        ) : null}

        <div className="max-h-[68vh] overflow-y-auto rounded-[22px] bg-[var(--surface-soft)] p-4">
          {segments.length > 0 ? (
            <div className="space-y-2">
              {segments.map((segment) => (
                <div
                  key={getSegmentKey(segment)}
                  className="grid gap-3 rounded-[16px] border border-border/50 bg-[var(--shell-card)] px-3 py-3 sm:grid-cols-[112px_minmax(0,1fr)]"
                >
                  <button
                    type="button"
                    className="text-left font-mono text-xs font-semibold text-[var(--brand-teal)] transition hover:text-foreground"
                    onClick={() => seekToSegment(segment)}
                    aria-label={t('pages.audio.history.actions.seekAria', {
                      time: formatSegmentRange(segment),
                    })}
                  >
                    {formatSegmentRange(segment)}
                  </button>
                  <p className="min-w-0 whitespace-pre-wrap break-words text-sm leading-6 text-foreground">
                    {segment.text?.trim() || t('pages.audio.history.emptySegment')}
                  </p>
                </div>
              ))}
            </div>
          ) : (
            <pre className="whitespace-pre-wrap break-words text-sm leading-6 text-foreground">
              {transcriptText || t('pages.audio.history.pendingTranscript')}
            </pre>
          )}
        </div>
      </div>

      <div className="space-y-4 rounded-[22px] bg-[var(--surface-soft)] p-4">
        <div className="flex flex-wrap gap-2">
          <Button variant="secondary" size="sm" onClick={() => void copyTranscript()}>
            <Copy className="size-3.5" />
            {t('pages.audio.history.actions.copy')}
          </Button>
          <Button variant="secondary" size="sm" onClick={() => exportFile('txt')}>
            <Download className="size-3.5" />
            {t('pages.audio.history.actions.exportTxt')}
          </Button>
          {timedSegments ? (
            <>
              <Button variant="secondary" size="sm" onClick={() => exportFile('srt')}>
                <Download className="size-3.5" />
                {t('pages.audio.history.actions.exportSrt')}
              </Button>
              <Button variant="secondary" size="sm" onClick={() => exportFile('vtt')}>
                <Download className="size-3.5" />
                {t('pages.audio.history.actions.exportVtt')}
              </Button>
            </>
          ) : null}
        </div>

        <div>
          <p className="text-caption font-bold uppercase tracking-eyebrow text-muted-foreground">
            {t('pages.audio.history.fields.source')}
          </p>
          <p className="mt-2 break-all text-sm text-foreground">{task.source_path}</p>
        </div>
        <div className="grid grid-cols-1 gap-3 text-sm">
          <div>
            <p className="text-xs text-muted-foreground">{t('pages.audio.history.fields.model')}</p>
            <p className="font-semibold">{task.model_id ?? task.backend_id}</p>
          </div>
          <div>
            <p className="text-xs text-muted-foreground">{t('pages.audio.history.fields.language')}</p>
            <p className="font-semibold">{task.language ?? '-'}</p>
          </div>
          <div>
            <p className="text-xs text-muted-foreground">{t('pages.audio.history.fields.segments')}</p>
            <p className="font-semibold">{segments.length}</p>
          </div>
        </div>
        {task.error_msg ? (
          <p className="rounded-xl bg-destructive/10 p-3 text-xs leading-5 text-destructive">
            {task.error_msg}
          </p>
        ) : null}
      </div>
    </div>
  );
}

function resolveAudioSource(path: string, isTauri: boolean) {
  if (!path.trim()) {
    return null;
  }

  if (/^(?:https?:|blob:|data:)/i.test(path)) {
    return path;
  }

  return isTauri ? convertFileSrc(path) : null;
}

function downloadText(filename: string, content: string, type: string) {
  const blob = new Blob([content], { type: `${type};charset=utf-8` });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(url);
}

function formatSegmentRange(segment: TranscriptSegment) {
  const start = typeof segment.start_ms === 'number' ? segment.start_ms : 0;
  const end = typeof segment.end_ms === 'number' ? segment.end_ms : start;
  return `${formatSegmentTime(start)} - ${formatSegmentTime(end)}`;
}

function formatSegmentTime(value: number) {
  const totalSeconds = Math.floor(value / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
}

function getSegmentKey(segment: TranscriptSegment) {
  const start = typeof segment.start_ms === 'number' ? segment.start_ms : 'unknown-start';
  const end = typeof segment.end_ms === 'number' ? segment.end_ms : 'unknown-end';
  const text = segment.text?.trim();
  return `${start}-${end}-${text ?? 'empty'}`;
}
