import { describe, expect, it } from "vitest"

import { groupTraceRuns, type TraceRun } from "../trace-runs"
import type { MessagePartItem } from "../../components/message/message-parts"

type Part = { type: string; text?: string }

function item(
  key: string,
  kind: string,
  toolCallId?: string
): MessagePartItem<Part, unknown> {
  return { key, part: { type: kind }, message: {}, index: 0, kind, toolCallId }
}

describe("groupTraceRuns", () => {
  it("groups consecutive tool and reasoning parts into one trace run", () => {
    const runs = groupTraceRuns([
      item("r1", "reasoning"),
      item("t1", "tool", "call-1"),
      item("t2", "tool", "call-2"),
    ])

    expect(runs).toEqual<TraceRun[]>([
      { trace: true, key: "r1", items: expect.any(Array) },
    ])
    expect(runs[0].items.map((i) => i.key)).toEqual(["r1", "t1", "t2"])
  })

  it("starts a new run at every text part", () => {
    const runs = groupTraceRuns([
      item("t1", "tool", "call-1"),
      item("x1", "text"),
      item("t2", "tool", "call-2"),
    ])

    expect(runs.map((r) => [r.trace, r.key])).toEqual([
      [true, "t1"],
      [false, "x1"],
      [true, "t2"],
    ])
  })

  it("ends the run after an approval-pending tool row", () => {
    const runs = groupTraceRuns(
      [item("t1", "tool", "call-1"), item("r1", "reasoning"), item("t2", "tool", "call-2")],
      (item) => item.toolCallId === "call-1"
    )

    // The approval-pending row renders standalone (the prompt lives in the
    // out-of-band ApprovalCard), and the next trace item starts a fresh run.
    expect(runs.map((r) => [r.trace, r.key])).toEqual([
      [false, "t1"],
      [true, "r1"],
    ])
    expect(runs[0].items.map((i) => i.key)).toEqual(["t1"])
    expect(runs[1].items.map((i) => i.key)).toEqual(["r1", "t2"])
  })

  it("keeps non-trace kinds outside any run", () => {
    const runs = groupTraceRuns([
      item("x1", "text"),
      item("f1", "file"),
      item("x2", "text"),
    ])

    expect(runs.every((r) => !r.trace)).toBe(true)
  })

  it("returns an empty list for an empty message", () => {
    expect(groupTraceRuns([])).toEqual([])
  })
})
