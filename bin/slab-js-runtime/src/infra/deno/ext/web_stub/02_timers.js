// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
import { op_defer, op_now } from "ext:core/ops";
const {
  Uint8Array,
  Uint32Array,
  PromisePrototypeThen,
  TypedArrayPrototypeGetBuffer,
  TypeError,
  indirectEval,
  ReflectApply,
} = primordials;
const {
  getAsyncContext,
  setAsyncContext,
} = core;

import * as webidl from "ext:deno_webidl/00_webidl.js";

const hrU8 = new Uint8Array(8);
const hr = new Uint32Array(TypedArrayPrototypeGetBuffer(hrU8));
function opNow() {
  op_now(hrU8);
  return (hr[0] * 1000 + hr[1] / 1e6);
}

const activeTimers = new Map();

// ---------------------------------------------------------------------------

function checkThis(thisArg) {
  if (thisArg !== null && thisArg !== undefined && thisArg !== globalThis) {
    throw new TypeError("Illegal invocation");
  }
}

/**
 * Call a callback function immediately.
 */
function setImmediate(callback, ...args) {
  const asyncContext = getAsyncContext();
  return core.queueImmediate(() => {
    const oldContext = getAsyncContext();
    try {
      setAsyncContext(asyncContext);
      return ReflectApply(callback, globalThis, args);
    } finally {
      setAsyncContext(oldContext);
    }
  });
}

/**
 * Call a callback function after a delay.
 */
function setTimeout(callback, timeout = 0, ...args) {
  checkThis(this);
  // If callback is a string, replace it with a function that evals the string on every timeout
  if (typeof callback !== "function") {
    const unboundCallback = webidl.converters.DOMString(callback);
    callback = () => indirectEval(unboundCallback);
  }
  const unboundCallback = callback;
  const asyncContext = getAsyncContext();
  let timer;
  callback = () => {
    const oldContext = getAsyncContext();
    try {
      setAsyncContext(asyncContext);
      ReflectApply(unboundCallback, globalThis, args);
    } finally {
      setAsyncContext(oldContext);
      if (timer) {
        activeTimers.delete(timer._timerId);
      }
    }
  };
  timeout = webidl.converters.long(timeout);
  timer = core.createTimer(callback, timeout, undefined, false, true, false);
  activeTimers.set(timer._timerId, timer);
  return timer._timerId;
}

/**
 * Call a callback function after a delay.
 */
function setInterval(callback, timeout = 0, ...args) {
  checkThis(this);
  if (typeof callback !== "function") {
    const unboundCallback = webidl.converters.DOMString(callback);
    callback = () => indirectEval(unboundCallback);
  }
  const unboundCallback = callback;
  const asyncContext = getAsyncContext();
  callback = () => {
    const oldContext = getAsyncContext(asyncContext);
    try {
      setAsyncContext(asyncContext);
      ReflectApply(unboundCallback, globalThis, args);
    } finally {
      setAsyncContext(oldContext);
    }
  };
  timeout = webidl.converters.long(timeout);
  const timer = core.createTimer(callback, timeout, undefined, true, true, false);
  activeTimers.set(timer._timerId, timer);
  return timer._timerId;
}

/**
 * Clear a timeout or interval.
 */
function clearTimeout(id = 0) {
  checkThis(this);
  id = webidl.converters.long(id);
  const timer = activeTimers.get(id);
  if (timer) {
    core.cancelTimer(timer);
    activeTimers.delete(id);
  }
}

/**
 * Clear a timeout or interval.
 */
function clearInterval(id = 0) {
  checkThis(this);
  id = webidl.converters.long(id);
  const timer = activeTimers.get(id);
  if (timer) {
    core.cancelTimer(timer);
    activeTimers.delete(id);
  }
}

/**
 * Mark a timer as not blocking event loop exit.
 */
function unrefTimer(id) {
  const timer = typeof id === "number" ? activeTimers.get(id) : id;
  if (timer) {
    core.unrefTimer(timer);
  }
}

/**
 * Mark a timer as blocking event loop exit.
 */
function refTimer(id) {
  const timer = typeof id === "number" ? activeTimers.get(id) : id;
  if (timer) {
    core.refTimer(timer);
  }
}

// Defer to avoid starving the event loop. Not using queueMicrotask()
// for that reason: it lets promises make forward progress but can
// still starve other parts of the event loop.
function defer(go) {
  PromisePrototypeThen(op_defer(), () => go());
}

export {
  clearInterval,
  clearTimeout,
  defer,
  opNow,
  refTimer,
  setImmediate,
  setInterval,
  setTimeout,
  unrefTimer,
};
