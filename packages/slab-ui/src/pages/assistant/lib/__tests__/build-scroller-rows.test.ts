import { describe, expect, it } from 'vitest'

import {
  HISTORY_MARKER_ID,
  buildScrollerRows,
  formatMarkerDate,
  type ScrollerRow,
} from '../build-scroller-rows'
import type { CompactionMarker, ModelLoadState } from '@slab/core/harness'

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

  it('appends unanchored settings markers after compaction markers, in arrival order', () => {
    const rows = buildScrollerRows([msg('m1')], [compact('c1')], {
      showHistoryMarker: false,
      settingsMarkers: [
        {
          id: 'permissionMode:1',
          kind: 'permissionMode',
          fromMode: 'approve_for_me',
          toMode: 'request_approval',
          afterMessageId: null,
        },
        {
          id: 'modelSwitch:2',
          kind: 'modelSwitch',
          fromModel: 'A',
          toModel: 'B',
          afterMessageId: null,
        },
      ],
    })

    expect(rows.map((r) => r.kind)).toEqual([
      'message',
      'compactMarker',
      'settingsMarker',
      'settingsMarker',
    ])
    const settings = rows.filter(
      (r): r is Extract<ScrollerRow, { kind: 'settingsMarker' }> => r.kind === 'settingsMarker',
    )
    expect(settings.map((r) => r.id)).toEqual(['permissionMode:1', 'modelSwitch:2'])
  })

  it('inserts anchored settings markers right after their anchor message', () => {
    const rows = buildScrollerRows([msg('m1'), msg('m2'), msg('m3')], [], {
      showHistoryMarker: false,
      settingsMarkers: [
        {
          id: 'modelSwitch:1',
          kind: 'modelSwitch',
          fromModel: 'A',
          toModel: 'B',
          afterMessageId: 'm1',
        },
      ],
    })

    expect(rows.map((r) => r.kind)).toEqual([
      'message',
      'settingsMarker',
      'message',
      'message',
    ])
    expect(rows[1]).toMatchObject({ kind: 'settingsMarker', id: 'modelSwitch:1' })
  })

  it('keeps push order for markers sharing one anchor and anchors to the last match', () => {
    const rows = buildScrollerRows([msg('m1'), msg('m1'), msg('m2')], [], {
      showHistoryMarker: false,
      settingsMarkers: [
        {
          id: 'permissionMode:1',
          kind: 'permissionMode',
          fromMode: 'request_approval',
          toMode: 'full_control',
          afterMessageId: 'm1',
        },
        {
          id: 'modelSwitch:2',
          kind: 'modelSwitch',
          fromModel: 'A',
          toModel: 'B',
          afterMessageId: 'm1',
        },
      ],
    })

    // Both land after the LAST m1, in arrival order (closest to the message
    // first — the backward scan skips the already-inserted marker row).
    expect(rows.map((r) => r.kind)).toEqual([
      'message',
      'message',
      'settingsMarker',
      'settingsMarker',
      'message',
    ])
    expect(rows.slice(2, 4).map((r) => (r as { id: string }).id)).toEqual([
      'permissionMode:1',
      'modelSwitch:2',
    ])
  })

  it('falls back to the tail when the anchor message no longer exists', () => {
    const rows = buildScrollerRows([msg('m1'), msg('m2')], [], {
      showHistoryMarker: false,
      settingsMarkers: [
        {
          id: 'modelSwitch:1',
          kind: 'modelSwitch',
          fromModel: 'A',
          toModel: 'B',
          afterMessageId: 'deleted-message',
        },
      ],
    })

    expect(rows.map((r) => r.kind)).toEqual(['message', 'message', 'settingsMarker'])
  })

  it('places unanchored settings markers between compaction markers and the model-load marker', () => {
    const load: ModelLoadState = { phase: 'loading', modelId: 'm' }
    const rows = buildScrollerRows([msg('m1')], [compact('c1')], {
      showHistoryMarker: false,
      modelLoad: load,
      settingsMarkers: [
        {
          id: 'modelSwitch:1',
          kind: 'modelSwitch',
          fromModel: 'A',
          toModel: 'B',
          afterMessageId: null,
        },
      ],
    })

    expect(rows.map((r) => r.kind)).toEqual([
      'message',
      'compactMarker',
      'settingsMarker',
      'modelLoadMarker',
    ])
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

  it('appends queued steering inputs as ghost rows at the absolute tail, in order', () => {
    const load: ModelLoadState = { phase: 'loading', modelId: 'm' }
    const rows = buildScrollerRows([msg('m1')], [compact('c1')], {
      showHistoryMarker: false,
      modelLoad: load,
      queuedTexts: ['first steer', 'second steer'],
    })

    expect(rows.map((r) => r.kind)).toEqual([
      'message',
      'compactMarker',
      'modelLoadMarker',
      'queuedInput',
      'queuedInput',
    ])
    const queued = rows.filter(
      (r): r is Extract<ScrollerRow, { kind: 'queuedInput' }> => r.kind === 'queuedInput',
    )
    expect(queued.map((r) => r.text)).toEqual(['first steer', 'second steer'])
    expect(queued.map((r) => r.id)).toEqual(['__queued_input_0', '__queued_input_1'])
  })

  it('omits queued rows when nothing is queued', () => {
    const rows = buildScrollerRows([msg('m1')], [], { showHistoryMarker: false })

    expect(rows.some((r) => r.kind === 'queuedInput')).toBe(false)
  })

  // Shell background tasks surface in the list WHILE RUNNING (the Running
  // backgroundTask/updated event arrives at register time — pin the tail
  // marker row so the live visibility contract cannot silently regress).
  it('renders a running shell background task as a tail marker row', () => {
    const rows = buildScrollerRows([msg('m1')], [], {
      showHistoryMarker: false,
      backgroundTasks: [
        { taskId: 'sh-1', status: 'running', pid: 4242, command: 'sleep 30' },
      ],
    })

    expect(rows.map((r) => r.kind)).toEqual(['message', 'backgroundTask'])
    expect(rows.at(-1)).toMatchObject({
      kind: 'backgroundTask',
      id: '__background_task_sh-1',
      task: { taskId: 'sh-1', status: 'running', command: 'sleep 30', pid: 4242 },
    })
  })

  it('drops the row once the task leaves the running state', () => {
    for (const status of ['exited', 'stopped', 'failed'] as const) {
      const rows = buildScrollerRows([msg('m1')], [], {
        showHistoryMarker: false,
        backgroundTasks: [{ taskId: 'sh-1', status, exitCode: 0, command: 'sleep 30' }],
      })
      expect(rows.some((r) => r.kind === 'backgroundTask')).toBe(false)
    }
  })
})

describe('formatMarkerDate', () => {
  it('formats a date as zero-padded YYYY-MM-DD', () => {
    expect(formatMarkerDate(new Date(2026, 0, 5))).toBe('2026-01-05')
    expect(formatMarkerDate(new Date(2026, 10, 23))).toBe('2026-11-23')
  })
})
