// Minimal console implementation for web_stub
// This provides the Console class and inspect function that init_console.js needs

import { primordials } from "ext:core/mod.js";
const {
  ObjectAssign,
  ObjectFromEntries,
  ArrayPrototypeMap,
  ObjectGetOwnPropertyDescriptor,
} = primordials;

const inspectOptions = {
  depth: 4,
  colors: false,
  iteratorLimit: 100,
  maxStringLength: Infinity,
};

function inspect(value, options = {}) {
  const opts = { ...inspectOptions, ...options };
  return formatValue(value, opts.depth, new Set());
}

// Creates a proxy object that filters properties for inspect
// This is used by DOMException and other objects for custom inspect
function createFilteredInspectProxy({ object, evaluate, keys }) {
  if (!evaluate) {
    return object;
  }
  const descriptors = ObjectFromEntries(
    ArrayPrototypeMap(keys, (key) => {
      return [key, ObjectGetOwnPropertyDescriptor(object, key) ?? {
        value: object[key],
        enumerable: true,
      }];
    }),
  );
  return ObjectAssign({}, descriptors);
}

function formatValue(value, depth, seen) {
  if (value === null) return 'null';
  if (value === undefined) return 'undefined';

  const type = typeof value;

  if (type === 'string') return JSON.stringify(value);
  if (type === 'number' || type === 'boolean') return String(value);
  if (type === 'bigint') return `${value}n`;
  if (type === 'symbol') return value.toString();
  if (type === 'function') return `[Function: ${value.name || 'anonymous'}]`;

  if (type === 'object') {
    if (seen.has(value)) return '[Circular]';
    seen.add(value);

    if (depth <= 0) return '[Object]';

    if (Array.isArray(value)) {
      const items = value.map(v => formatValue(v, depth - 1, seen));
      return `[ ${items.join(', ')} ]`;
    }

    if (value instanceof Error) {
      return value.stack || value.message || String(value);
    }

    if (value instanceof Date) {
      return value.toISOString();
    }

    if (value instanceof RegExp) {
      return value.toString();
    }

    const entries = Object.entries(value)
      .map(([k, v]) => `${k}: ${formatValue(v, depth - 1, seen)}`)
      .join(', ');
    return `{ ${entries} }`;
  }

  return String(value);
}

class Console {
  #printFn;

  constructor(printFn) {
    this.#printFn = printFn;
  }

  #print(level, ...args) {
    const msg = args.map(a => typeof a === 'string' ? a : inspect(a)).join(' ') + '\n';
    this.#printFn(msg, level);
  }

  log(...args) { this.#print(1, ...args); }
  debug(...args) { this.#print(0, ...args); }
  info(...args) { this.#print(1, ...args); }
  warn(...args) { this.#print(2, ...args); }
  error(...args) { this.#print(3, ...args); }

  dir(obj, options) { this.log(inspect(obj, options)); }
  dirxml(...args) { this.log(...args); }

  assert(condition, ...args) {
    if (!condition) {
      this.error('Assertion failed:', ...args);
    }
  }

  clear() { /* no-op in stub */ }

  count(label = 'default') {
    this.#counts.set(label, (this.#counts.get(label) || 0) + 1);
    this.log(`${label}: ${this.#counts.get(label)}`);
  }
  #counts = new Map();

  countReset(label = 'default') {
    this.#counts.delete(label);
  }

  group(...args) { this.log(...args); }
  groupCollapsed(...args) { this.log(...args); }
  groupEnd() { /* no-op */ }

  table(data) { this.log(inspect(data)); }

  time(label = 'default') {
    this.#timers.set(label, performance.now());
  }
  #timers = new Map();

  timeEnd(label = 'default') {
    const start = this.#timers.get(label);
    if (start !== undefined) {
      this.log(`${label}: ${(performance.now() - start).toFixed(3)}ms`);
      this.#timers.delete(label);
    }
  }

  timeLog(label = 'default', ...args) {
    const start = this.#timers.get(label);
    if (start !== undefined) {
      this.log(`${label}: ${(performance.now() - start).toFixed(3)}ms`, ...args);
    }
  }

  trace(...args) {
    const err = new Error();
    this.log(...args, '\n', err.stack);
  }
}

export { Console, inspect, createFilteredInspectProxy };
