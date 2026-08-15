import { describe, expect, it } from 'vitest'

import {
  HISTORY_MARKER_ID,
  buildScrollerRows,
  formatMarkerDate,
  type ScrollerRow,
} from '../build-scroller-rows'
import type { CompactionMarker, ModelLoadState } from '@slab/ui/pages/assistant/hooks/use-harness-conversation'

const msg = (id: string) => ({ id, role: 'assistant' })

const compact = (id: string): CompactionMarker => ({
  id,
  mode: 'auto',
  phase: 'compacted',
  threadId: 't1',
})

const compactRowIds = (rows: ScrollerRow[]) =>
  rows
    .filter(
      (r): r is Extract<ScrollerRow, { kind: 'compactMarker' }> => r.kind === 'compactMarker',
    )
    .map((r) => r.id)

describe('buildScrollerRows', () => {
  it('omits the history marker when showHistoryMarker is false', () => {
    const rows = buildScrollerRows([msg('m1'), msg('m2')], [], { showHistoryMarker: false })

    expect(rows.some((r) => r.kind === 'historyMarker')).toBe(false)
    expect(rows).toHaveLength(2)
  })

  it('never inserts the marker for an empty session even when requested', () => {
    const rows = buildScrollerRows([], [], { showHistoryMarker: true })

    expect(rows.some((r) => r.kind === 'historyMarker')).toBe(false)
    expect(rows).toHaveLength(0)
  })

  it('places the marker at the end when historyCount is omitted', () => {
    const rows = buildScrollerRows([msg('m1'), msg('m2')], [], { showHistoryMarker: true })

    expect(rows.at(-1)?.kind).toBe('historyMarker')
  })

  it('places the marker between restored and live messages by historyCount', () => {
    const rows = buildScrollerRows([msg('m1'), msg('m2')], [], {
      showHistoryMarker: true,
      historyCount: 1,
    })
    const idx = rows.findIndex((r) => r.kind === 'historyMarker')

    expect(idx).toBe(1)
    expect(rows[idx - 1]).toMatchObject({ kind: 'message', id: 'm1' })
    expect(rows[idx + 1]).toMatchObject({ kind: 'message', id: 'm2' })
  })

  it('uses the stable history marker id', () => {
    const rows = buildScrollerRows([msg('m1')], [], { showHistoryMarker: true })
    const marker = rows.find((r) => r.kind === 'historyMarker')

    expect(marker).toMatchObject({ kind: 'historyMarker', id: HISTORY_MARKER_ID })
  })

  it('appends compaction markers after the messages, in order', () => {
    const rows = buildScrollerRows([msg('m1')], [compact('auto:t1:1'), compact('manual:t1:2')], {
      showHistoryMarker: false,
    })

    expect(compactRowIds(rows)).toEqual(['auto:t1:1', 'manual:t1:2'])
    expect(rows.at(-1)?.kind).toBe('compactMarker')
  })

  it('leads with a session-load marker only while restoring with no messages', () => {
    const loading = buildScrollerRows([], [], { showHistoryMarker: false, sessionLoading: true })
    expect(loading).toHaveLength(1)
    expect(loading[0]).toMatchObject({ kind: 'sessionLoadMarker' })

    // Once there are messages, the session-load marker is suppressed even if still loading.
    const withMessages = buildScrollerRows([msg('m1')], [], {
      showHistoryMarker: false,
      sessionLoading: true,
    })
    expect(withMessages.some((r) => r.kind === 'sessionLoadMarker')).toBe(false)
  })

  it('places the model-load marker after compaction markers at the live edge', () => {
    const load: ModelLoadState = { phase: 'loading', modelId: 'm' }
    const rows = buildScrollerRows([msg('m1')], [compact('c1')], {
      showHistoryMarker: false,
      modelLoad: load,
    })

    expect(rows.map((r) => r.kind)).toEqual(['message', 'compactMarker', 'modelLoadMarker'])
  })

  it('orders the full status timeline: history → compaction → model-load', () => {
    const load: ModelLoadState = { phase: 'downloading', downloadedBytes: 1, totalBytes: 2 }
    const rows = buildScrollerRows([msg('m1'), msg('m2')], [compact('c1')], {
      showHistoryMarker: true,
      historyCount: 1,
      modelLoad: load,
    })

    expect(rows.map((r) => r.kind)).toEqual([
      'message',
      'historyMarker',
      'message',
      'compactMarker',
      'modelLoadMarker',
    ])
  })
})

describe('formatMarkerDate', () => {
  it('formats a date as zero-padded YYYY-MM-DD', () => {
    expect(formatMarkerDate(new Date(2026, 0, 5))).toBe('2026-01-05')
    expect(formatMarkerDate(new Date(2026, 10, 23))).toBe('2026-11-23')
  })
})
