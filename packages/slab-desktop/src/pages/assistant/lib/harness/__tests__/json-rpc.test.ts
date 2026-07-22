import { describe, expect, it } from "vitest"

import { classifyJsonRpcMessage, nextRequestId } from "../json-rpc"

describe("harness nextRequestId", () => {
  it("mints monotonic integer ids starting at 1", () => {
    expect(nextRequestId()).toBe(1)
    expect(nextRequestId()).toBe(2)
    expect(nextRequestId()).toBe(3)
  })
})

describe("harness classifyJsonRpcMessage", () => {
  it("classifies a frame with method and id as a request", () => {
    expect(classifyJsonRpcMessage({ jsonrpc: "2.0", id: 1, method: "turn/start" })).toEqual({
      kind: "request",
      message: { jsonrpc: "2.0", id: 1, method: "turn/start" },
    })
  })

  it("classifies a frame with method but no id as a notification", () => {
    expect(classifyJsonRpcMessage({ jsonrpc: "2.0", method: "item/started", params: {} })).toEqual({
      kind: "notification",
      message: { jsonrpc: "2.0", method: "item/started", params: {} },
    })
  })

  it("classifies an error response when both error and id are present", () => {
    const message = { jsonrpc: "2.0", id: 7, error: { code: -32000, message: "boom" } }
    expect(classifyJsonRpcMessage(message)).toEqual({ kind: "error", message })
  })

  it("classifies a success response when both result and id are present", () => {
    const message = { jsonrpc: "2.0", id: 7, result: { ok: true } }
    expect(classifyJsonRpcMessage(message)).toEqual({ kind: "response", message })
  })

  it("prefers error over result when both are present", () => {
    // Regression: a failed request may carry a stale `result` from a prior
    // turn. The classifier must treat the frame as an error so failures are
    // not masked as success. `error` is checked before `result`.
    const message = {
      jsonrpc: "2.0",
      id: 7,
      error: { code: -32000, message: "boom" },
      result: { ok: true },
    }
    expect(classifyJsonRpcMessage(message)).toEqual({ kind: "error", message })
  })

  it("marks non-envelopes and non-2.0 frames as invalid", () => {
    expect(classifyJsonRpcMessage(null)).toEqual({ kind: "invalid" })
    expect(classifyJsonRpcMessage("not an object")).toEqual({ kind: "invalid" })
    expect(classifyJsonRpcMessage({})).toEqual({ kind: "invalid" })
    expect(classifyJsonRpcMessage({ jsonrpc: "1.0", id: 1, method: "x" })).toEqual({
      kind: "invalid",
    })
    expect(classifyJsonRpcMessage({ jsonrpc: "2.0" })).toEqual({ kind: "invalid" })
  })

  it("marks result/error frames that lack an id as invalid", () => {
    // A result/error frame is only meaningful as a reply to a known request;
    // without an id it cannot be routed, so it is invalid rather than a
    // notification (notifications require a `method`).
    expect(classifyJsonRpcMessage({ jsonrpc: "2.0", result: {} })).toEqual({ kind: "invalid" })
    expect(classifyJsonRpcMessage({ jsonrpc: "2.0", error: { code: -1, message: "x" } })).toEqual({
      kind: "invalid",
    })
  })

  it("treats an explicit zero or null id as a present id (request, not notification)", () => {
    // Only an absent id (`=== undefined`) makes a method frame a notification;
    // `id: 0` and `id: null` are still present and route a request.
    expect(classifyJsonRpcMessage({ jsonrpc: "2.0", id: 0, method: "x" })).toEqual({
      kind: "request",
      message: { jsonrpc: "2.0", id: 0, method: "x" },
    })
    expect(classifyJsonRpcMessage({ jsonrpc: "2.0", id: null, method: "x" })).toEqual({
      kind: "request",
      message: { jsonrpc: "2.0", id: null, method: "x" },
    })
  })
})
