(() => {
  var __create = Object.create;
  var __getProtoOf = Object.getPrototypeOf;
  var __defProp = Object.defineProperty;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  function __accessProp(key) {
    return this[key];
  }
  var __toESMCache_node;
  var __toESMCache_esm;
  var __toESM = (mod, isNodeMode, target) => {
    var canCache = mod != null && typeof mod === "object";
    if (canCache) {
      var cache = isNodeMode ? __toESMCache_node ??= new WeakMap : __toESMCache_esm ??= new WeakMap;
      var cached = cache.get(mod);
      if (cached)
        return cached;
    }
    target = mod != null ? __create(__getProtoOf(mod)) : {};
    const to = isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target;
    for (let key of __getOwnPropNames(mod))
      if (!__hasOwnProp.call(to, key))
        __defProp(to, key, {
          get: __accessProp.bind(mod, key),
          enumerable: true
        });
    if (canCache)
      cache.set(mod, to);
    return to;
  };
  var __toCommonJS = (from) => {
    var entry = (__moduleCache ??= new WeakMap).get(from), desc;
    if (entry)
      return entry;
    entry = __defProp({}, "__esModule", { value: true });
    if (from && typeof from === "object" || typeof from === "function") {
      for (var key of __getOwnPropNames(from))
        if (!__hasOwnProp.call(entry, key))
          __defProp(entry, key, {
            get: __accessProp.bind(from, key),
            enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
          });
    }
    __moduleCache.set(from, entry);
    return entry;
  };
  var __moduleCache;
  var __commonJS = (cb, mod) => () => (mod || cb((mod = { exports: {} }).exports, mod), mod.exports);
  var __returnValue = (v) => v;
  function __exportSetter(name, newValue) {
    this[name] = __returnValue.bind(null, newValue);
  }
  var __export = (target, all) => {
    for (var name in all)
      __defProp(target, name, {
        get: all[name],
        enumerable: true,
        configurable: true,
        set: __exportSetter.bind(all, name)
      });
  };
  var __require = /* @__PURE__ */ ((x) => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, {
    get: (a, b) => (typeof require !== "undefined" ? require : a)[b]
  }) : x)(function(x) {
    if (typeof require !== "undefined")
      return require.apply(this, arguments);
    throw Error('Dynamic require of "' + x + '" is not supported');
  });

  // ../../node_modules/.bun/react@19.2.7/node_modules/react/cjs/react.development.js
  var require_react_development = __commonJS((exports, module) => {
    (function() {
      function defineDeprecationWarning(methodName, info) {
        Object.defineProperty(Component.prototype, methodName, {
          get: function() {
            console.warn("%s(...) is deprecated in plain JavaScript React classes. %s", info[0], info[1]);
          }
        });
      }
      function getIteratorFn(maybeIterable) {
        if (maybeIterable === null || typeof maybeIterable !== "object")
          return null;
        maybeIterable = MAYBE_ITERATOR_SYMBOL && maybeIterable[MAYBE_ITERATOR_SYMBOL] || maybeIterable["@@iterator"];
        return typeof maybeIterable === "function" ? maybeIterable : null;
      }
      function warnNoop(publicInstance, callerName) {
        publicInstance = (publicInstance = publicInstance.constructor) && (publicInstance.displayName || publicInstance.name) || "ReactClass";
        var warningKey = publicInstance + "." + callerName;
        didWarnStateUpdateForUnmountedComponent[warningKey] || (console.error("Can't call %s on a component that is not yet mounted. This is a no-op, but it might indicate a bug in your application. Instead, assign to `this.state` directly or define a `state = {};` class property with the desired state in the %s component.", callerName, publicInstance), didWarnStateUpdateForUnmountedComponent[warningKey] = true);
      }
      function Component(props, context, updater) {
        this.props = props;
        this.context = context;
        this.refs = emptyObject;
        this.updater = updater || ReactNoopUpdateQueue;
      }
      function ComponentDummy() {}
      function PureComponent(props, context, updater) {
        this.props = props;
        this.context = context;
        this.refs = emptyObject;
        this.updater = updater || ReactNoopUpdateQueue;
      }
      function noop2() {}
      function testStringCoercion(value) {
        return "" + value;
      }
      function checkKeyStringCoercion(value) {
        try {
          testStringCoercion(value);
          var JSCompiler_inline_result = false;
        } catch (e) {
          JSCompiler_inline_result = true;
        }
        if (JSCompiler_inline_result) {
          JSCompiler_inline_result = console;
          var JSCompiler_temp_const = JSCompiler_inline_result.error;
          var JSCompiler_inline_result$jscomp$0 = typeof Symbol === "function" && Symbol.toStringTag && value[Symbol.toStringTag] || value.constructor.name || "Object";
          JSCompiler_temp_const.call(JSCompiler_inline_result, "The provided key is an unsupported type %s. This value must be coerced to a string before using it here.", JSCompiler_inline_result$jscomp$0);
          return testStringCoercion(value);
        }
      }
      function getComponentNameFromType(type) {
        if (type == null)
          return null;
        if (typeof type === "function")
          return type.$$typeof === REACT_CLIENT_REFERENCE ? null : type.displayName || type.name || null;
        if (typeof type === "string")
          return type;
        switch (type) {
          case REACT_FRAGMENT_TYPE:
            return "Fragment";
          case REACT_PROFILER_TYPE:
            return "Profiler";
          case REACT_STRICT_MODE_TYPE:
            return "StrictMode";
          case REACT_SUSPENSE_TYPE:
            return "Suspense";
          case REACT_SUSPENSE_LIST_TYPE:
            return "SuspenseList";
          case REACT_ACTIVITY_TYPE:
            return "Activity";
        }
        if (typeof type === "object")
          switch (typeof type.tag === "number" && console.error("Received an unexpected object in getComponentNameFromType(). This is likely a bug in React. Please file an issue."), type.$$typeof) {
            case REACT_PORTAL_TYPE:
              return "Portal";
            case REACT_CONTEXT_TYPE:
              return type.displayName || "Context";
            case REACT_CONSUMER_TYPE:
              return (type._context.displayName || "Context") + ".Consumer";
            case REACT_FORWARD_REF_TYPE:
              var innerType = type.render;
              type = type.displayName;
              type || (type = innerType.displayName || innerType.name || "", type = type !== "" ? "ForwardRef(" + type + ")" : "ForwardRef");
              return type;
            case REACT_MEMO_TYPE:
              return innerType = type.displayName || null, innerType !== null ? innerType : getComponentNameFromType(type.type) || "Memo";
            case REACT_LAZY_TYPE:
              innerType = type._payload;
              type = type._init;
              try {
                return getComponentNameFromType(type(innerType));
              } catch (x) {}
          }
        return null;
      }
      function getTaskName(type) {
        if (type === REACT_FRAGMENT_TYPE)
          return "<>";
        if (typeof type === "object" && type !== null && type.$$typeof === REACT_LAZY_TYPE)
          return "<...>";
        try {
          var name = getComponentNameFromType(type);
          return name ? "<" + name + ">" : "<...>";
        } catch (x) {
          return "<...>";
        }
      }
      function getOwner() {
        var dispatcher = ReactSharedInternals.A;
        return dispatcher === null ? null : dispatcher.getOwner();
      }
      function UnknownOwner() {
        return Error("react-stack-top-frame");
      }
      function hasValidKey(config) {
        if (hasOwnProperty.call(config, "key")) {
          var getter = Object.getOwnPropertyDescriptor(config, "key").get;
          if (getter && getter.isReactWarning)
            return false;
        }
        return config.key !== undefined;
      }
      function defineKeyPropWarningGetter(props, displayName) {
        function warnAboutAccessingKey() {
          specialPropKeyWarningShown || (specialPropKeyWarningShown = true, console.error("%s: `key` is not a prop. Trying to access it will result in `undefined` being returned. If you need to access the same value within the child component, you should pass it as a different prop. (https://react.dev/link/special-props)", displayName));
        }
        warnAboutAccessingKey.isReactWarning = true;
        Object.defineProperty(props, "key", {
          get: warnAboutAccessingKey,
          configurable: true
        });
      }
      function elementRefGetterWithDeprecationWarning() {
        var componentName = getComponentNameFromType(this.type);
        didWarnAboutElementRef[componentName] || (didWarnAboutElementRef[componentName] = true, console.error("Accessing element.ref was removed in React 19. ref is now a regular prop. It will be removed from the JSX Element type in a future release."));
        componentName = this.props.ref;
        return componentName !== undefined ? componentName : null;
      }
      function ReactElement(type, key, props, owner, debugStack, debugTask) {
        var refProp = props.ref;
        type = {
          $$typeof: REACT_ELEMENT_TYPE,
          type,
          key,
          props,
          _owner: owner
        };
        (refProp !== undefined ? refProp : null) !== null ? Object.defineProperty(type, "ref", {
          enumerable: false,
          get: elementRefGetterWithDeprecationWarning
        }) : Object.defineProperty(type, "ref", { enumerable: false, value: null });
        type._store = {};
        Object.defineProperty(type._store, "validated", {
          configurable: false,
          enumerable: false,
          writable: true,
          value: 0
        });
        Object.defineProperty(type, "_debugInfo", {
          configurable: false,
          enumerable: false,
          writable: true,
          value: null
        });
        Object.defineProperty(type, "_debugStack", {
          configurable: false,
          enumerable: false,
          writable: true,
          value: debugStack
        });
        Object.defineProperty(type, "_debugTask", {
          configurable: false,
          enumerable: false,
          writable: true,
          value: debugTask
        });
        Object.freeze && (Object.freeze(type.props), Object.freeze(type));
        return type;
      }
      function cloneAndReplaceKey(oldElement, newKey) {
        newKey = ReactElement(oldElement.type, newKey, oldElement.props, oldElement._owner, oldElement._debugStack, oldElement._debugTask);
        oldElement._store && (newKey._store.validated = oldElement._store.validated);
        return newKey;
      }
      function validateChildKeys(node) {
        isValidElement(node) ? node._store && (node._store.validated = 1) : typeof node === "object" && node !== null && node.$$typeof === REACT_LAZY_TYPE && (node._payload.status === "fulfilled" ? isValidElement(node._payload.value) && node._payload.value._store && (node._payload.value._store.validated = 1) : node._store && (node._store.validated = 1));
      }
      function isValidElement(object) {
        return typeof object === "object" && object !== null && object.$$typeof === REACT_ELEMENT_TYPE;
      }
      function escape(key) {
        var escaperLookup = { "=": "=0", ":": "=2" };
        return "$" + key.replace(/[=:]/g, function(match) {
          return escaperLookup[match];
        });
      }
      function getElementKey(element, index) {
        return typeof element === "object" && element !== null && element.key != null ? (checkKeyStringCoercion(element.key), escape("" + element.key)) : index.toString(36);
      }
      function resolveThenable(thenable) {
        switch (thenable.status) {
          case "fulfilled":
            return thenable.value;
          case "rejected":
            throw thenable.reason;
          default:
            switch (typeof thenable.status === "string" ? thenable.then(noop2, noop2) : (thenable.status = "pending", thenable.then(function(fulfilledValue) {
              thenable.status === "pending" && (thenable.status = "fulfilled", thenable.value = fulfilledValue);
            }, function(error) {
              thenable.status === "pending" && (thenable.status = "rejected", thenable.reason = error);
            })), thenable.status) {
              case "fulfilled":
                return thenable.value;
              case "rejected":
                throw thenable.reason;
            }
        }
        throw thenable;
      }
      function mapIntoArray(children, array, escapedPrefix, nameSoFar, callback) {
        var type = typeof children;
        if (type === "undefined" || type === "boolean")
          children = null;
        var invokeCallback = false;
        if (children === null)
          invokeCallback = true;
        else
          switch (type) {
            case "bigint":
            case "string":
            case "number":
              invokeCallback = true;
              break;
            case "object":
              switch (children.$$typeof) {
                case REACT_ELEMENT_TYPE:
                case REACT_PORTAL_TYPE:
                  invokeCallback = true;
                  break;
                case REACT_LAZY_TYPE:
                  return invokeCallback = children._init, mapIntoArray(invokeCallback(children._payload), array, escapedPrefix, nameSoFar, callback);
              }
          }
        if (invokeCallback) {
          invokeCallback = children;
          callback = callback(invokeCallback);
          var childKey = nameSoFar === "" ? "." + getElementKey(invokeCallback, 0) : nameSoFar;
          isArrayImpl(callback) ? (escapedPrefix = "", childKey != null && (escapedPrefix = childKey.replace(userProvidedKeyEscapeRegex, "$&/") + "/"), mapIntoArray(callback, array, escapedPrefix, "", function(c) {
            return c;
          })) : callback != null && (isValidElement(callback) && (callback.key != null && (invokeCallback && invokeCallback.key === callback.key || checkKeyStringCoercion(callback.key)), escapedPrefix = cloneAndReplaceKey(callback, escapedPrefix + (callback.key == null || invokeCallback && invokeCallback.key === callback.key ? "" : ("" + callback.key).replace(userProvidedKeyEscapeRegex, "$&/") + "/") + childKey), nameSoFar !== "" && invokeCallback != null && isValidElement(invokeCallback) && invokeCallback.key == null && invokeCallback._store && !invokeCallback._store.validated && (escapedPrefix._store.validated = 2), callback = escapedPrefix), array.push(callback));
          return 1;
        }
        invokeCallback = 0;
        childKey = nameSoFar === "" ? "." : nameSoFar + ":";
        if (isArrayImpl(children))
          for (var i = 0;i < children.length; i++)
            nameSoFar = children[i], type = childKey + getElementKey(nameSoFar, i), invokeCallback += mapIntoArray(nameSoFar, array, escapedPrefix, type, callback);
        else if (i = getIteratorFn(children), typeof i === "function")
          for (i === children.entries && (didWarnAboutMaps || console.warn("Using Maps as children is not supported. Use an array of keyed ReactElements instead."), didWarnAboutMaps = true), children = i.call(children), i = 0;!(nameSoFar = children.next()).done; )
            nameSoFar = nameSoFar.value, type = childKey + getElementKey(nameSoFar, i++), invokeCallback += mapIntoArray(nameSoFar, array, escapedPrefix, type, callback);
        else if (type === "object") {
          if (typeof children.then === "function")
            return mapIntoArray(resolveThenable(children), array, escapedPrefix, nameSoFar, callback);
          array = String(children);
          throw Error("Objects are not valid as a React child (found: " + (array === "[object Object]" ? "object with keys {" + Object.keys(children).join(", ") + "}" : array) + "). If you meant to render a collection of children, use an array instead.");
        }
        return invokeCallback;
      }
      function mapChildren(children, func, context) {
        if (children == null)
          return children;
        var result = [], count = 0;
        mapIntoArray(children, result, "", "", function(child) {
          return func.call(context, child, count++);
        });
        return result;
      }
      function lazyInitializer(payload) {
        if (payload._status === -1) {
          var ioInfo = payload._ioInfo;
          ioInfo != null && (ioInfo.start = ioInfo.end = performance.now());
          ioInfo = payload._result;
          var thenable = ioInfo();
          thenable.then(function(moduleObject) {
            if (payload._status === 0 || payload._status === -1) {
              payload._status = 1;
              payload._result = moduleObject;
              var _ioInfo = payload._ioInfo;
              _ioInfo != null && (_ioInfo.end = performance.now());
              thenable.status === undefined && (thenable.status = "fulfilled", thenable.value = moduleObject);
            }
          }, function(error) {
            if (payload._status === 0 || payload._status === -1) {
              payload._status = 2;
              payload._result = error;
              var _ioInfo2 = payload._ioInfo;
              _ioInfo2 != null && (_ioInfo2.end = performance.now());
              thenable.status === undefined && (thenable.status = "rejected", thenable.reason = error);
            }
          });
          ioInfo = payload._ioInfo;
          if (ioInfo != null) {
            ioInfo.value = thenable;
            var displayName = thenable.displayName;
            typeof displayName === "string" && (ioInfo.name = displayName);
          }
          payload._status === -1 && (payload._status = 0, payload._result = thenable);
        }
        if (payload._status === 1)
          return ioInfo = payload._result, ioInfo === undefined && console.error(`lazy: Expected the result of a dynamic import() call. Instead received: %s

Your code should look like: 
  const MyComponent = lazy(() => import('./MyComponent'))

Did you accidentally put curly braces around the import?`, ioInfo), "default" in ioInfo || console.error(`lazy: Expected the result of a dynamic import() call. Instead received: %s

Your code should look like: 
  const MyComponent = lazy(() => import('./MyComponent'))`, ioInfo), ioInfo.default;
        throw payload._result;
      }
      function resolveDispatcher() {
        var dispatcher = ReactSharedInternals.H;
        dispatcher === null && console.error(`Invalid hook call. Hooks can only be called inside of the body of a function component. This could happen for one of the following reasons:
1. You might have mismatching versions of React and the renderer (such as React DOM)
2. You might be breaking the Rules of Hooks
3. You might have more than one copy of React in the same app
See https://react.dev/link/invalid-hook-call for tips about how to debug and fix this problem.`);
        return dispatcher;
      }
      function releaseAsyncTransition() {
        ReactSharedInternals.asyncTransitions--;
      }
      function enqueueTask(task) {
        if (enqueueTaskImpl === null)
          try {
            var requireString = ("require" + Math.random()).slice(0, 7);
            enqueueTaskImpl = (module && module[requireString]).call(module, "timers").setImmediate;
          } catch (_err) {
            enqueueTaskImpl = function(callback) {
              didWarnAboutMessageChannel === false && (didWarnAboutMessageChannel = true, typeof MessageChannel === "undefined" && console.error("This browser does not have a MessageChannel implementation, so enqueuing tasks via await act(async () => ...) will fail. Please file an issue at https://github.com/facebook/react/issues if you encounter this warning."));
              var channel = new MessageChannel;
              channel.port1.onmessage = callback;
              channel.port2.postMessage(undefined);
            };
          }
        return enqueueTaskImpl(task);
      }
      function aggregateErrors(errors) {
        return 1 < errors.length && typeof AggregateError === "function" ? new AggregateError(errors) : errors[0];
      }
      function popActScope(prevActQueue, prevActScopeDepth) {
        prevActScopeDepth !== actScopeDepth - 1 && console.error("You seem to have overlapping act() calls, this is not supported. Be sure to await previous act() calls before making a new one. ");
        actScopeDepth = prevActScopeDepth;
      }
      function recursivelyFlushAsyncActWork(returnValue, resolve, reject) {
        var queue = ReactSharedInternals.actQueue;
        if (queue !== null)
          if (queue.length !== 0)
            try {
              flushActQueue(queue);
              enqueueTask(function() {
                return recursivelyFlushAsyncActWork(returnValue, resolve, reject);
              });
              return;
            } catch (error) {
              ReactSharedInternals.thrownErrors.push(error);
            }
          else
            ReactSharedInternals.actQueue = null;
        0 < ReactSharedInternals.thrownErrors.length ? (queue = aggregateErrors(ReactSharedInternals.thrownErrors), ReactSharedInternals.thrownErrors.length = 0, reject(queue)) : resolve(returnValue);
      }
      function flushActQueue(queue) {
        if (!isFlushing) {
          isFlushing = true;
          var i = 0;
          try {
            for (;i < queue.length; i++) {
              var callback = queue[i];
              do {
                ReactSharedInternals.didUsePromise = false;
                var continuation = callback(false);
                if (continuation !== null) {
                  if (ReactSharedInternals.didUsePromise) {
                    queue[i] = callback;
                    queue.splice(0, i);
                    return;
                  }
                  callback = continuation;
                } else
                  break;
              } while (1);
            }
            queue.length = 0;
          } catch (error) {
            queue.splice(0, i + 1), ReactSharedInternals.thrownErrors.push(error);
          } finally {
            isFlushing = false;
          }
        }
      }
      typeof __REACT_DEVTOOLS_GLOBAL_HOOK__ !== "undefined" && typeof __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStart === "function" && __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStart(Error());
      var REACT_ELEMENT_TYPE = Symbol.for("react.transitional.element"), REACT_PORTAL_TYPE = Symbol.for("react.portal"), REACT_FRAGMENT_TYPE = Symbol.for("react.fragment"), REACT_STRICT_MODE_TYPE = Symbol.for("react.strict_mode"), REACT_PROFILER_TYPE = Symbol.for("react.profiler"), REACT_CONSUMER_TYPE = Symbol.for("react.consumer"), REACT_CONTEXT_TYPE = Symbol.for("react.context"), REACT_FORWARD_REF_TYPE = Symbol.for("react.forward_ref"), REACT_SUSPENSE_TYPE = Symbol.for("react.suspense"), REACT_SUSPENSE_LIST_TYPE = Symbol.for("react.suspense_list"), REACT_MEMO_TYPE = Symbol.for("react.memo"), REACT_LAZY_TYPE = Symbol.for("react.lazy"), REACT_ACTIVITY_TYPE = Symbol.for("react.activity"), MAYBE_ITERATOR_SYMBOL = Symbol.iterator, didWarnStateUpdateForUnmountedComponent = {}, ReactNoopUpdateQueue = {
        isMounted: function() {
          return false;
        },
        enqueueForceUpdate: function(publicInstance) {
          warnNoop(publicInstance, "forceUpdate");
        },
        enqueueReplaceState: function(publicInstance) {
          warnNoop(publicInstance, "replaceState");
        },
        enqueueSetState: function(publicInstance) {
          warnNoop(publicInstance, "setState");
        }
      }, assign = Object.assign, emptyObject = {};
      Object.freeze(emptyObject);
      Component.prototype.isReactComponent = {};
      Component.prototype.setState = function(partialState, callback) {
        if (typeof partialState !== "object" && typeof partialState !== "function" && partialState != null)
          throw Error("takes an object of state variables to update or a function which returns an object of state variables.");
        this.updater.enqueueSetState(this, partialState, callback, "setState");
      };
      Component.prototype.forceUpdate = function(callback) {
        this.updater.enqueueForceUpdate(this, callback, "forceUpdate");
      };
      var deprecatedAPIs = {
        isMounted: [
          "isMounted",
          "Instead, make sure to clean up subscriptions and pending requests in componentWillUnmount to prevent memory leaks."
        ],
        replaceState: [
          "replaceState",
          "Refactor your code to use setState instead (see https://github.com/facebook/react/issues/3236)."
        ]
      };
      for (fnName in deprecatedAPIs)
        deprecatedAPIs.hasOwnProperty(fnName) && defineDeprecationWarning(fnName, deprecatedAPIs[fnName]);
      ComponentDummy.prototype = Component.prototype;
      deprecatedAPIs = PureComponent.prototype = new ComponentDummy;
      deprecatedAPIs.constructor = PureComponent;
      assign(deprecatedAPIs, Component.prototype);
      deprecatedAPIs.isPureReactComponent = true;
      var isArrayImpl = Array.isArray, REACT_CLIENT_REFERENCE = Symbol.for("react.client.reference"), ReactSharedInternals = {
        H: null,
        A: null,
        T: null,
        S: null,
        actQueue: null,
        asyncTransitions: 0,
        isBatchingLegacy: false,
        didScheduleLegacyUpdate: false,
        didUsePromise: false,
        thrownErrors: [],
        getCurrentStack: null,
        recentlyCreatedOwnerStacks: 0
      }, hasOwnProperty = Object.prototype.hasOwnProperty, createTask = console.createTask ? console.createTask : function() {
        return null;
      };
      deprecatedAPIs = {
        react_stack_bottom_frame: function(callStackForError) {
          return callStackForError();
        }
      };
      var specialPropKeyWarningShown, didWarnAboutOldJSXRuntime;
      var didWarnAboutElementRef = {};
      var unknownOwnerDebugStack = deprecatedAPIs.react_stack_bottom_frame.bind(deprecatedAPIs, UnknownOwner)();
      var unknownOwnerDebugTask = createTask(getTaskName(UnknownOwner));
      var didWarnAboutMaps = false, userProvidedKeyEscapeRegex = /\/+/g, reportGlobalError = typeof reportError === "function" ? reportError : function(error) {
        if (typeof window === "object" && typeof window.ErrorEvent === "function") {
          var event = new window.ErrorEvent("error", {
            bubbles: true,
            cancelable: true,
            message: typeof error === "object" && error !== null && typeof error.message === "string" ? String(error.message) : String(error),
            error
          });
          if (!window.dispatchEvent(event))
            return;
        } else if (typeof process === "object" && typeof process.emit === "function") {
          process.emit("uncaughtException", error);
          return;
        }
        console.error(error);
      }, didWarnAboutMessageChannel = false, enqueueTaskImpl = null, actScopeDepth = 0, didWarnNoAwaitAct = false, isFlushing = false, queueSeveralMicrotasks = typeof queueMicrotask === "function" ? function(callback) {
        queueMicrotask(function() {
          return queueMicrotask(callback);
        });
      } : enqueueTask;
      deprecatedAPIs = Object.freeze({
        __proto__: null,
        c: function(size) {
          return resolveDispatcher().useMemoCache(size);
        }
      });
      var fnName = {
        map: mapChildren,
        forEach: function(children, forEachFunc, forEachContext) {
          mapChildren(children, function() {
            forEachFunc.apply(this, arguments);
          }, forEachContext);
        },
        count: function(children) {
          var n = 0;
          mapChildren(children, function() {
            n++;
          });
          return n;
        },
        toArray: function(children) {
          return mapChildren(children, function(child) {
            return child;
          }) || [];
        },
        only: function(children) {
          if (!isValidElement(children))
            throw Error("React.Children.only expected to receive a single React element child.");
          return children;
        }
      };
      exports.Activity = REACT_ACTIVITY_TYPE;
      exports.Children = fnName;
      exports.Component = Component;
      exports.Fragment = REACT_FRAGMENT_TYPE;
      exports.Profiler = REACT_PROFILER_TYPE;
      exports.PureComponent = PureComponent;
      exports.StrictMode = REACT_STRICT_MODE_TYPE;
      exports.Suspense = REACT_SUSPENSE_TYPE;
      exports.__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE = ReactSharedInternals;
      exports.__COMPILER_RUNTIME = deprecatedAPIs;
      exports.act = function(callback) {
        var prevActQueue = ReactSharedInternals.actQueue, prevActScopeDepth = actScopeDepth;
        actScopeDepth++;
        var queue = ReactSharedInternals.actQueue = prevActQueue !== null ? prevActQueue : [], didAwaitActCall = false;
        try {
          var result = callback();
        } catch (error) {
          ReactSharedInternals.thrownErrors.push(error);
        }
        if (0 < ReactSharedInternals.thrownErrors.length)
          throw popActScope(prevActQueue, prevActScopeDepth), callback = aggregateErrors(ReactSharedInternals.thrownErrors), ReactSharedInternals.thrownErrors.length = 0, callback;
        if (result !== null && typeof result === "object" && typeof result.then === "function") {
          var thenable = result;
          queueSeveralMicrotasks(function() {
            didAwaitActCall || didWarnNoAwaitAct || (didWarnNoAwaitAct = true, console.error("You called act(async () => ...) without await. This could lead to unexpected testing behaviour, interleaving multiple act calls and mixing their scopes. You should - await act(async () => ...);"));
          });
          return {
            then: function(resolve, reject) {
              didAwaitActCall = true;
              thenable.then(function(returnValue) {
                popActScope(prevActQueue, prevActScopeDepth);
                if (prevActScopeDepth === 0) {
                  try {
                    flushActQueue(queue), enqueueTask(function() {
                      return recursivelyFlushAsyncActWork(returnValue, resolve, reject);
                    });
                  } catch (error$0) {
                    ReactSharedInternals.thrownErrors.push(error$0);
                  }
                  if (0 < ReactSharedInternals.thrownErrors.length) {
                    var _thrownError = aggregateErrors(ReactSharedInternals.thrownErrors);
                    ReactSharedInternals.thrownErrors.length = 0;
                    reject(_thrownError);
                  }
                } else
                  resolve(returnValue);
              }, function(error) {
                popActScope(prevActQueue, prevActScopeDepth);
                0 < ReactSharedInternals.thrownErrors.length ? (error = aggregateErrors(ReactSharedInternals.thrownErrors), ReactSharedInternals.thrownErrors.length = 0, reject(error)) : reject(error);
              });
            }
          };
        }
        var returnValue$jscomp$0 = result;
        popActScope(prevActQueue, prevActScopeDepth);
        prevActScopeDepth === 0 && (flushActQueue(queue), queue.length !== 0 && queueSeveralMicrotasks(function() {
          didAwaitActCall || didWarnNoAwaitAct || (didWarnNoAwaitAct = true, console.error("A component suspended inside an `act` scope, but the `act` call was not awaited. When testing React components that depend on asynchronous data, you must await the result:\n\nawait act(() => ...)"));
        }), ReactSharedInternals.actQueue = null);
        if (0 < ReactSharedInternals.thrownErrors.length)
          throw callback = aggregateErrors(ReactSharedInternals.thrownErrors), ReactSharedInternals.thrownErrors.length = 0, callback;
        return {
          then: function(resolve, reject) {
            didAwaitActCall = true;
            prevActScopeDepth === 0 ? (ReactSharedInternals.actQueue = queue, enqueueTask(function() {
              return recursivelyFlushAsyncActWork(returnValue$jscomp$0, resolve, reject);
            })) : resolve(returnValue$jscomp$0);
          }
        };
      };
      exports.cache = function(fn) {
        return function() {
          return fn.apply(null, arguments);
        };
      };
      exports.cacheSignal = function() {
        return null;
      };
      exports.captureOwnerStack = function() {
        var getCurrentStack = ReactSharedInternals.getCurrentStack;
        return getCurrentStack === null ? null : getCurrentStack();
      };
      exports.cloneElement = function(element, config, children) {
        if (element === null || element === undefined)
          throw Error("The argument must be a React element, but you passed " + element + ".");
        var props = assign({}, element.props), key = element.key, owner = element._owner;
        if (config != null) {
          var JSCompiler_inline_result;
          a: {
            if (hasOwnProperty.call(config, "ref") && (JSCompiler_inline_result = Object.getOwnPropertyDescriptor(config, "ref").get) && JSCompiler_inline_result.isReactWarning) {
              JSCompiler_inline_result = false;
              break a;
            }
            JSCompiler_inline_result = config.ref !== undefined;
          }
          JSCompiler_inline_result && (owner = getOwner());
          hasValidKey(config) && (checkKeyStringCoercion(config.key), key = "" + config.key);
          for (propName in config)
            !hasOwnProperty.call(config, propName) || propName === "key" || propName === "__self" || propName === "__source" || propName === "ref" && config.ref === undefined || (props[propName] = config[propName]);
        }
        var propName = arguments.length - 2;
        if (propName === 1)
          props.children = children;
        else if (1 < propName) {
          JSCompiler_inline_result = Array(propName);
          for (var i = 0;i < propName; i++)
            JSCompiler_inline_result[i] = arguments[i + 2];
          props.children = JSCompiler_inline_result;
        }
        props = ReactElement(element.type, key, props, owner, element._debugStack, element._debugTask);
        for (key = 2;key < arguments.length; key++)
          validateChildKeys(arguments[key]);
        return props;
      };
      exports.createContext = function(defaultValue) {
        defaultValue = {
          $$typeof: REACT_CONTEXT_TYPE,
          _currentValue: defaultValue,
          _currentValue2: defaultValue,
          _threadCount: 0,
          Provider: null,
          Consumer: null
        };
        defaultValue.Provider = defaultValue;
        defaultValue.Consumer = {
          $$typeof: REACT_CONSUMER_TYPE,
          _context: defaultValue
        };
        defaultValue._currentRenderer = null;
        defaultValue._currentRenderer2 = null;
        return defaultValue;
      };
      exports.createElement = function(type, config, children) {
        for (var i = 2;i < arguments.length; i++)
          validateChildKeys(arguments[i]);
        i = {};
        var key = null;
        if (config != null)
          for (propName in didWarnAboutOldJSXRuntime || !("__self" in config) || "key" in config || (didWarnAboutOldJSXRuntime = true, console.warn("Your app (or one of its dependencies) is using an outdated JSX transform. Update to the modern JSX transform for faster performance: https://react.dev/link/new-jsx-transform")), hasValidKey(config) && (checkKeyStringCoercion(config.key), key = "" + config.key), config)
            hasOwnProperty.call(config, propName) && propName !== "key" && propName !== "__self" && propName !== "__source" && (i[propName] = config[propName]);
        var childrenLength = arguments.length - 2;
        if (childrenLength === 1)
          i.children = children;
        else if (1 < childrenLength) {
          for (var childArray = Array(childrenLength), _i = 0;_i < childrenLength; _i++)
            childArray[_i] = arguments[_i + 2];
          Object.freeze && Object.freeze(childArray);
          i.children = childArray;
        }
        if (type && type.defaultProps)
          for (propName in childrenLength = type.defaultProps, childrenLength)
            i[propName] === undefined && (i[propName] = childrenLength[propName]);
        key && defineKeyPropWarningGetter(i, typeof type === "function" ? type.displayName || type.name || "Unknown" : type);
        var propName = 1e4 > ReactSharedInternals.recentlyCreatedOwnerStacks++;
        return ReactElement(type, key, i, getOwner(), propName ? Error("react-stack-top-frame") : unknownOwnerDebugStack, propName ? createTask(getTaskName(type)) : unknownOwnerDebugTask);
      };
      exports.createRef = function() {
        var refObject = { current: null };
        Object.seal(refObject);
        return refObject;
      };
      exports.forwardRef = function(render) {
        render != null && render.$$typeof === REACT_MEMO_TYPE ? console.error("forwardRef requires a render function but received a `memo` component. Instead of forwardRef(memo(...)), use memo(forwardRef(...)).") : typeof render !== "function" ? console.error("forwardRef requires a render function but was given %s.", render === null ? "null" : typeof render) : render.length !== 0 && render.length !== 2 && console.error("forwardRef render functions accept exactly two parameters: props and ref. %s", render.length === 1 ? "Did you forget to use the ref parameter?" : "Any additional parameter will be undefined.");
        render != null && render.defaultProps != null && console.error("forwardRef render functions do not support defaultProps. Did you accidentally pass a React component?");
        var elementType = { $$typeof: REACT_FORWARD_REF_TYPE, render }, ownName;
        Object.defineProperty(elementType, "displayName", {
          enumerable: false,
          configurable: true,
          get: function() {
            return ownName;
          },
          set: function(name) {
            ownName = name;
            render.name || render.displayName || (Object.defineProperty(render, "name", { value: name }), render.displayName = name);
          }
        });
        return elementType;
      };
      exports.isValidElement = isValidElement;
      exports.lazy = function(ctor) {
        ctor = { _status: -1, _result: ctor };
        var lazyType = {
          $$typeof: REACT_LAZY_TYPE,
          _payload: ctor,
          _init: lazyInitializer
        }, ioInfo = {
          name: "lazy",
          start: -1,
          end: -1,
          value: null,
          owner: null,
          debugStack: Error("react-stack-top-frame"),
          debugTask: console.createTask ? console.createTask("lazy()") : null
        };
        ctor._ioInfo = ioInfo;
        lazyType._debugInfo = [{ awaited: ioInfo }];
        return lazyType;
      };
      exports.memo = function(type, compare) {
        type == null && console.error("memo: The first argument must be a component. Instead received: %s", type === null ? "null" : typeof type);
        compare = {
          $$typeof: REACT_MEMO_TYPE,
          type,
          compare: compare === undefined ? null : compare
        };
        var ownName;
        Object.defineProperty(compare, "displayName", {
          enumerable: false,
          configurable: true,
          get: function() {
            return ownName;
          },
          set: function(name) {
            ownName = name;
            type.name || type.displayName || (Object.defineProperty(type, "name", { value: name }), type.displayName = name);
          }
        });
        return compare;
      };
      exports.startTransition = function(scope) {
        var prevTransition = ReactSharedInternals.T, currentTransition = {};
        currentTransition._updatedFibers = new Set;
        ReactSharedInternals.T = currentTransition;
        try {
          var returnValue = scope(), onStartTransitionFinish = ReactSharedInternals.S;
          onStartTransitionFinish !== null && onStartTransitionFinish(currentTransition, returnValue);
          typeof returnValue === "object" && returnValue !== null && typeof returnValue.then === "function" && (ReactSharedInternals.asyncTransitions++, returnValue.then(releaseAsyncTransition, releaseAsyncTransition), returnValue.then(noop2, reportGlobalError));
        } catch (error) {
          reportGlobalError(error);
        } finally {
          prevTransition === null && currentTransition._updatedFibers && (scope = currentTransition._updatedFibers.size, currentTransition._updatedFibers.clear(), 10 < scope && console.warn("Detected a large number of updates inside startTransition. If this is due to a subscription please re-write it to use React provided hooks. Otherwise concurrent mode guarantees are off the table.")), prevTransition !== null && currentTransition.types !== null && (prevTransition.types !== null && prevTransition.types !== currentTransition.types && console.error("We expected inner Transitions to have transferred the outer types set and that you cannot add to the outer Transition while inside the inner.This is a bug in React."), prevTransition.types = currentTransition.types), ReactSharedInternals.T = prevTransition;
        }
      };
      exports.unstable_useCacheRefresh = function() {
        return resolveDispatcher().useCacheRefresh();
      };
      exports.use = function(usable) {
        return resolveDispatcher().use(usable);
      };
      exports.useActionState = function(action, initialState, permalink) {
        return resolveDispatcher().useActionState(action, initialState, permalink);
      };
      exports.useCallback = function(callback, deps) {
        return resolveDispatcher().useCallback(callback, deps);
      };
      exports.useContext = function(Context) {
        var dispatcher = resolveDispatcher();
        Context.$$typeof === REACT_CONSUMER_TYPE && console.error("Calling useContext(Context.Consumer) is not supported and will cause bugs. Did you mean to call useContext(Context) instead?");
        return dispatcher.useContext(Context);
      };
      exports.useDebugValue = function(value, formatterFn) {
        return resolveDispatcher().useDebugValue(value, formatterFn);
      };
      exports.useDeferredValue = function(value, initialValue) {
        return resolveDispatcher().useDeferredValue(value, initialValue);
      };
      exports.useEffect = function(create, deps) {
        create == null && console.warn("React Hook useEffect requires an effect callback. Did you forget to pass a callback to the hook?");
        return resolveDispatcher().useEffect(create, deps);
      };
      exports.useEffectEvent = function(callback) {
        return resolveDispatcher().useEffectEvent(callback);
      };
      exports.useId = function() {
        return resolveDispatcher().useId();
      };
      exports.useImperativeHandle = function(ref, create, deps) {
        return resolveDispatcher().useImperativeHandle(ref, create, deps);
      };
      exports.useInsertionEffect = function(create, deps) {
        create == null && console.warn("React Hook useInsertionEffect requires an effect callback. Did you forget to pass a callback to the hook?");
        return resolveDispatcher().useInsertionEffect(create, deps);
      };
      exports.useLayoutEffect = function(create, deps) {
        create == null && console.warn("React Hook useLayoutEffect requires an effect callback. Did you forget to pass a callback to the hook?");
        return resolveDispatcher().useLayoutEffect(create, deps);
      };
      exports.useMemo = function(create, deps) {
        return resolveDispatcher().useMemo(create, deps);
      };
      exports.useOptimistic = function(passthrough, reducer) {
        return resolveDispatcher().useOptimistic(passthrough, reducer);
      };
      exports.useReducer = function(reducer, initialArg, init) {
        return resolveDispatcher().useReducer(reducer, initialArg, init);
      };
      exports.useRef = function(initialValue) {
        return resolveDispatcher().useRef(initialValue);
      };
      exports.useState = function(initialState) {
        return resolveDispatcher().useState(initialState);
      };
      exports.useSyncExternalStore = function(subscribe, getSnapshot, getServerSnapshot) {
        return resolveDispatcher().useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
      };
      exports.useTransition = function() {
        return resolveDispatcher().useTransition();
      };
      exports.version = "19.2.7";
      typeof __REACT_DEVTOOLS_GLOBAL_HOOK__ !== "undefined" && typeof __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStop === "function" && __REACT_DEVTOOLS_GLOBAL_HOOK__.registerInternalModuleStop(Error());
    })();
  });

  // ../../node_modules/.bun/react@19.2.7/node_modules/react/index.js
  var require_react = __commonJS((exports, module) => {
    if (false) {} else {
      module.exports = require_react_development();
    }
  });

  // ../../node_modules/.bun/react@19.2.7/node_modules/react/cjs/react-jsx-runtime.development.js
  var require_react_jsx_runtime_development = __commonJS((exports) => {
    (function() {
      function getComponentNameFromType(type) {
        if (type == null)
          return null;
        if (typeof type === "function")
          return type.$$typeof === REACT_CLIENT_REFERENCE ? null : type.displayName || type.name || null;
        if (typeof type === "string")
          return type;
        switch (type) {
          case REACT_FRAGMENT_TYPE:
            return "Fragment";
          case REACT_PROFILER_TYPE:
            return "Profiler";
          case REACT_STRICT_MODE_TYPE:
            return "StrictMode";
          case REACT_SUSPENSE_TYPE:
            return "Suspense";
          case REACT_SUSPENSE_LIST_TYPE:
            return "SuspenseList";
          case REACT_ACTIVITY_TYPE:
            return "Activity";
        }
        if (typeof type === "object")
          switch (typeof type.tag === "number" && console.error("Received an unexpected object in getComponentNameFromType(). This is likely a bug in React. Please file an issue."), type.$$typeof) {
            case REACT_PORTAL_TYPE:
              return "Portal";
            case REACT_CONTEXT_TYPE:
              return type.displayName || "Context";
            case REACT_CONSUMER_TYPE:
              return (type._context.displayName || "Context") + ".Consumer";
            case REACT_FORWARD_REF_TYPE:
              var innerType = type.render;
              type = type.displayName;
              type || (type = innerType.displayName || innerType.name || "", type = type !== "" ? "ForwardRef(" + type + ")" : "ForwardRef");
              return type;
            case REACT_MEMO_TYPE:
              return innerType = type.displayName || null, innerType !== null ? innerType : getComponentNameFromType(type.type) || "Memo";
            case REACT_LAZY_TYPE:
              innerType = type._payload;
              type = type._init;
              try {
                return getComponentNameFromType(type(innerType));
              } catch (x) {}
          }
        return null;
      }
      function testStringCoercion(value) {
        return "" + value;
      }
      function checkKeyStringCoercion(value) {
        try {
          testStringCoercion(value);
          var JSCompiler_inline_result = false;
        } catch (e) {
          JSCompiler_inline_result = true;
        }
        if (JSCompiler_inline_result) {
          JSCompiler_inline_result = console;
          var JSCompiler_temp_const = JSCompiler_inline_result.error;
          var JSCompiler_inline_result$jscomp$0 = typeof Symbol === "function" && Symbol.toStringTag && value[Symbol.toStringTag] || value.constructor.name || "Object";
          JSCompiler_temp_const.call(JSCompiler_inline_result, "The provided key is an unsupported type %s. This value must be coerced to a string before using it here.", JSCompiler_inline_result$jscomp$0);
          return testStringCoercion(value);
        }
      }
      function getTaskName(type) {
        if (type === REACT_FRAGMENT_TYPE)
          return "<>";
        if (typeof type === "object" && type !== null && type.$$typeof === REACT_LAZY_TYPE)
          return "<...>";
        try {
          var name = getComponentNameFromType(type);
          return name ? "<" + name + ">" : "<...>";
        } catch (x) {
          return "<...>";
        }
      }
      function getOwner() {
        var dispatcher = ReactSharedInternals.A;
        return dispatcher === null ? null : dispatcher.getOwner();
      }
      function UnknownOwner() {
        return Error("react-stack-top-frame");
      }
      function hasValidKey(config) {
        if (hasOwnProperty.call(config, "key")) {
          var getter = Object.getOwnPropertyDescriptor(config, "key").get;
          if (getter && getter.isReactWarning)
            return false;
        }
        return config.key !== undefined;
      }
      function defineKeyPropWarningGetter(props, displayName) {
        function warnAboutAccessingKey() {
          specialPropKeyWarningShown || (specialPropKeyWarningShown = true, console.error("%s: `key` is not a prop. Trying to access it will result in `undefined` being returned. If you need to access the same value within the child component, you should pass it as a different prop. (https://react.dev/link/special-props)", displayName));
        }
        warnAboutAccessingKey.isReactWarning = true;
        Object.defineProperty(props, "key", {
          get: warnAboutAccessingKey,
          configurable: true
        });
      }
      function elementRefGetterWithDeprecationWarning() {
        var componentName = getComponentNameFromType(this.type);
        didWarnAboutElementRef[componentName] || (didWarnAboutElementRef[componentName] = true, console.error("Accessing element.ref was removed in React 19. ref is now a regular prop. It will be removed from the JSX Element type in a future release."));
        componentName = this.props.ref;
        return componentName !== undefined ? componentName : null;
      }
      function ReactElement(type, key, props, owner, debugStack, debugTask) {
        var refProp = props.ref;
        type = {
          $$typeof: REACT_ELEMENT_TYPE,
          type,
          key,
          props,
          _owner: owner
        };
        (refProp !== undefined ? refProp : null) !== null ? Object.defineProperty(type, "ref", {
          enumerable: false,
          get: elementRefGetterWithDeprecationWarning
        }) : Object.defineProperty(type, "ref", { enumerable: false, value: null });
        type._store = {};
        Object.defineProperty(type._store, "validated", {
          configurable: false,
          enumerable: false,
          writable: true,
          value: 0
        });
        Object.defineProperty(type, "_debugInfo", {
          configurable: false,
          enumerable: false,
          writable: true,
          value: null
        });
        Object.defineProperty(type, "_debugStack", {
          configurable: false,
          enumerable: false,
          writable: true,
          value: debugStack
        });
        Object.defineProperty(type, "_debugTask", {
          configurable: false,
          enumerable: false,
          writable: true,
          value: debugTask
        });
        Object.freeze && (Object.freeze(type.props), Object.freeze(type));
        return type;
      }
      function jsxDEVImpl(type, config, maybeKey, isStaticChildren, debugStack, debugTask) {
        var children = config.children;
        if (children !== undefined)
          if (isStaticChildren)
            if (isArrayImpl(children)) {
              for (isStaticChildren = 0;isStaticChildren < children.length; isStaticChildren++)
                validateChildKeys(children[isStaticChildren]);
              Object.freeze && Object.freeze(children);
            } else
              console.error("React.jsx: Static children should always be an array. You are likely explicitly calling React.jsxs or React.jsxDEV. Use the Babel transform instead.");
          else
            validateChildKeys(children);
        if (hasOwnProperty.call(config, "key")) {
          children = getComponentNameFromType(type);
          var keys = Object.keys(config).filter(function(k) {
            return k !== "key";
          });
          isStaticChildren = 0 < keys.length ? "{key: someKey, " + keys.join(": ..., ") + ": ...}" : "{key: someKey}";
          didWarnAboutKeySpread[children + isStaticChildren] || (keys = 0 < keys.length ? "{" + keys.join(": ..., ") + ": ...}" : "{}", console.error(`A props object containing a "key" prop is being spread into JSX:
  let props = %s;
  <%s {...props} />
React keys must be passed directly to JSX without using spread:
  let props = %s;
  <%s key={someKey} {...props} />`, isStaticChildren, children, keys, children), didWarnAboutKeySpread[children + isStaticChildren] = true);
        }
        children = null;
        maybeKey !== undefined && (checkKeyStringCoercion(maybeKey), children = "" + maybeKey);
        hasValidKey(config) && (checkKeyStringCoercion(config.key), children = "" + config.key);
        if ("key" in config) {
          maybeKey = {};
          for (var propName in config)
            propName !== "key" && (maybeKey[propName] = config[propName]);
        } else
          maybeKey = config;
        children && defineKeyPropWarningGetter(maybeKey, typeof type === "function" ? type.displayName || type.name || "Unknown" : type);
        return ReactElement(type, children, maybeKey, getOwner(), debugStack, debugTask);
      }
      function validateChildKeys(node) {
        isValidElement(node) ? node._store && (node._store.validated = 1) : typeof node === "object" && node !== null && node.$$typeof === REACT_LAZY_TYPE && (node._payload.status === "fulfilled" ? isValidElement(node._payload.value) && node._payload.value._store && (node._payload.value._store.validated = 1) : node._store && (node._store.validated = 1));
      }
      function isValidElement(object) {
        return typeof object === "object" && object !== null && object.$$typeof === REACT_ELEMENT_TYPE;
      }
      var React = require_react(), REACT_ELEMENT_TYPE = Symbol.for("react.transitional.element"), REACT_PORTAL_TYPE = Symbol.for("react.portal"), REACT_FRAGMENT_TYPE = Symbol.for("react.fragment"), REACT_STRICT_MODE_TYPE = Symbol.for("react.strict_mode"), REACT_PROFILER_TYPE = Symbol.for("react.profiler"), REACT_CONSUMER_TYPE = Symbol.for("react.consumer"), REACT_CONTEXT_TYPE = Symbol.for("react.context"), REACT_FORWARD_REF_TYPE = Symbol.for("react.forward_ref"), REACT_SUSPENSE_TYPE = Symbol.for("react.suspense"), REACT_SUSPENSE_LIST_TYPE = Symbol.for("react.suspense_list"), REACT_MEMO_TYPE = Symbol.for("react.memo"), REACT_LAZY_TYPE = Symbol.for("react.lazy"), REACT_ACTIVITY_TYPE = Symbol.for("react.activity"), REACT_CLIENT_REFERENCE = Symbol.for("react.client.reference"), ReactSharedInternals = React.__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE, hasOwnProperty = Object.prototype.hasOwnProperty, isArrayImpl = Array.isArray, createTask = console.createTask ? console.createTask : function() {
        return null;
      };
      React = {
        react_stack_bottom_frame: function(callStackForError) {
          return callStackForError();
        }
      };
      var specialPropKeyWarningShown;
      var didWarnAboutElementRef = {};
      var unknownOwnerDebugStack = React.react_stack_bottom_frame.bind(React, UnknownOwner)();
      var unknownOwnerDebugTask = createTask(getTaskName(UnknownOwner));
      var didWarnAboutKeySpread = {};
      exports.Fragment = REACT_FRAGMENT_TYPE;
      exports.jsx = function(type, config, maybeKey) {
        var trackActualOwner = 1e4 > ReactSharedInternals.recentlyCreatedOwnerStacks++;
        return jsxDEVImpl(type, config, maybeKey, false, trackActualOwner ? Error("react-stack-top-frame") : unknownOwnerDebugStack, trackActualOwner ? createTask(getTaskName(type)) : unknownOwnerDebugTask);
      };
      exports.jsxs = function(type, config, maybeKey) {
        var trackActualOwner = 1e4 > ReactSharedInternals.recentlyCreatedOwnerStacks++;
        return jsxDEVImpl(type, config, maybeKey, true, trackActualOwner ? Error("react-stack-top-frame") : unknownOwnerDebugStack, trackActualOwner ? createTask(getTaskName(type)) : unknownOwnerDebugTask);
      };
    })();
  });

  // ../../node_modules/.bun/react@19.2.7/node_modules/react/jsx-runtime.js
  var require_jsx_runtime = __commonJS((exports, module) => {
    if (false) {} else {
      module.exports = require_react_jsx_runtime_development();
    }
  });

  // src/index.ts
  var exports_src = {};
  __export(exports_src, {
    unmountPluginUI: () => unmountPluginUI,
    requiredSlabApiPermission: () => requiredSlabApiPermission,
    mountPluginUI: () => mountPluginUI,
    isKnownSlabApiPermission: () => isKnownSlabApiPermission,
    getSlabPluginSdk: () => getSlabPluginSdk,
    describeSlabApiPermission: () => describeSlabApiPermission,
    createSlabPluginSdk: () => createSlabPluginSdk,
    applySlabThemeToDocument: () => applySlabThemeToDocument,
    SlabPluginApiError: () => SlabPluginApiError,
    SLAB_THEME_TOKENS: () => SLAB_THEME_TOKENS,
    SLAB_API_PERMISSION_LABELS: () => SLAB_API_PERMISSION_LABELS,
    SLAB_API_PERMISSIONS: () => SLAB_API_PERMISSIONS
  });

  // ../../node_modules/.bun/openapi-fetch@0.17.0/node_modules/openapi-fetch/dist/index.mjs
  var PATH_PARAM_RE = /\{[^{}]+\}/g;
  var supportsRequestInitExt = () => {
    return typeof process === "object" && Number.parseInt(process?.versions?.node?.substring(0, 2)) >= 18 && process.versions.undici;
  };
  function randomID() {
    return Math.random().toString(36).slice(2, 11);
  }
  function createClient(clientOptions) {
    let {
      baseUrl = "",
      Request: CustomRequest = globalThis.Request,
      fetch: baseFetch = globalThis.fetch,
      querySerializer: globalQuerySerializer,
      bodySerializer: globalBodySerializer,
      pathSerializer: globalPathSerializer,
      headers: baseHeaders,
      requestInitExt = undefined,
      ...baseOptions
    } = { ...clientOptions };
    requestInitExt = supportsRequestInitExt() ? requestInitExt : undefined;
    baseUrl = removeTrailingSlash(baseUrl);
    const globalMiddlewares = [];
    async function coreFetch(schemaPath, fetchOptions) {
      const {
        baseUrl: localBaseUrl,
        fetch: fetch2 = baseFetch,
        Request = CustomRequest,
        headers,
        params = {},
        parseAs = "json",
        querySerializer: requestQuerySerializer,
        bodySerializer = globalBodySerializer ?? defaultBodySerializer,
        pathSerializer: requestPathSerializer,
        body,
        middleware: requestMiddlewares = [],
        ...init
      } = fetchOptions || {};
      let finalBaseUrl = baseUrl;
      if (localBaseUrl) {
        finalBaseUrl = removeTrailingSlash(localBaseUrl) ?? baseUrl;
      }
      let querySerializer = typeof globalQuerySerializer === "function" ? globalQuerySerializer : createQuerySerializer(globalQuerySerializer);
      if (requestQuerySerializer) {
        querySerializer = typeof requestQuerySerializer === "function" ? requestQuerySerializer : createQuerySerializer({
          ...typeof globalQuerySerializer === "object" ? globalQuerySerializer : {},
          ...requestQuerySerializer
        });
      }
      const pathSerializer = requestPathSerializer || globalPathSerializer || defaultPathSerializer;
      const serializedBody = body === undefined ? undefined : bodySerializer(body, mergeHeaders(baseHeaders, headers, params.header));
      const finalHeaders = mergeHeaders(serializedBody === undefined || serializedBody instanceof FormData ? {} : {
        "Content-Type": "application/json"
      }, baseHeaders, headers, params.header);
      const finalMiddlewares = [...globalMiddlewares, ...requestMiddlewares];
      const requestInit = {
        redirect: "follow",
        ...baseOptions,
        ...init,
        body: serializedBody,
        headers: finalHeaders
      };
      let id;
      let options;
      let request = new Request(createFinalURL(schemaPath, { baseUrl: finalBaseUrl, params, querySerializer, pathSerializer }), requestInit);
      let response;
      for (const key in init) {
        if (!(key in request)) {
          request[key] = init[key];
        }
      }
      if (finalMiddlewares.length) {
        id = randomID();
        options = Object.freeze({
          baseUrl: finalBaseUrl,
          fetch: fetch2,
          parseAs,
          querySerializer,
          bodySerializer,
          pathSerializer
        });
        for (const m of finalMiddlewares) {
          if (m && typeof m === "object" && typeof m.onRequest === "function") {
            const result = await m.onRequest({
              request,
              schemaPath,
              params,
              options,
              id
            });
            if (result) {
              if (result instanceof Request) {
                request = result;
              } else if (result instanceof Response) {
                response = result;
                break;
              } else {
                throw new Error("onRequest: must return new Request() or Response() when modifying the request");
              }
            }
          }
        }
      }
      if (!response) {
        try {
          response = await fetch2(request, requestInitExt);
        } catch (error2) {
          let errorAfterMiddleware = error2;
          if (finalMiddlewares.length) {
            for (let i = finalMiddlewares.length - 1;i >= 0; i--) {
              const m = finalMiddlewares[i];
              if (m && typeof m === "object" && typeof m.onError === "function") {
                const result = await m.onError({
                  request,
                  error: errorAfterMiddleware,
                  schemaPath,
                  params,
                  options,
                  id
                });
                if (result) {
                  if (result instanceof Response) {
                    errorAfterMiddleware = undefined;
                    response = result;
                    break;
                  }
                  if (result instanceof Error) {
                    errorAfterMiddleware = result;
                    continue;
                  }
                  throw new Error("onError: must return new Response() or instance of Error");
                }
              }
            }
          }
          if (errorAfterMiddleware) {
            throw errorAfterMiddleware;
          }
        }
        if (finalMiddlewares.length) {
          for (let i = finalMiddlewares.length - 1;i >= 0; i--) {
            const m = finalMiddlewares[i];
            if (m && typeof m === "object" && typeof m.onResponse === "function") {
              const result = await m.onResponse({
                request,
                response,
                schemaPath,
                params,
                options,
                id
              });
              if (result) {
                if (!(result instanceof Response)) {
                  throw new Error("onResponse: must return new Response() when modifying the response");
                }
                response = result;
              }
            }
          }
        }
      }
      const contentLength = response.headers.get("Content-Length");
      if (response.status === 204 || request.method === "HEAD" || contentLength === "0" && !response.headers.get("Transfer-Encoding")?.includes("chunked")) {
        return response.ok ? { data: undefined, response } : { error: undefined, response };
      }
      if (response.ok) {
        const getResponseData = async () => {
          if (parseAs === "stream") {
            return response.body;
          }
          if (parseAs === "json" && !contentLength) {
            const raw = await response.text();
            return raw ? JSON.parse(raw) : undefined;
          }
          return await response[parseAs]();
        };
        return { data: await getResponseData(), response };
      }
      let error = await response.text();
      try {
        error = JSON.parse(error);
      } catch {}
      return { error, response };
    }
    return {
      request(method, url, init) {
        return coreFetch(url, { ...init, method: method.toUpperCase() });
      },
      GET(url, init) {
        return coreFetch(url, { ...init, method: "GET" });
      },
      PUT(url, init) {
        return coreFetch(url, { ...init, method: "PUT" });
      },
      POST(url, init) {
        return coreFetch(url, { ...init, method: "POST" });
      },
      DELETE(url, init) {
        return coreFetch(url, { ...init, method: "DELETE" });
      },
      OPTIONS(url, init) {
        return coreFetch(url, { ...init, method: "OPTIONS" });
      },
      HEAD(url, init) {
        return coreFetch(url, { ...init, method: "HEAD" });
      },
      PATCH(url, init) {
        return coreFetch(url, { ...init, method: "PATCH" });
      },
      TRACE(url, init) {
        return coreFetch(url, { ...init, method: "TRACE" });
      },
      use(...middleware) {
        for (const m of middleware) {
          if (!m) {
            continue;
          }
          if (typeof m !== "object" || !(("onRequest" in m) || ("onResponse" in m) || ("onError" in m))) {
            throw new Error("Middleware must be an object with one of `onRequest()`, `onResponse() or `onError()`");
          }
          globalMiddlewares.push(m);
        }
      },
      eject(...middleware) {
        for (const m of middleware) {
          const i = globalMiddlewares.indexOf(m);
          if (i !== -1) {
            globalMiddlewares.splice(i, 1);
          }
        }
      }
    };
  }
  function serializePrimitiveParam(name, value, options) {
    if (value === undefined || value === null) {
      return "";
    }
    if (typeof value === "object") {
      throw new Error("Deeply-nested arrays/objects aren’t supported. Provide your own `querySerializer()` to handle these.");
    }
    return `${name}=${options?.allowReserved === true ? value : encodeURIComponent(value)}`;
  }
  function serializeObjectParam(name, value, options) {
    if (!value || typeof value !== "object") {
      return "";
    }
    const values = [];
    const joiner = {
      simple: ",",
      label: ".",
      matrix: ";"
    }[options.style] || "&";
    if (options.style !== "deepObject" && options.explode === false) {
      for (const k in value) {
        values.push(k, options.allowReserved === true ? value[k] : encodeURIComponent(value[k]));
      }
      const final2 = values.join(",");
      switch (options.style) {
        case "form": {
          return `${name}=${final2}`;
        }
        case "label": {
          return `.${final2}`;
        }
        case "matrix": {
          return `;${name}=${final2}`;
        }
        default: {
          return final2;
        }
      }
    }
    for (const k in value) {
      const finalName = options.style === "deepObject" ? `${name}[${k}]` : k;
      values.push(serializePrimitiveParam(finalName, value[k], options));
    }
    const final = values.join(joiner);
    return options.style === "label" || options.style === "matrix" ? `${joiner}${final}` : final;
  }
  function serializeArrayParam(name, value, options) {
    if (!Array.isArray(value)) {
      return "";
    }
    if (options.explode === false) {
      const joiner2 = { form: ",", spaceDelimited: "%20", pipeDelimited: "|" }[options.style] || ",";
      const final = (options.allowReserved === true ? value : value.map((v) => encodeURIComponent(v))).join(joiner2);
      switch (options.style) {
        case "simple": {
          return final;
        }
        case "label": {
          return `.${final}`;
        }
        case "matrix": {
          return `;${name}=${final}`;
        }
        default: {
          return `${name}=${final}`;
        }
      }
    }
    const joiner = { simple: ",", label: ".", matrix: ";" }[options.style] || "&";
    const values = [];
    for (const v of value) {
      if (options.style === "simple" || options.style === "label") {
        values.push(options.allowReserved === true ? v : encodeURIComponent(v));
      } else {
        values.push(serializePrimitiveParam(name, v, options));
      }
    }
    return options.style === "label" || options.style === "matrix" ? `${joiner}${values.join(joiner)}` : values.join(joiner);
  }
  function createQuerySerializer(options) {
    return function querySerializer(queryParams) {
      const search = [];
      if (queryParams && typeof queryParams === "object") {
        for (const name in queryParams) {
          const value = queryParams[name];
          if (value === undefined || value === null) {
            continue;
          }
          if (Array.isArray(value)) {
            if (value.length === 0) {
              continue;
            }
            search.push(serializeArrayParam(name, value, {
              style: "form",
              explode: true,
              ...options?.array,
              allowReserved: options?.allowReserved || false
            }));
            continue;
          }
          if (typeof value === "object") {
            search.push(serializeObjectParam(name, value, {
              style: "deepObject",
              explode: true,
              ...options?.object,
              allowReserved: options?.allowReserved || false
            }));
            continue;
          }
          search.push(serializePrimitiveParam(name, value, options));
        }
      }
      return search.join("&");
    };
  }
  function defaultPathSerializer(pathname, pathParams) {
    let nextURL = pathname;
    for (const match of pathname.match(PATH_PARAM_RE) ?? []) {
      let name = match.substring(1, match.length - 1);
      let explode = false;
      let style = "simple";
      if (name.endsWith("*")) {
        explode = true;
        name = name.substring(0, name.length - 1);
      }
      if (name.startsWith(".")) {
        style = "label";
        name = name.substring(1);
      } else if (name.startsWith(";")) {
        style = "matrix";
        name = name.substring(1);
      }
      if (!pathParams || pathParams[name] === undefined || pathParams[name] === null) {
        continue;
      }
      const value = pathParams[name];
      if (Array.isArray(value)) {
        nextURL = nextURL.replace(match, serializeArrayParam(name, value, { style, explode }));
        continue;
      }
      if (typeof value === "object") {
        nextURL = nextURL.replace(match, serializeObjectParam(name, value, { style, explode }));
        continue;
      }
      if (style === "matrix") {
        nextURL = nextURL.replace(match, `;${serializePrimitiveParam(name, value)}`);
        continue;
      }
      nextURL = nextURL.replace(match, style === "label" ? `.${encodeURIComponent(value)}` : encodeURIComponent(value));
    }
    return nextURL;
  }
  function defaultBodySerializer(body, headers) {
    if (body instanceof FormData) {
      return body;
    }
    if (headers) {
      const contentType = headers.get instanceof Function ? headers.get("Content-Type") ?? headers.get("content-type") : headers["Content-Type"] ?? headers["content-type"];
      if (contentType === "application/x-www-form-urlencoded") {
        return new URLSearchParams(body).toString();
      }
    }
    return JSON.stringify(body);
  }
  function createFinalURL(pathname, options) {
    let finalURL = `${options.baseUrl}${pathname}`;
    if (options.params?.path) {
      finalURL = options.pathSerializer(finalURL, options.params.path);
    }
    let search = options.querySerializer(options.params.query ?? {});
    if (search.startsWith("?")) {
      search = search.substring(1);
    }
    if (search) {
      finalURL += `?${search}`;
    }
    return finalURL;
  }
  function mergeHeaders(...allHeaders) {
    const finalHeaders = new Headers;
    for (const h of allHeaders) {
      if (!h || typeof h !== "object") {
        continue;
      }
      const iterator = h instanceof Headers ? h.entries() : Object.entries(h);
      for (const [k, v] of iterator) {
        if (v === null) {
          finalHeaders.delete(k);
        } else if (Array.isArray(v)) {
          for (const v2 of v) {
            finalHeaders.append(k, v2);
          }
        } else if (v !== undefined) {
          finalHeaders.set(k, v);
        }
      }
    }
    return finalHeaders;
  }
  function removeTrailingSlash(url) {
    if (url.endsWith("/")) {
      return url.substring(0, url.length - 1);
    }
    return url;
  }

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/subscribable.js
  var Subscribable = class {
    constructor() {
      this.listeners = /* @__PURE__ */ new Set;
      this.subscribe = this.subscribe.bind(this);
    }
    subscribe(listener) {
      this.listeners.add(listener);
      this.onSubscribe();
      return () => {
        this.listeners.delete(listener);
        this.onUnsubscribe();
      };
    }
    hasListeners() {
      return this.listeners.size > 0;
    }
    onSubscribe() {}
    onUnsubscribe() {}
  };

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/focusManager.js
  var FocusManager = class extends Subscribable {
    #focused;
    #cleanup;
    #setup;
    constructor() {
      super();
      this.#setup = (onFocus) => {
        if (typeof window !== "undefined" && window.addEventListener) {
          const listener = () => onFocus();
          window.addEventListener("visibilitychange", listener, false);
          return () => {
            window.removeEventListener("visibilitychange", listener);
          };
        }
        return;
      };
    }
    onSubscribe() {
      if (!this.#cleanup) {
        this.setEventListener(this.#setup);
      }
    }
    onUnsubscribe() {
      if (!this.hasListeners()) {
        this.#cleanup?.();
        this.#cleanup = undefined;
      }
    }
    setEventListener(setup) {
      this.#setup = setup;
      this.#cleanup?.();
      this.#cleanup = setup((focused) => {
        if (typeof focused === "boolean") {
          this.setFocused(focused);
        } else {
          this.onFocus();
        }
      });
    }
    setFocused(focused) {
      const changed = this.#focused !== focused;
      if (changed) {
        this.#focused = focused;
        this.onFocus();
      }
    }
    onFocus() {
      const isFocused = this.isFocused();
      this.listeners.forEach((listener) => {
        listener(isFocused);
      });
    }
    isFocused() {
      if (typeof this.#focused === "boolean") {
        return this.#focused;
      }
      return globalThis.document?.visibilityState !== "hidden";
    }
  };
  var focusManager = new FocusManager;

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/timeoutManager.js
  var defaultTimeoutProvider = {
    setTimeout: (callback, delay) => setTimeout(callback, delay),
    clearTimeout: (timeoutId) => clearTimeout(timeoutId),
    setInterval: (callback, delay) => setInterval(callback, delay),
    clearInterval: (intervalId) => clearInterval(intervalId)
  };
  var TimeoutManager = class {
    #provider = defaultTimeoutProvider;
    #providerCalled = false;
    setTimeoutProvider(provider) {
      if (true) {
        if (this.#providerCalled && provider !== this.#provider) {
          console.error(`[timeoutManager]: Switching provider after calls to previous provider might result in unexpected behavior.`, { previous: this.#provider, provider });
        }
      }
      this.#provider = provider;
      if (true) {
        this.#providerCalled = false;
      }
    }
    setTimeout(callback, delay) {
      if (true) {
        this.#providerCalled = true;
      }
      return this.#provider.setTimeout(callback, delay);
    }
    clearTimeout(timeoutId) {
      this.#provider.clearTimeout(timeoutId);
    }
    setInterval(callback, delay) {
      if (true) {
        this.#providerCalled = true;
      }
      return this.#provider.setInterval(callback, delay);
    }
    clearInterval(intervalId) {
      this.#provider.clearInterval(intervalId);
    }
  };
  var timeoutManager = new TimeoutManager;
  function systemSetTimeoutZero(callback) {
    setTimeout(callback, 0);
  }

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/utils.js
  var isServer = typeof window === "undefined" || "Deno" in globalThis;
  function noop() {}
  function isValidTimeout(value) {
    return typeof value === "number" && value >= 0 && value !== Infinity;
  }
  function timeUntilStale(updatedAt, staleTime) {
    return Math.max(updatedAt + (staleTime || 0) - Date.now(), 0);
  }
  function resolveStaleTime(staleTime, query) {
    return typeof staleTime === "function" ? staleTime(query) : staleTime;
  }
  function resolveQueryBoolean(option, query) {
    return typeof option === "function" ? option(query) : option;
  }
  function hashKey(queryKey) {
    return JSON.stringify(queryKey, (_, val) => isPlainObject(val) ? Object.keys(val).sort().reduce((result, key) => {
      result[key] = val[key];
      return result;
    }, {}) : val);
  }
  var hasOwn = Object.prototype.hasOwnProperty;
  function replaceEqualDeep(a, b, depth = 0) {
    if (a === b) {
      return a;
    }
    if (depth > 500)
      return b;
    const array = isPlainArray(a) && isPlainArray(b);
    if (!array && !(isPlainObject(a) && isPlainObject(b)))
      return b;
    const aItems = array ? a : Object.keys(a);
    const aSize = aItems.length;
    const bItems = array ? b : Object.keys(b);
    const bSize = bItems.length;
    const copy = array ? new Array(bSize) : {};
    let equalItems = 0;
    for (let i = 0;i < bSize; i++) {
      const key = array ? i : bItems[i];
      const aItem = a[key];
      const bItem = b[key];
      if (aItem === bItem) {
        copy[key] = aItem;
        if (array ? i < aSize : hasOwn.call(a, key))
          equalItems++;
        continue;
      }
      if (aItem === null || bItem === null || typeof aItem !== "object" || typeof bItem !== "object") {
        copy[key] = bItem;
        continue;
      }
      const v = replaceEqualDeep(aItem, bItem, depth + 1);
      copy[key] = v;
      if (v === aItem)
        equalItems++;
    }
    return aSize === bSize && equalItems === aSize ? a : copy;
  }
  function shallowEqualObjects(a, b) {
    if (!b || Object.keys(a).length !== Object.keys(b).length) {
      return false;
    }
    for (const key in a) {
      if (a[key] !== b[key]) {
        return false;
      }
    }
    return true;
  }
  function isPlainArray(value) {
    return Array.isArray(value) && value.length === Object.keys(value).length;
  }
  function isPlainObject(o) {
    if (!hasObjectPrototype(o)) {
      return false;
    }
    const ctor = o.constructor;
    if (ctor === undefined) {
      return true;
    }
    const prot = ctor.prototype;
    if (!hasObjectPrototype(prot)) {
      return false;
    }
    if (!prot.hasOwnProperty("isPrototypeOf")) {
      return false;
    }
    if (Object.getPrototypeOf(o) !== Object.prototype) {
      return false;
    }
    return true;
  }
  function hasObjectPrototype(o) {
    return Object.prototype.toString.call(o) === "[object Object]";
  }
  function sleep(timeout) {
    return new Promise((resolve) => {
      timeoutManager.setTimeout(resolve, timeout);
    });
  }
  function replaceData(prevData, data, options) {
    if (typeof options.structuralSharing === "function") {
      return options.structuralSharing(prevData, data);
    } else if (options.structuralSharing !== false) {
      if (true) {
        try {
          return replaceEqualDeep(prevData, data);
        } catch (error) {
          console.error(`Structural sharing requires data to be JSON serializable. To fix this, turn off structuralSharing or return JSON-serializable data from your queryFn. [${options.queryHash}]: ${error}`);
          throw error;
        }
      }
      return replaceEqualDeep(prevData, data);
    }
    return data;
  }
  function addToEnd(items, item, max = 0) {
    const newItems = [...items, item];
    return max && newItems.length > max ? newItems.slice(1) : newItems;
  }
  function addToStart(items, item, max = 0) {
    const newItems = [item, ...items];
    return max && newItems.length > max ? newItems.slice(0, -1) : newItems;
  }
  var skipToken = /* @__PURE__ */ Symbol();
  function ensureQueryFn(options, fetchOptions) {
    if (true) {
      if (options.queryFn === skipToken) {
        console.error(`Attempted to invoke queryFn when set to skipToken. This is likely a configuration error. Query hash: '${options.queryHash}'`);
      }
    }
    if (!options.queryFn && fetchOptions?.initialPromise) {
      return () => fetchOptions.initialPromise;
    }
    if (!options.queryFn || options.queryFn === skipToken) {
      return () => Promise.reject(new Error(`Missing queryFn: '${options.queryHash}'`));
    }
    return options.queryFn;
  }
  function shouldThrowError(throwOnError, params) {
    if (typeof throwOnError === "function") {
      return throwOnError(...params);
    }
    return !!throwOnError;
  }
  function addConsumeAwareSignal(object, getSignal, onCancelled) {
    let consumed = false;
    let signal;
    Object.defineProperty(object, "signal", {
      enumerable: true,
      get: () => {
        signal ??= getSignal();
        if (consumed) {
          return signal;
        }
        consumed = true;
        if (signal.aborted) {
          onCancelled();
        } else {
          signal.addEventListener("abort", onCancelled, { once: true });
        }
        return signal;
      }
    });
    return object;
  }

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/environmentManager.js
  var environmentManager = /* @__PURE__ */ (() => {
    let isServerFn = () => isServer;
    return {
      isServer() {
        return isServerFn();
      },
      setIsServer(isServerValue) {
        isServerFn = isServerValue;
      }
    };
  })();

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/thenable.js
  function pendingThenable() {
    let resolve;
    let reject;
    const thenable = new Promise((_resolve, _reject) => {
      resolve = _resolve;
      reject = _reject;
    });
    thenable.status = "pending";
    thenable.catch(() => {});
    function finalize(data) {
      Object.assign(thenable, data);
      delete thenable.resolve;
      delete thenable.reject;
    }
    thenable.resolve = (value) => {
      finalize({
        status: "fulfilled",
        value
      });
      resolve(value);
    };
    thenable.reject = (reason) => {
      finalize({
        status: "rejected",
        reason
      });
      reject(reason);
    };
    return thenable;
  }

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/notifyManager.js
  var defaultScheduler = systemSetTimeoutZero;
  function createNotifyManager() {
    let queue = [];
    let transactions = 0;
    let notifyFn = (callback) => {
      callback();
    };
    let batchNotifyFn = (callback) => {
      callback();
    };
    let scheduleFn = defaultScheduler;
    const schedule = (callback) => {
      if (transactions) {
        queue.push(callback);
      } else {
        scheduleFn(() => {
          notifyFn(callback);
        });
      }
    };
    const flush = () => {
      const originalQueue = queue;
      queue = [];
      if (originalQueue.length) {
        scheduleFn(() => {
          batchNotifyFn(() => {
            originalQueue.forEach((callback) => {
              notifyFn(callback);
            });
          });
        });
      }
    };
    return {
      batch: (callback) => {
        let result;
        transactions++;
        try {
          result = callback();
        } finally {
          transactions--;
          if (!transactions) {
            flush();
          }
        }
        return result;
      },
      batchCalls: (callback) => {
        return (...args) => {
          schedule(() => {
            callback(...args);
          });
        };
      },
      schedule,
      setNotifyFunction: (fn) => {
        notifyFn = fn;
      },
      setBatchNotifyFunction: (fn) => {
        batchNotifyFn = fn;
      },
      setScheduler: (fn) => {
        scheduleFn = fn;
      }
    };
  }
  var notifyManager = createNotifyManager();

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/onlineManager.js
  var OnlineManager = class extends Subscribable {
    #online = true;
    #cleanup;
    #setup;
    constructor() {
      super();
      this.#setup = (onOnline) => {
        if (typeof window !== "undefined" && window.addEventListener) {
          const onlineListener = () => onOnline(true);
          const offlineListener = () => onOnline(false);
          window.addEventListener("online", onlineListener, false);
          window.addEventListener("offline", offlineListener, false);
          return () => {
            window.removeEventListener("online", onlineListener);
            window.removeEventListener("offline", offlineListener);
          };
        }
        return;
      };
    }
    onSubscribe() {
      if (!this.#cleanup) {
        this.setEventListener(this.#setup);
      }
    }
    onUnsubscribe() {
      if (!this.hasListeners()) {
        this.#cleanup?.();
        this.#cleanup = undefined;
      }
    }
    setEventListener(setup) {
      this.#setup = setup;
      this.#cleanup?.();
      this.#cleanup = setup(this.setOnline.bind(this));
    }
    setOnline(online) {
      const changed = this.#online !== online;
      if (changed) {
        this.#online = online;
        this.listeners.forEach((listener) => {
          listener(online);
        });
      }
    }
    isOnline() {
      return this.#online;
    }
  };
  var onlineManager = new OnlineManager;

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/retryer.js
  function defaultRetryDelay(failureCount) {
    return Math.min(1000 * 2 ** failureCount, 30000);
  }
  function canFetch(networkMode) {
    return (networkMode ?? "online") === "online" ? onlineManager.isOnline() : true;
  }
  var CancelledError = class extends Error {
    constructor(options) {
      super("CancelledError");
      this.revert = options?.revert;
      this.silent = options?.silent;
    }
  };
  function createRetryer(config) {
    let isRetryCancelled = false;
    let failureCount = 0;
    let continueFn;
    const thenable = pendingThenable();
    const isResolved = () => thenable.status !== "pending";
    const cancel = (cancelOptions) => {
      if (!isResolved()) {
        const error = new CancelledError(cancelOptions);
        reject(error);
        config.onCancel?.(error);
      }
    };
    const cancelRetry = () => {
      isRetryCancelled = true;
    };
    const continueRetry = () => {
      isRetryCancelled = false;
    };
    const canContinue = () => focusManager.isFocused() && (config.networkMode === "always" || onlineManager.isOnline()) && config.canRun();
    const canStart = () => canFetch(config.networkMode) && config.canRun();
    const resolve = (value) => {
      if (!isResolved()) {
        continueFn?.();
        thenable.resolve(value);
      }
    };
    const reject = (value) => {
      if (!isResolved()) {
        continueFn?.();
        thenable.reject(value);
      }
    };
    const pause = () => {
      return new Promise((continueResolve) => {
        continueFn = (value) => {
          if (isResolved() || canContinue()) {
            continueResolve(value);
          }
        };
        config.onPause?.();
      }).then(() => {
        continueFn = undefined;
        if (!isResolved()) {
          config.onContinue?.();
        }
      });
    };
    const run = () => {
      if (isResolved()) {
        return;
      }
      let promiseOrValue;
      const initialPromise = failureCount === 0 ? config.initialPromise : undefined;
      try {
        promiseOrValue = initialPromise ?? config.fn();
      } catch (error) {
        promiseOrValue = Promise.reject(error);
      }
      Promise.resolve(promiseOrValue).then(resolve).catch((error) => {
        if (isResolved()) {
          return;
        }
        const retry = config.retry ?? (environmentManager.isServer() ? 0 : 3);
        const retryDelay = config.retryDelay ?? defaultRetryDelay;
        const delay = typeof retryDelay === "function" ? retryDelay(failureCount, error) : retryDelay;
        const shouldRetry = retry === true || typeof retry === "number" && failureCount < retry || typeof retry === "function" && retry(failureCount, error);
        if (isRetryCancelled || !shouldRetry) {
          reject(error);
          return;
        }
        failureCount++;
        config.onFail?.(failureCount, error);
        sleep(delay).then(() => {
          return canContinue() ? undefined : pause();
        }).then(() => {
          if (isRetryCancelled) {
            reject(error);
          } else {
            run();
          }
        });
      });
    };
    return {
      promise: thenable,
      status: () => thenable.status,
      cancel,
      continue: () => {
        continueFn?.();
        return thenable;
      },
      cancelRetry,
      continueRetry,
      canStart,
      start: () => {
        if (canStart()) {
          run();
        } else {
          pause().then(run);
        }
        return thenable;
      }
    };
  }

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/removable.js
  var Removable = class {
    #gcTimeout;
    destroy() {
      this.clearGcTimeout();
    }
    scheduleGc() {
      this.clearGcTimeout();
      if (isValidTimeout(this.gcTime)) {
        this.#gcTimeout = timeoutManager.setTimeout(() => {
          this.optionalRemove();
        }, this.gcTime);
      }
    }
    updateGcTime(newGcTime) {
      this.gcTime = Math.max(this.gcTime || 0, newGcTime ?? (environmentManager.isServer() ? Infinity : 5 * 60 * 1000));
    }
    clearGcTimeout() {
      if (this.#gcTimeout !== undefined) {
        timeoutManager.clearTimeout(this.#gcTimeout);
        this.#gcTimeout = undefined;
      }
    }
  };

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/infiniteQueryBehavior.js
  function infiniteQueryBehavior(pages) {
    return {
      onFetch: (context, query) => {
        const options = context.options;
        const direction = context.fetchOptions?.meta?.fetchMore?.direction;
        const oldPages = context.state.data?.pages || [];
        const oldPageParams = context.state.data?.pageParams || [];
        let result = { pages: [], pageParams: [] };
        let currentPage = 0;
        const fetchFn = async () => {
          let cancelled = false;
          const addSignalProperty = (object) => {
            addConsumeAwareSignal(object, () => context.signal, () => cancelled = true);
          };
          const queryFn = ensureQueryFn(context.options, context.fetchOptions);
          const fetchPage = async (data, param, previous) => {
            if (cancelled) {
              return Promise.reject(context.signal.reason);
            }
            if (param == null && data.pages.length) {
              return Promise.resolve(data);
            }
            const createQueryFnContext = () => {
              const queryFnContext2 = {
                client: context.client,
                queryKey: context.queryKey,
                pageParam: param,
                direction: previous ? "backward" : "forward",
                meta: context.options.meta
              };
              addSignalProperty(queryFnContext2);
              return queryFnContext2;
            };
            const queryFnContext = createQueryFnContext();
            const page = await queryFn(queryFnContext);
            const { maxPages } = context.options;
            const addTo = previous ? addToStart : addToEnd;
            return {
              pages: addTo(data.pages, page, maxPages),
              pageParams: addTo(data.pageParams, param, maxPages)
            };
          };
          if (direction && oldPages.length) {
            const previous = direction === "backward";
            const pageParamFn = previous ? getPreviousPageParam : getNextPageParam;
            const oldData = {
              pages: oldPages,
              pageParams: oldPageParams
            };
            const param = pageParamFn(options, oldData);
            result = await fetchPage(oldData, param, previous);
          } else {
            const remainingPages = pages ?? oldPages.length;
            do {
              const param = currentPage === 0 ? oldPageParams[0] ?? options.initialPageParam : getNextPageParam(options, result);
              if (currentPage > 0 && param == null) {
                break;
              }
              result = await fetchPage(result, param);
              currentPage++;
            } while (currentPage < remainingPages);
          }
          return result;
        };
        if (context.options.persister) {
          context.fetchFn = () => {
            return context.options.persister?.(fetchFn, {
              client: context.client,
              queryKey: context.queryKey,
              meta: context.options.meta,
              signal: context.signal
            }, query);
          };
        } else {
          context.fetchFn = fetchFn;
        }
      }
    };
  }
  function getNextPageParam(options, { pages, pageParams }) {
    const lastIndex = pages.length - 1;
    return pages.length > 0 ? options.getNextPageParam(pages[lastIndex], pages, pageParams[lastIndex], pageParams) : undefined;
  }
  function getPreviousPageParam(options, { pages, pageParams }) {
    return pages.length > 0 ? options.getPreviousPageParam?.(pages[0], pages, pageParams[0], pageParams) : undefined;
  }
  function hasNextPage(options, data) {
    if (!data)
      return false;
    return getNextPageParam(options, data) != null;
  }
  function hasPreviousPage(options, data) {
    if (!data || !options.getPreviousPageParam)
      return false;
    return getPreviousPageParam(options, data) != null;
  }

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/query.js
  var Query = class extends Removable {
    #queryType;
    #initialState;
    #revertState;
    #cache;
    #client;
    #retryer;
    #defaultOptions;
    #abortSignalConsumed;
    constructor(config) {
      super();
      this.#abortSignalConsumed = false;
      this.#defaultOptions = config.defaultOptions;
      this.setOptions(config.options);
      this.observers = [];
      this.#client = config.client;
      this.#cache = this.#client.getQueryCache();
      this.queryKey = config.queryKey;
      this.queryHash = config.queryHash;
      this.#initialState = getDefaultState(this.options);
      this.state = config.state ?? this.#initialState;
      this.scheduleGc();
    }
    get meta() {
      return this.options.meta;
    }
    get queryType() {
      return this.#queryType;
    }
    get promise() {
      return this.#retryer?.promise;
    }
    setOptions(options) {
      this.options = { ...this.#defaultOptions, ...options };
      if (options?._type) {
        this.#queryType = options._type;
      }
      this.updateGcTime(this.options.gcTime);
      if (this.state && this.state.data === undefined) {
        const defaultState = getDefaultState(this.options);
        if (defaultState.data !== undefined) {
          this.setState(successState(defaultState.data, defaultState.dataUpdatedAt));
          this.#initialState = defaultState;
        }
      }
    }
    optionalRemove() {
      if (!this.observers.length && this.state.fetchStatus === "idle") {
        this.#cache.remove(this);
      }
    }
    setData(newData, options) {
      const data = replaceData(this.state.data, newData, this.options);
      this.#dispatch({
        data,
        type: "success",
        dataUpdatedAt: options?.updatedAt,
        manual: options?.manual
      });
      return data;
    }
    setState(state) {
      this.#dispatch({ type: "setState", state });
    }
    cancel(options) {
      const promise = this.#retryer?.promise;
      this.#retryer?.cancel(options);
      return promise ? promise.then(noop).catch(noop) : Promise.resolve();
    }
    destroy() {
      super.destroy();
      this.cancel({ silent: true });
    }
    get resetState() {
      return this.#initialState;
    }
    reset() {
      this.destroy();
      this.setState(this.resetState);
    }
    isActive() {
      return this.observers.some((observer) => resolveQueryBoolean(observer.options.enabled, this) !== false);
    }
    isDisabled() {
      if (this.getObserversCount() > 0) {
        return !this.isActive();
      }
      return this.options.queryFn === skipToken || !this.isFetched();
    }
    isFetched() {
      return this.state.dataUpdateCount + this.state.errorUpdateCount > 0;
    }
    isStatic() {
      if (this.getObserversCount() > 0) {
        return this.observers.some((observer) => resolveStaleTime(observer.options.staleTime, this) === "static");
      }
      return false;
    }
    isStale() {
      if (this.getObserversCount() > 0) {
        return this.observers.some((observer) => observer.getCurrentResult().isStale);
      }
      return this.state.data === undefined || this.state.isInvalidated;
    }
    isStaleByTime(staleTime = 0) {
      if (this.state.data === undefined) {
        return true;
      }
      if (staleTime === "static") {
        return false;
      }
      if (this.state.isInvalidated) {
        return true;
      }
      return !timeUntilStale(this.state.dataUpdatedAt, staleTime);
    }
    onFocus() {
      const observer = this.observers.find((x) => x.shouldFetchOnWindowFocus());
      observer?.refetch({ cancelRefetch: false });
      this.#retryer?.continue();
    }
    onOnline() {
      const observer = this.observers.find((x) => x.shouldFetchOnReconnect());
      observer?.refetch({ cancelRefetch: false });
      this.#retryer?.continue();
    }
    addObserver(observer) {
      if (!this.observers.includes(observer)) {
        this.observers.push(observer);
        this.clearGcTimeout();
        this.#cache.notify({ type: "observerAdded", query: this, observer });
      }
    }
    removeObserver(observer) {
      if (this.observers.includes(observer)) {
        this.observers = this.observers.filter((x) => x !== observer);
        if (!this.observers.length) {
          if (this.#retryer) {
            if (this.#abortSignalConsumed || this.#isInitialPausedFetch()) {
              this.#retryer.cancel({ revert: true });
            } else {
              this.#retryer.cancelRetry();
            }
          }
          this.scheduleGc();
        }
        this.#cache.notify({ type: "observerRemoved", query: this, observer });
      }
    }
    getObserversCount() {
      return this.observers.length;
    }
    #isInitialPausedFetch() {
      return this.state.fetchStatus === "paused" && this.state.status === "pending";
    }
    invalidate() {
      if (!this.state.isInvalidated) {
        this.#dispatch({ type: "invalidate" });
      }
    }
    async fetch(options, fetchOptions) {
      if (this.state.fetchStatus !== "idle" && this.#retryer?.status() !== "rejected") {
        if (this.state.data !== undefined && fetchOptions?.cancelRefetch) {
          this.cancel({ silent: true });
        } else if (this.#retryer) {
          this.#retryer.continueRetry();
          return this.#retryer.promise;
        }
      }
      if (options) {
        this.setOptions(options);
      }
      if (!this.options.queryFn) {
        const observer = this.observers.find((x) => x.options.queryFn);
        if (observer) {
          this.setOptions(observer.options);
        }
      }
      if (true) {
        if (!Array.isArray(this.options.queryKey)) {
          console.error(`As of v4, queryKey needs to be an Array. If you are using a string like 'repoData', please change it to an Array, e.g. ['repoData']`);
        }
      }
      const abortController = new AbortController;
      const addSignalProperty = (object) => {
        Object.defineProperty(object, "signal", {
          enumerable: true,
          get: () => {
            this.#abortSignalConsumed = true;
            return abortController.signal;
          }
        });
      };
      const fetchFn = () => {
        const queryFn = ensureQueryFn(this.options, fetchOptions);
        const createQueryFnContext = () => {
          const queryFnContext2 = {
            client: this.#client,
            queryKey: this.queryKey,
            meta: this.meta
          };
          addSignalProperty(queryFnContext2);
          return queryFnContext2;
        };
        const queryFnContext = createQueryFnContext();
        this.#abortSignalConsumed = false;
        if (this.options.persister) {
          return this.options.persister(queryFn, queryFnContext, this);
        }
        return queryFn(queryFnContext);
      };
      const createFetchContext = () => {
        const context2 = {
          fetchOptions,
          options: this.options,
          queryKey: this.queryKey,
          client: this.#client,
          state: this.state,
          fetchFn
        };
        addSignalProperty(context2);
        return context2;
      };
      const context = createFetchContext();
      const behavior = this.#queryType === "infinite" ? infiniteQueryBehavior(this.options.pages) : this.options.behavior;
      behavior?.onFetch(context, this);
      this.#revertState = this.state;
      if (this.state.fetchStatus === "idle" || this.state.fetchMeta !== context.fetchOptions?.meta) {
        this.#dispatch({ type: "fetch", meta: context.fetchOptions?.meta });
      }
      this.#retryer = createRetryer({
        initialPromise: fetchOptions?.initialPromise,
        fn: context.fetchFn,
        onCancel: (error) => {
          if (error instanceof CancelledError && error.revert) {
            this.setState({
              ...this.#revertState,
              fetchStatus: "idle"
            });
          }
          abortController.abort();
        },
        onFail: (failureCount, error) => {
          this.#dispatch({ type: "failed", failureCount, error });
        },
        onPause: () => {
          this.#dispatch({ type: "pause" });
        },
        onContinue: () => {
          this.#dispatch({ type: "continue" });
        },
        retry: context.options.retry,
        retryDelay: context.options.retryDelay,
        networkMode: context.options.networkMode,
        canRun: () => true
      });
      try {
        const data = await this.#retryer.start();
        if (data === undefined) {
          if (true) {
            console.error(`Query data cannot be undefined. Please make sure to return a value other than undefined from your query function. Affected query key: ${this.queryHash}`);
          }
          throw new Error(`${this.queryHash} data is undefined`);
        }
        this.setData(data);
        this.#cache.config.onSuccess?.(data, this);
        this.#cache.config.onSettled?.(data, this.state.error, this);
        return data;
      } catch (error) {
        if (error instanceof CancelledError) {
          if (error.silent) {
            return this.#retryer.promise;
          } else if (error.revert) {
            if (this.state.data === undefined) {
              throw error;
            }
            return this.state.data;
          }
        }
        this.#dispatch({
          type: "error",
          error
        });
        this.#cache.config.onError?.(error, this);
        this.#cache.config.onSettled?.(this.state.data, error, this);
        throw error;
      } finally {
        this.scheduleGc();
      }
    }
    #dispatch(action) {
      const reducer = (state) => {
        switch (action.type) {
          case "failed":
            return {
              ...state,
              fetchFailureCount: action.failureCount,
              fetchFailureReason: action.error
            };
          case "pause":
            return {
              ...state,
              fetchStatus: "paused"
            };
          case "continue":
            return {
              ...state,
              fetchStatus: "fetching"
            };
          case "fetch":
            return {
              ...state,
              ...fetchState(state.data, this.options),
              fetchMeta: action.meta ?? null
            };
          case "success":
            const newState = {
              ...state,
              ...successState(action.data, action.dataUpdatedAt),
              dataUpdateCount: state.dataUpdateCount + 1,
              ...!action.manual && {
                fetchStatus: "idle",
                fetchFailureCount: 0,
                fetchFailureReason: null
              }
            };
            this.#revertState = action.manual ? newState : undefined;
            return newState;
          case "error":
            const error = action.error;
            return {
              ...state,
              error,
              errorUpdateCount: state.errorUpdateCount + 1,
              errorUpdatedAt: Date.now(),
              fetchFailureCount: state.fetchFailureCount + 1,
              fetchFailureReason: error,
              fetchStatus: "idle",
              status: "error",
              isInvalidated: true
            };
          case "invalidate":
            return {
              ...state,
              isInvalidated: true
            };
          case "setState":
            return {
              ...state,
              ...action.state
            };
        }
      };
      this.state = reducer(this.state);
      notifyManager.batch(() => {
        this.observers.forEach((observer) => {
          observer.onQueryUpdate();
        });
        this.#cache.notify({ query: this, type: "updated", action });
      });
    }
  };
  function fetchState(data, options) {
    return {
      fetchFailureCount: 0,
      fetchFailureReason: null,
      fetchStatus: canFetch(options.networkMode) ? "fetching" : "paused",
      ...data === undefined && {
        error: null,
        status: "pending"
      }
    };
  }
  function successState(data, dataUpdatedAt) {
    return {
      data,
      dataUpdatedAt: dataUpdatedAt ?? Date.now(),
      error: null,
      isInvalidated: false,
      status: "success"
    };
  }
  function getDefaultState(options) {
    const data = typeof options.initialData === "function" ? options.initialData() : options.initialData;
    const hasData = data !== undefined;
    const initialDataUpdatedAt = hasData ? typeof options.initialDataUpdatedAt === "function" ? options.initialDataUpdatedAt() : options.initialDataUpdatedAt : 0;
    return {
      data,
      dataUpdateCount: 0,
      dataUpdatedAt: hasData ? initialDataUpdatedAt ?? Date.now() : 0,
      error: null,
      errorUpdateCount: 0,
      errorUpdatedAt: 0,
      fetchFailureCount: 0,
      fetchFailureReason: null,
      fetchMeta: null,
      isInvalidated: false,
      status: hasData ? "success" : "pending",
      fetchStatus: "idle"
    };
  }

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/queryObserver.js
  var QueryObserver = class extends Subscribable {
    constructor(client, options) {
      super();
      this.options = options;
      this.#client = client;
      this.#selectError = null;
      this.#currentThenable = pendingThenable();
      this.bindMethods();
      this.setOptions(options);
    }
    #client;
    #currentQuery = undefined;
    #currentQueryInitialState = undefined;
    #currentResult = undefined;
    #currentResultState;
    #currentResultOptions;
    #currentThenable;
    #selectError;
    #selectFn;
    #selectResult;
    #lastQueryWithDefinedData;
    #staleTimeoutId;
    #refetchIntervalId;
    #currentRefetchInterval;
    #trackedProps = /* @__PURE__ */ new Set;
    bindMethods() {
      this.refetch = this.refetch.bind(this);
    }
    onSubscribe() {
      if (this.listeners.size === 1) {
        this.#currentQuery.addObserver(this);
        if (shouldFetchOnMount(this.#currentQuery, this.options)) {
          this.#executeFetch();
        } else {
          this.updateResult();
        }
        this.#updateTimers();
      }
    }
    onUnsubscribe() {
      if (!this.hasListeners()) {
        this.destroy();
      }
    }
    shouldFetchOnReconnect() {
      return shouldFetchOn(this.#currentQuery, this.options, this.options.refetchOnReconnect);
    }
    shouldFetchOnWindowFocus() {
      return shouldFetchOn(this.#currentQuery, this.options, this.options.refetchOnWindowFocus);
    }
    destroy() {
      this.listeners = /* @__PURE__ */ new Set;
      this.#clearStaleTimeout();
      this.#clearRefetchInterval();
      this.#currentQuery.removeObserver(this);
    }
    setOptions(options) {
      const prevOptions = this.options;
      const prevQuery = this.#currentQuery;
      this.options = this.#client.defaultQueryOptions(options);
      if (this.options.enabled !== undefined && typeof this.options.enabled !== "boolean" && typeof this.options.enabled !== "function" && typeof resolveQueryBoolean(this.options.enabled, this.#currentQuery) !== "boolean") {
        throw new Error("Expected enabled to be a boolean or a callback that returns a boolean");
      }
      this.#updateQuery();
      this.#currentQuery.setOptions(this.options);
      if (prevOptions._defaulted && !shallowEqualObjects(this.options, prevOptions)) {
        this.#client.getQueryCache().notify({
          type: "observerOptionsUpdated",
          query: this.#currentQuery,
          observer: this
        });
      }
      const mounted = this.hasListeners();
      if (mounted && shouldFetchOptionally(this.#currentQuery, prevQuery, this.options, prevOptions)) {
        this.#executeFetch();
      }
      this.updateResult();
      if (mounted && (this.#currentQuery !== prevQuery || resolveQueryBoolean(this.options.enabled, this.#currentQuery) !== resolveQueryBoolean(prevOptions.enabled, this.#currentQuery) || resolveStaleTime(this.options.staleTime, this.#currentQuery) !== resolveStaleTime(prevOptions.staleTime, this.#currentQuery))) {
        this.#updateStaleTimeout();
      }
      const nextRefetchInterval = this.#computeRefetchInterval();
      if (mounted && (this.#currentQuery !== prevQuery || resolveQueryBoolean(this.options.enabled, this.#currentQuery) !== resolveQueryBoolean(prevOptions.enabled, this.#currentQuery) || nextRefetchInterval !== this.#currentRefetchInterval)) {
        this.#updateRefetchInterval(nextRefetchInterval);
      }
    }
    getOptimisticResult(options) {
      const query = this.#client.getQueryCache().build(this.#client, options);
      const result = this.createResult(query, options);
      if (shouldAssignObserverCurrentProperties(this, result)) {
        this.#currentResult = result;
        this.#currentResultOptions = this.options;
        this.#currentResultState = this.#currentQuery.state;
      }
      return result;
    }
    getCurrentResult() {
      return this.#currentResult;
    }
    trackResult(result, onPropTracked) {
      return new Proxy(result, {
        get: (target, key) => {
          this.trackProp(key);
          onPropTracked?.(key);
          if (key === "promise") {
            this.trackProp("data");
            if (!this.options.experimental_prefetchInRender && this.#currentThenable.status === "pending") {
              this.#currentThenable.reject(new Error("experimental_prefetchInRender feature flag is not enabled"));
            }
          }
          return Reflect.get(target, key);
        }
      });
    }
    trackProp(key) {
      this.#trackedProps.add(key);
    }
    getCurrentQuery() {
      return this.#currentQuery;
    }
    refetch({ ...options } = {}) {
      return this.fetch({
        ...options
      });
    }
    fetchOptimistic(options) {
      const defaultedOptions = this.#client.defaultQueryOptions(options);
      const query = this.#client.getQueryCache().build(this.#client, defaultedOptions);
      return query.fetch().then(() => this.createResult(query, defaultedOptions));
    }
    fetch(fetchOptions) {
      return this.#executeFetch({
        ...fetchOptions,
        cancelRefetch: fetchOptions.cancelRefetch ?? true
      }).then(() => {
        this.updateResult();
        return this.#currentResult;
      });
    }
    #executeFetch(fetchOptions) {
      this.#updateQuery();
      let promise = this.#currentQuery.fetch(this.options, fetchOptions);
      if (!fetchOptions?.throwOnError) {
        promise = promise.catch(noop);
      }
      return promise;
    }
    #updateStaleTimeout() {
      this.#clearStaleTimeout();
      const staleTime = resolveStaleTime(this.options.staleTime, this.#currentQuery);
      if (environmentManager.isServer() || this.#currentResult.isStale || !isValidTimeout(staleTime)) {
        return;
      }
      const time = timeUntilStale(this.#currentResult.dataUpdatedAt, staleTime);
      const timeout = time + 1;
      this.#staleTimeoutId = timeoutManager.setTimeout(() => {
        if (!this.#currentResult.isStale) {
          this.updateResult();
        }
      }, timeout);
    }
    #computeRefetchInterval() {
      return (typeof this.options.refetchInterval === "function" ? this.options.refetchInterval(this.#currentQuery) : this.options.refetchInterval) ?? false;
    }
    #updateRefetchInterval(nextInterval) {
      this.#clearRefetchInterval();
      this.#currentRefetchInterval = nextInterval;
      if (environmentManager.isServer() || resolveQueryBoolean(this.options.enabled, this.#currentQuery) === false || !isValidTimeout(this.#currentRefetchInterval) || this.#currentRefetchInterval === 0) {
        return;
      }
      this.#refetchIntervalId = timeoutManager.setInterval(() => {
        if (this.options.refetchIntervalInBackground || focusManager.isFocused()) {
          this.#executeFetch();
        }
      }, this.#currentRefetchInterval);
    }
    #updateTimers() {
      this.#updateStaleTimeout();
      this.#updateRefetchInterval(this.#computeRefetchInterval());
    }
    #clearStaleTimeout() {
      if (this.#staleTimeoutId !== undefined) {
        timeoutManager.clearTimeout(this.#staleTimeoutId);
        this.#staleTimeoutId = undefined;
      }
    }
    #clearRefetchInterval() {
      if (this.#refetchIntervalId !== undefined) {
        timeoutManager.clearInterval(this.#refetchIntervalId);
        this.#refetchIntervalId = undefined;
      }
    }
    createResult(query, options) {
      const prevQuery = this.#currentQuery;
      const prevOptions = this.options;
      const prevResult = this.#currentResult;
      const prevResultState = this.#currentResultState;
      const prevResultOptions = this.#currentResultOptions;
      const queryChange = query !== prevQuery;
      const queryInitialState = queryChange ? query.state : this.#currentQueryInitialState;
      const { state } = query;
      let newState = { ...state };
      let isPlaceholderData = false;
      let data;
      if (options._optimisticResults) {
        const mounted = this.hasListeners();
        const fetchOnMount = !mounted && shouldFetchOnMount(query, options);
        const fetchOptionally = mounted && shouldFetchOptionally(query, prevQuery, options, prevOptions);
        if (fetchOnMount || fetchOptionally) {
          newState = {
            ...newState,
            ...fetchState(state.data, query.options)
          };
        }
        if (options._optimisticResults === "isRestoring") {
          newState.fetchStatus = "idle";
        }
      }
      let { error, errorUpdatedAt, status } = newState;
      data = newState.data;
      let skipSelect = false;
      if (options.placeholderData !== undefined && data === undefined && status === "pending") {
        let placeholderData;
        if (prevResult?.isPlaceholderData && options.placeholderData === prevResultOptions?.placeholderData) {
          placeholderData = prevResult.data;
          skipSelect = true;
        } else {
          placeholderData = typeof options.placeholderData === "function" ? options.placeholderData(this.#lastQueryWithDefinedData?.state.data, this.#lastQueryWithDefinedData) : options.placeholderData;
        }
        if (placeholderData !== undefined) {
          status = "success";
          data = replaceData(prevResult?.data, placeholderData, options);
          isPlaceholderData = true;
        }
      }
      if (options.select && data !== undefined && !skipSelect) {
        if (prevResult && data === prevResultState?.data && options.select === this.#selectFn) {
          data = this.#selectResult;
        } else {
          try {
            this.#selectFn = options.select;
            data = options.select(data);
            data = replaceData(prevResult?.data, data, options);
            this.#selectResult = data;
            this.#selectError = null;
          } catch (selectError) {
            this.#selectError = selectError;
          }
        }
      }
      if (this.#selectError) {
        error = this.#selectError;
        data = this.#selectResult;
        errorUpdatedAt = Date.now();
        status = "error";
      }
      const isFetching = newState.fetchStatus === "fetching";
      const isPending = status === "pending";
      const isError = status === "error";
      const isLoading = isPending && isFetching;
      const hasData = data !== undefined;
      const result = {
        status,
        fetchStatus: newState.fetchStatus,
        isPending,
        isSuccess: status === "success",
        isError,
        isInitialLoading: isLoading,
        isLoading,
        data,
        dataUpdatedAt: newState.dataUpdatedAt,
        error,
        errorUpdatedAt,
        failureCount: newState.fetchFailureCount,
        failureReason: newState.fetchFailureReason,
        errorUpdateCount: newState.errorUpdateCount,
        isFetched: query.isFetched(),
        isFetchedAfterMount: newState.dataUpdateCount > queryInitialState.dataUpdateCount || newState.errorUpdateCount > queryInitialState.errorUpdateCount,
        isFetching,
        isRefetching: isFetching && !isPending,
        isLoadingError: isError && !hasData,
        isPaused: newState.fetchStatus === "paused",
        isPlaceholderData,
        isRefetchError: isError && hasData,
        isStale: isStale(query, options),
        refetch: this.refetch,
        promise: this.#currentThenable,
        isEnabled: resolveQueryBoolean(options.enabled, query) !== false
      };
      const nextResult = result;
      if (this.options.experimental_prefetchInRender) {
        const hasResultData = nextResult.data !== undefined;
        const isErrorWithoutData = nextResult.status === "error" && !hasResultData;
        const finalizeThenableIfPossible = (thenable) => {
          if (isErrorWithoutData) {
            thenable.reject(nextResult.error);
          } else if (hasResultData) {
            thenable.resolve(nextResult.data);
          }
        };
        const recreateThenable = () => {
          const pending = this.#currentThenable = nextResult.promise = pendingThenable();
          finalizeThenableIfPossible(pending);
        };
        const prevThenable = this.#currentThenable;
        switch (prevThenable.status) {
          case "pending":
            if (query.queryHash === prevQuery.queryHash) {
              finalizeThenableIfPossible(prevThenable);
            }
            break;
          case "fulfilled":
            if (isErrorWithoutData || nextResult.data !== prevThenable.value) {
              recreateThenable();
            }
            break;
          case "rejected":
            if (!isErrorWithoutData || nextResult.error !== prevThenable.reason) {
              recreateThenable();
            }
            break;
        }
      }
      return nextResult;
    }
    updateResult() {
      const prevResult = this.#currentResult;
      const nextResult = this.createResult(this.#currentQuery, this.options);
      this.#currentResultState = this.#currentQuery.state;
      this.#currentResultOptions = this.options;
      if (this.#currentResultState.data !== undefined) {
        this.#lastQueryWithDefinedData = this.#currentQuery;
      }
      if (shallowEqualObjects(nextResult, prevResult)) {
        return;
      }
      this.#currentResult = nextResult;
      const shouldNotifyListeners = () => {
        if (!prevResult) {
          return true;
        }
        const { notifyOnChangeProps } = this.options;
        const notifyOnChangePropsValue = typeof notifyOnChangeProps === "function" ? notifyOnChangeProps() : notifyOnChangeProps;
        if (notifyOnChangePropsValue === "all" || !notifyOnChangePropsValue && !this.#trackedProps.size) {
          return true;
        }
        const includedProps = new Set(notifyOnChangePropsValue ?? this.#trackedProps);
        if (this.options.throwOnError) {
          includedProps.add("error");
        }
        return Object.keys(this.#currentResult).some((key) => {
          const typedKey = key;
          const changed = this.#currentResult[typedKey] !== prevResult[typedKey];
          return changed && includedProps.has(typedKey);
        });
      };
      this.#notify({ listeners: shouldNotifyListeners() });
    }
    #updateQuery() {
      const query = this.#client.getQueryCache().build(this.#client, this.options);
      if (query === this.#currentQuery) {
        return;
      }
      const prevQuery = this.#currentQuery;
      this.#currentQuery = query;
      this.#currentQueryInitialState = query.state;
      if (this.hasListeners()) {
        prevQuery?.removeObserver(this);
        query.addObserver(this);
      }
    }
    onQueryUpdate() {
      this.updateResult();
      if (this.hasListeners()) {
        this.#updateTimers();
      }
    }
    #notify(notifyOptions) {
      notifyManager.batch(() => {
        if (notifyOptions.listeners) {
          this.listeners.forEach((listener) => {
            listener(this.#currentResult);
          });
        }
        this.#client.getQueryCache().notify({
          query: this.#currentQuery,
          type: "observerResultsUpdated"
        });
      });
    }
  };
  function shouldLoadOnMount(query, options) {
    return resolveQueryBoolean(options.enabled, query) !== false && query.state.data === undefined && !(query.state.status === "error" && resolveQueryBoolean(options.retryOnMount, query) === false);
  }
  function shouldFetchOnMount(query, options) {
    return shouldLoadOnMount(query, options) || query.state.data !== undefined && shouldFetchOn(query, options, options.refetchOnMount);
  }
  function shouldFetchOn(query, options, field) {
    if (resolveQueryBoolean(options.enabled, query) !== false && resolveStaleTime(options.staleTime, query) !== "static") {
      const value = typeof field === "function" ? field(query) : field;
      return value === "always" || value !== false && isStale(query, options);
    }
    return false;
  }
  function shouldFetchOptionally(query, prevQuery, options, prevOptions) {
    return (query !== prevQuery || resolveQueryBoolean(prevOptions.enabled, query) === false) && (!options.suspense || query.state.status !== "error") && isStale(query, options);
  }
  function isStale(query, options) {
    return resolveQueryBoolean(options.enabled, query) !== false && query.isStaleByTime(resolveStaleTime(options.staleTime, query));
  }
  function shouldAssignObserverCurrentProperties(observer, optimisticResult) {
    if (!shallowEqualObjects(observer.getCurrentResult(), optimisticResult)) {
      return true;
    }
    return false;
  }

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/infiniteQueryObserver.js
  var InfiniteQueryObserver = class extends QueryObserver {
    constructor(client, options) {
      super(client, options);
    }
    bindMethods() {
      super.bindMethods();
      this.fetchNextPage = this.fetchNextPage.bind(this);
      this.fetchPreviousPage = this.fetchPreviousPage.bind(this);
    }
    setOptions(options) {
      options._type = "infinite";
      super.setOptions(options);
    }
    getOptimisticResult(options) {
      options._type = "infinite";
      return super.getOptimisticResult(options);
    }
    fetchNextPage(options) {
      return this.fetch({
        ...options,
        meta: {
          fetchMore: { direction: "forward" }
        }
      });
    }
    fetchPreviousPage(options) {
      return this.fetch({
        ...options,
        meta: {
          fetchMore: { direction: "backward" }
        }
      });
    }
    createResult(query, options) {
      const { state } = query;
      const parentResult = super.createResult(query, options);
      const { isFetching, isRefetching, isError, isRefetchError } = parentResult;
      const fetchDirection = state.fetchMeta?.fetchMore?.direction;
      const isFetchNextPageError = isError && fetchDirection === "forward";
      const isFetchingNextPage = isFetching && fetchDirection === "forward";
      const isFetchPreviousPageError = isError && fetchDirection === "backward";
      const isFetchingPreviousPage = isFetching && fetchDirection === "backward";
      const result = {
        ...parentResult,
        fetchNextPage: this.fetchNextPage,
        fetchPreviousPage: this.fetchPreviousPage,
        hasNextPage: hasNextPage(options, state.data),
        hasPreviousPage: hasPreviousPage(options, state.data),
        isFetchNextPageError,
        isFetchingNextPage,
        isFetchPreviousPageError,
        isFetchingPreviousPage,
        isRefetchError: isRefetchError && !isFetchNextPageError && !isFetchPreviousPageError,
        isRefetching: isRefetching && !isFetchingNextPage && !isFetchingPreviousPage
      };
      return result;
    }
  };

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/mutation.js
  var Mutation = class extends Removable {
    #client;
    #observers;
    #mutationCache;
    #retryer;
    constructor(config) {
      super();
      this.#client = config.client;
      this.mutationId = config.mutationId;
      this.#mutationCache = config.mutationCache;
      this.#observers = [];
      this.state = config.state || getDefaultState2();
      this.setOptions(config.options);
      this.scheduleGc();
    }
    setOptions(options) {
      this.options = options;
      this.updateGcTime(this.options.gcTime);
    }
    get meta() {
      return this.options.meta;
    }
    addObserver(observer) {
      if (!this.#observers.includes(observer)) {
        this.#observers.push(observer);
        this.clearGcTimeout();
        this.#mutationCache.notify({
          type: "observerAdded",
          mutation: this,
          observer
        });
      }
    }
    removeObserver(observer) {
      this.#observers = this.#observers.filter((x) => x !== observer);
      this.scheduleGc();
      this.#mutationCache.notify({
        type: "observerRemoved",
        mutation: this,
        observer
      });
    }
    optionalRemove() {
      if (!this.#observers.length) {
        if (this.state.status === "pending") {
          this.scheduleGc();
        } else {
          this.#mutationCache.remove(this);
        }
      }
    }
    continue() {
      return this.#retryer?.continue() ?? this.execute(this.state.variables);
    }
    async execute(variables) {
      const onContinue = () => {
        this.#dispatch({ type: "continue" });
      };
      const mutationFnContext = {
        client: this.#client,
        meta: this.options.meta,
        mutationKey: this.options.mutationKey
      };
      this.#retryer = createRetryer({
        fn: () => {
          if (!this.options.mutationFn) {
            return Promise.reject(new Error("No mutationFn found"));
          }
          return this.options.mutationFn(variables, mutationFnContext);
        },
        onFail: (failureCount, error) => {
          this.#dispatch({ type: "failed", failureCount, error });
        },
        onPause: () => {
          this.#dispatch({ type: "pause" });
        },
        onContinue,
        retry: this.options.retry ?? 0,
        retryDelay: this.options.retryDelay,
        networkMode: this.options.networkMode,
        canRun: () => this.#mutationCache.canRun(this)
      });
      const restored = this.state.status === "pending";
      const isPaused = !this.#retryer.canStart();
      try {
        if (restored) {
          onContinue();
        } else {
          this.#dispatch({ type: "pending", variables, isPaused });
          if (this.#mutationCache.config.onMutate) {
            await this.#mutationCache.config.onMutate(variables, this, mutationFnContext);
          }
          const context = await this.options.onMutate?.(variables, mutationFnContext);
          if (context !== this.state.context) {
            this.#dispatch({
              type: "pending",
              context,
              variables,
              isPaused
            });
          }
        }
        const data = await this.#retryer.start();
        await this.#mutationCache.config.onSuccess?.(data, variables, this.state.context, this, mutationFnContext);
        await this.options.onSuccess?.(data, variables, this.state.context, mutationFnContext);
        await this.#mutationCache.config.onSettled?.(data, null, this.state.variables, this.state.context, this, mutationFnContext);
        await this.options.onSettled?.(data, null, variables, this.state.context, mutationFnContext);
        this.#dispatch({ type: "success", data });
        return data;
      } catch (error) {
        try {
          await this.#mutationCache.config.onError?.(error, variables, this.state.context, this, mutationFnContext);
        } catch (e) {
          Promise.reject(e);
        }
        try {
          await this.options.onError?.(error, variables, this.state.context, mutationFnContext);
        } catch (e) {
          Promise.reject(e);
        }
        try {
          await this.#mutationCache.config.onSettled?.(undefined, error, this.state.variables, this.state.context, this, mutationFnContext);
        } catch (e) {
          Promise.reject(e);
        }
        try {
          await this.options.onSettled?.(undefined, error, variables, this.state.context, mutationFnContext);
        } catch (e) {
          Promise.reject(e);
        }
        this.#dispatch({ type: "error", error });
        throw error;
      } finally {
        this.#mutationCache.runNext(this);
      }
    }
    #dispatch(action) {
      const reducer = (state) => {
        switch (action.type) {
          case "failed":
            return {
              ...state,
              failureCount: action.failureCount,
              failureReason: action.error
            };
          case "pause":
            return {
              ...state,
              isPaused: true
            };
          case "continue":
            return {
              ...state,
              isPaused: false
            };
          case "pending":
            return {
              ...state,
              context: action.context,
              data: undefined,
              failureCount: 0,
              failureReason: null,
              error: null,
              isPaused: action.isPaused,
              status: "pending",
              variables: action.variables,
              submittedAt: Date.now()
            };
          case "success":
            return {
              ...state,
              data: action.data,
              failureCount: 0,
              failureReason: null,
              error: null,
              status: "success",
              isPaused: false
            };
          case "error":
            return {
              ...state,
              data: undefined,
              error: action.error,
              failureCount: state.failureCount + 1,
              failureReason: action.error,
              isPaused: false,
              status: "error"
            };
        }
      };
      this.state = reducer(this.state);
      notifyManager.batch(() => {
        this.#observers.forEach((observer) => {
          observer.onMutationUpdate(action);
        });
        this.#mutationCache.notify({
          mutation: this,
          type: "updated",
          action
        });
      });
    }
  };
  function getDefaultState2() {
    return {
      context: undefined,
      data: undefined,
      error: null,
      failureCount: 0,
      failureReason: null,
      isPaused: false,
      status: "idle",
      variables: undefined,
      submittedAt: 0
    };
  }

  // ../../node_modules/.bun/@tanstack+query-core@5.101.0/node_modules/@tanstack/query-core/build/modern/mutationObserver.js
  var MutationObserver = class extends Subscribable {
    #client;
    #currentResult = undefined;
    #currentMutation;
    #mutateOptions;
    constructor(client, options) {
      super();
      this.#client = client;
      this.setOptions(options);
      this.bindMethods();
      this.#updateResult();
    }
    bindMethods() {
      this.mutate = this.mutate.bind(this);
      this.reset = this.reset.bind(this);
    }
    setOptions(options) {
      const prevOptions = this.options;
      this.options = this.#client.defaultMutationOptions(options);
      if (!shallowEqualObjects(this.options, prevOptions)) {
        this.#client.getMutationCache().notify({
          type: "observerOptionsUpdated",
          mutation: this.#currentMutation,
          observer: this
        });
      }
      if (prevOptions?.mutationKey && this.options.mutationKey && hashKey(prevOptions.mutationKey) !== hashKey(this.options.mutationKey)) {
        this.reset();
      } else if (this.#currentMutation?.state.status === "pending") {
        this.#currentMutation.setOptions(this.options);
      }
    }
    onUnsubscribe() {
      if (!this.hasListeners()) {
        this.#currentMutation?.removeObserver(this);
      }
    }
    onMutationUpdate(action) {
      this.#updateResult();
      this.#notify(action);
    }
    getCurrentResult() {
      return this.#currentResult;
    }
    reset() {
      this.#currentMutation?.removeObserver(this);
      this.#currentMutation = undefined;
      this.#updateResult();
      this.#notify();
    }
    mutate(variables, options) {
      this.#mutateOptions = options;
      this.#currentMutation?.removeObserver(this);
      this.#currentMutation = this.#client.getMutationCache().build(this.#client, this.options);
      this.#currentMutation.addObserver(this);
      return this.#currentMutation.execute(variables);
    }
    #updateResult() {
      const state = this.#currentMutation?.state ?? getDefaultState2();
      this.#currentResult = {
        ...state,
        isPending: state.status === "pending",
        isSuccess: state.status === "success",
        isError: state.status === "error",
        isIdle: state.status === "idle",
        mutate: this.mutate,
        reset: this.reset
      };
    }
    #notify(action) {
      notifyManager.batch(() => {
        if (this.#mutateOptions && this.hasListeners()) {
          const variables = this.#currentResult.variables;
          const onMutateResult = this.#currentResult.context;
          const context = {
            client: this.#client,
            meta: this.options.meta,
            mutationKey: this.options.mutationKey
          };
          if (action?.type === "success") {
            try {
              this.#mutateOptions.onSuccess?.(action.data, variables, onMutateResult, context);
            } catch (e) {
              Promise.reject(e);
            }
            try {
              this.#mutateOptions.onSettled?.(action.data, null, variables, onMutateResult, context);
            } catch (e) {
              Promise.reject(e);
            }
          } else if (action?.type === "error") {
            try {
              this.#mutateOptions.onError?.(action.error, variables, onMutateResult, context);
            } catch (e) {
              Promise.reject(e);
            }
            try {
              this.#mutateOptions.onSettled?.(undefined, action.error, variables, onMutateResult, context);
            } catch (e) {
              Promise.reject(e);
            }
          }
        }
        this.listeners.forEach((listener) => {
          listener(this.#currentResult);
        });
      });
    }
  };
  // ../../node_modules/.bun/@tanstack+react-query@5.101.0+e14d3f224186685e/node_modules/@tanstack/react-query/build/modern/useBaseQuery.js
  var React5 = __toESM(require_react(), 1);

  // ../../node_modules/.bun/@tanstack+react-query@5.101.0+e14d3f224186685e/node_modules/@tanstack/react-query/build/modern/QueryClientProvider.js
  var React = __toESM(require_react(), 1);
  var import_jsx_runtime = __toESM(require_jsx_runtime(), 1);
  "use client";
  var QueryClientContext = React.createContext(undefined);
  var useQueryClient = (queryClient) => {
    const client = React.useContext(QueryClientContext);
    if (queryClient) {
      return queryClient;
    }
    if (!client) {
      throw new Error("No QueryClient set, use QueryClientProvider to set one");
    }
    return client;
  };

  // ../../node_modules/.bun/@tanstack+react-query@5.101.0+e14d3f224186685e/node_modules/@tanstack/react-query/build/modern/QueryErrorResetBoundary.js
  var React2 = __toESM(require_react(), 1);
  var import_jsx_runtime2 = __toESM(require_jsx_runtime(), 1);
  "use client";
  function createValue() {
    let isReset = false;
    return {
      clearReset: () => {
        isReset = false;
      },
      reset: () => {
        isReset = true;
      },
      isReset: () => {
        return isReset;
      }
    };
  }
  var QueryErrorResetBoundaryContext = React2.createContext(createValue());
  var useQueryErrorResetBoundary = () => React2.useContext(QueryErrorResetBoundaryContext);

  // ../../node_modules/.bun/@tanstack+react-query@5.101.0+e14d3f224186685e/node_modules/@tanstack/react-query/build/modern/errorBoundaryUtils.js
  var React3 = __toESM(require_react(), 1);
  "use client";
  var ensurePreventErrorBoundaryRetry = (options, errorResetBoundary, query) => {
    const throwOnError = query?.state.error && typeof options.throwOnError === "function" ? shouldThrowError(options.throwOnError, [query.state.error, query]) : options.throwOnError;
    if (options.suspense || options.experimental_prefetchInRender || throwOnError) {
      if (!errorResetBoundary.isReset()) {
        options.retryOnMount = false;
      }
    }
  };
  var useClearResetErrorBoundary = (errorResetBoundary) => {
    React3.useEffect(() => {
      errorResetBoundary.clearReset();
    }, [errorResetBoundary]);
  };
  var getHasError = ({
    result,
    errorResetBoundary,
    throwOnError,
    query,
    suspense
  }) => {
    return result.isError && !errorResetBoundary.isReset() && !result.isFetching && query && (suspense && result.data === undefined || shouldThrowError(throwOnError, [result.error, query]));
  };

  // ../../node_modules/.bun/@tanstack+react-query@5.101.0+e14d3f224186685e/node_modules/@tanstack/react-query/build/modern/IsRestoringProvider.js
  var React4 = __toESM(require_react(), 1);
  "use client";
  var IsRestoringContext = React4.createContext(false);
  var useIsRestoring = () => React4.useContext(IsRestoringContext);
  var IsRestoringProvider = IsRestoringContext.Provider;

  // ../../node_modules/.bun/@tanstack+react-query@5.101.0+e14d3f224186685e/node_modules/@tanstack/react-query/build/modern/suspense.js
  var defaultThrowOnError = (_error, query) => query.state.data === undefined;
  var ensureSuspenseTimers = (defaultedOptions) => {
    if (defaultedOptions.suspense) {
      const MIN_SUSPENSE_TIME_MS = 1000;
      const clamp = (value) => value === "static" ? value : Math.max(value ?? MIN_SUSPENSE_TIME_MS, MIN_SUSPENSE_TIME_MS);
      const originalStaleTime = defaultedOptions.staleTime;
      defaultedOptions.staleTime = typeof originalStaleTime === "function" ? (...args) => clamp(originalStaleTime(...args)) : clamp(originalStaleTime);
      if (typeof defaultedOptions.gcTime === "number") {
        defaultedOptions.gcTime = Math.max(defaultedOptions.gcTime, MIN_SUSPENSE_TIME_MS);
      }
    }
  };
  var willFetch = (result, isRestoring) => result.isLoading && result.isFetching && !isRestoring;
  var shouldSuspend = (defaultedOptions, result) => defaultedOptions?.suspense && result.isPending;
  var fetchOptimistic = (defaultedOptions, observer, errorResetBoundary) => observer.fetchOptimistic(defaultedOptions).catch(() => {
    errorResetBoundary.clearReset();
  });

  // ../../node_modules/.bun/@tanstack+react-query@5.101.0+e14d3f224186685e/node_modules/@tanstack/react-query/build/modern/useBaseQuery.js
  "use client";
  function useBaseQuery(options, Observer, queryClient) {
    if (true) {
      if (typeof options !== "object" || Array.isArray(options)) {
        throw new Error('Bad argument type. Starting with v5, only the "Object" form is allowed when calling query related functions. Please use the error stack to find the culprit call. More info here: https://tanstack.com/query/latest/docs/react/guides/migrating-to-v5#supports-a-single-signature-one-object');
      }
    }
    const isRestoring = useIsRestoring();
    const errorResetBoundary = useQueryErrorResetBoundary();
    const client = useQueryClient(queryClient);
    const defaultedOptions = client.defaultQueryOptions(options);
    client.getDefaultOptions().queries?._experimental_beforeQuery?.(defaultedOptions);
    const query = client.getQueryCache().get(defaultedOptions.queryHash);
    if (true) {
      if (!defaultedOptions.queryFn) {
        console.error(`[${defaultedOptions.queryHash}]: No queryFn was passed as an option, and no default queryFn was found. The queryFn parameter is only optional when using a default queryFn. More info here: https://tanstack.com/query/latest/docs/framework/react/guides/default-query-function`);
      }
    }
    const subscribed = options.subscribed !== false;
    defaultedOptions._optimisticResults = isRestoring ? "isRestoring" : subscribed ? "optimistic" : undefined;
    ensureSuspenseTimers(defaultedOptions);
    ensurePreventErrorBoundaryRetry(defaultedOptions, errorResetBoundary, query);
    useClearResetErrorBoundary(errorResetBoundary);
    const isNewCacheEntry = !client.getQueryCache().get(defaultedOptions.queryHash);
    const [observer] = React5.useState(() => new Observer(client, defaultedOptions));
    const result = observer.getOptimisticResult(defaultedOptions);
    const shouldSubscribe = !isRestoring && subscribed;
    React5.useSyncExternalStore(React5.useCallback((onStoreChange) => {
      const unsubscribe = shouldSubscribe ? observer.subscribe(notifyManager.batchCalls(onStoreChange)) : noop;
      observer.updateResult();
      return unsubscribe;
    }, [observer, shouldSubscribe]), () => observer.getCurrentResult(), () => observer.getCurrentResult());
    React5.useEffect(() => {
      observer.setOptions(defaultedOptions);
    }, [defaultedOptions, observer]);
    if (shouldSuspend(defaultedOptions, result)) {
      throw fetchOptimistic(defaultedOptions, observer, errorResetBoundary);
    }
    if (getHasError({
      result,
      errorResetBoundary,
      throwOnError: defaultedOptions.throwOnError,
      query,
      suspense: defaultedOptions.suspense
    })) {
      throw result.error;
    }
    client.getDefaultOptions().queries?._experimental_afterQuery?.(defaultedOptions, result);
    if (defaultedOptions.experimental_prefetchInRender && !environmentManager.isServer() && willFetch(result, isRestoring)) {
      const promise = isNewCacheEntry ? fetchOptimistic(defaultedOptions, observer, errorResetBoundary) : query?.promise;
      promise?.catch(noop).finally(() => {
        observer.updateResult();
      });
    }
    return !defaultedOptions.notifyOnChangeProps ? observer.trackResult(result) : result;
  }

  // ../../node_modules/.bun/@tanstack+react-query@5.101.0+e14d3f224186685e/node_modules/@tanstack/react-query/build/modern/useQuery.js
  "use client";
  function useQuery(options, queryClient) {
    return useBaseQuery(options, QueryObserver, queryClient);
  }

  // ../../node_modules/.bun/@tanstack+react-query@5.101.0+e14d3f224186685e/node_modules/@tanstack/react-query/build/modern/useSuspenseQuery.js
  "use client";
  function useSuspenseQuery(options, queryClient) {
    if (true) {
      if (options.queryFn === skipToken) {
        console.error("skipToken is not allowed for useSuspenseQuery");
      }
    }
    return useBaseQuery({
      ...options,
      enabled: true,
      suspense: true,
      throwOnError: defaultThrowOnError,
      placeholderData: undefined
    }, QueryObserver, queryClient);
  }
  // ../../node_modules/.bun/@tanstack+react-query@5.101.0+e14d3f224186685e/node_modules/@tanstack/react-query/build/modern/useMutation.js
  var React6 = __toESM(require_react(), 1);
  "use client";
  function useMutation(options, queryClient) {
    const client = useQueryClient(queryClient);
    const [observer] = React6.useState(() => new MutationObserver(client, options));
    React6.useEffect(() => {
      observer.setOptions(options);
    }, [observer, options]);
    const result = React6.useSyncExternalStore(React6.useCallback((onStoreChange) => observer.subscribe(notifyManager.batchCalls(onStoreChange)), [observer]), () => observer.getCurrentResult(), () => observer.getCurrentResult());
    const mutate = React6.useCallback((variables, mutateOptions) => {
      observer.mutate(variables, mutateOptions).catch(noop);
    }, [observer]);
    if (result.error && shouldThrowError(observer.options.throwOnError, [result.error])) {
      throw result.error;
    }
    return { ...result, mutate, mutateAsync: result.mutate };
  }
  // ../../node_modules/.bun/@tanstack+react-query@5.101.0+e14d3f224186685e/node_modules/@tanstack/react-query/build/modern/useInfiniteQuery.js
  "use client";
  function useInfiniteQuery(options, queryClient) {
    return useBaseQuery(options, InfiniteQueryObserver, queryClient);
  }
  // ../../node_modules/.bun/openapi-react-query@0.5.4+74bf208874e03033/node_modules/openapi-react-query/dist/index.mjs
  function createClient2(client) {
    const queryFn = async ({
      queryKey: [method, path, init],
      signal
    }) => {
      const mth = method.toUpperCase();
      const fn = client[mth];
      const { data, error, response } = await fn(path, { signal, ...init });
      if (error) {
        throw error;
      }
      if (response.status === 204 || response.headers.get("Content-Length") === "0") {
        return data ?? null;
      }
      return data;
    };
    const queryOptions2 = (method, path, ...[init, options]) => ({
      queryKey: init === undefined ? [method, path] : [method, path, init],
      queryFn,
      ...options
    });
    return {
      queryOptions: queryOptions2,
      useQuery: (method, path, ...[init, options, queryClient]) => useQuery(queryOptions2(method, path, init, options), queryClient),
      useSuspenseQuery: (method, path, ...[init, options, queryClient]) => useSuspenseQuery(queryOptions2(method, path, init, options), queryClient),
      useInfiniteQuery: (method, path, init, options, queryClient) => {
        const { pageParamName = "cursor", ...restOptions } = options;
        const { queryKey } = queryOptions2(method, path, init);
        return useInfiniteQuery({
          queryKey,
          queryFn: async ({ queryKey: [method2, path2, init2], pageParam = 0, signal }) => {
            const mth = method2.toUpperCase();
            const fn = client[mth];
            const mergedInit = {
              ...init2,
              signal,
              params: {
                ...init2?.params || {},
                query: {
                  ...init2?.params?.query,
                  [pageParamName]: pageParam
                }
              }
            };
            const { data, error } = await fn(path2, mergedInit);
            if (error) {
              throw error;
            }
            return data;
          },
          ...restOptions
        }, queryClient);
      },
      useMutation: (method, path, options, queryClient) => useMutation({
        mutationKey: [method, path],
        mutationFn: async (init) => {
          const mth = method.toUpperCase();
          const fn = client[mth];
          const { data, error } = await fn(path, init);
          if (error) {
            throw error;
          }
          return data;
        },
        ...options
      }, queryClient)
    };
  }

  // ../api/src/config.ts
  var DEFAULT_API_BASE_URL = "http://127.0.0.1:3000";
  function normalizeApiBaseUrl(value) {
    const trimmed = value?.trim();
    const candidate = trimmed && trimmed.length > 0 ? trimmed : DEFAULT_API_BASE_URL;
    return candidate.replace(/\/+$/, "");
  }
  function resolveViteApiBaseUrl() {
    return import.meta.env?.VITE_API_BASE_URL;
  }
  function resolveRuntimeApiBaseUrl() {
    if (typeof globalThis === "undefined") {
      return;
    }
    return globalThis.__SLAB_API_BASE_URL__;
  }
  var SERVER_BASE_URL = normalizeApiBaseUrl(resolveRuntimeApiBaseUrl() ?? resolveViteApiBaseUrl());

  // ../api/src/errors.ts
  var UNAUTHORIZED_CODE = 4010;
  var ADMIN_AUTH_MESSAGE = "Admin API authorization failed. Configure server.admin.token or provide the matching bearer token.";

  class ApiError extends Error {
    code;
    status;
    data;
    i18n;
    constructor(code, message, data, status, i18n) {
      super(message);
      this.name = "ApiError";
      this.code = code;
      this.status = status;
      this.data = data;
      this.i18n = i18n;
    }
    static fromResponse(response, errorData) {
      if (response.status === 401) {
        return new ApiError(UNAUTHORIZED_CODE, ADMIN_AUTH_MESSAGE, errorData, response.status);
      }
      if (isApiErrorResponse(errorData)) {
        const payload = errorData;
        return new ApiError(payload.code, payload.message, payload.data, response.status, payload.i18n);
      }
      return new ApiError(response.status * 10, `${response.status} ${response.statusText}`, errorData, response.status);
    }
    isClientError() {
      return this.code >= 4000 && this.code < 5000;
    }
    isServerError() {
      return this.code >= 5000;
    }
    getUserMessage() {
      if (this.status === 401 || this.code === UNAUTHORIZED_CODE) {
        return ADMIN_AUTH_MESSAGE;
      }
      if (this.message && !this.message.includes("error:")) {
        return this.message;
      }
      switch (this.code) {
        case 4000:
          return "Invalid request. Please check your input and try again.";
        case 4004:
          return "The requested resource was not found.";
        case 4009:
          return "The request conflicts with the current state. Refresh and try again.";
        case 4029:
          return "Too many requests. Wait a moment and try again.";
        case 5003:
          return "Backend service is not ready. Please ensure all backends are properly configured.";
        case 5000:
          return "An error occurred while processing your request.";
        case 5001:
          return "A database error occurred. Please try again later.";
        case 5002:
          return "An internal server error occurred. Please try again later.";
        case 5010:
          return "This operation is not implemented yet.";
        default:
          return "An unexpected error occurred. Please try again.";
      }
    }
  }
  var errorMiddleware = {
    async onResponse({ response }) {
      if (response.ok) {
        return;
      }
      if (response.status === 401) {
        throw new ApiError(UNAUTHORIZED_CODE, ADMIN_AUTH_MESSAGE, null, response.status);
      }
      const clonedResponse = response.clone();
      try {
        const errorData = await clonedResponse.json();
        if (isApiErrorResponse(errorData)) {
          throw new ApiError(errorData.code, errorData.message, errorData.data, response.status, errorData.i18n);
        }
        throw new Error(`${response.url}: ${response.status} ${response.statusText}`);
      } catch (error) {
        if (error instanceof ApiError) {
          throw error;
        }
        throw new Error(`${response.url}: ${response.status} ${response.statusText}`, { cause: error });
      }
    },
    async onError({ error }) {
      if (error instanceof Error) {
        return new ApiError(5002, error.message);
      }
      return new Error(String(error));
    }
  };
  function isApiErrorResponse(value) {
    return typeof value === "object" && value !== null && "code" in value && typeof value.code === "number" && "message" in value && typeof value.message === "string";
  }

  // ../api/src/index.ts
  function buildClientConfig(options = {}) {
    return {
      baseUrl: `${normalizeApiBaseUrl(options.baseUrl ?? SERVER_BASE_URL)}/`,
      fetch: options.fetch ?? fetch
    };
  }
  function createSlabApiFetchClient(options = {}) {
    const client = createClient(buildClientConfig(options));
    if (options.getAdminToken) {
      client.use(adminTokenMiddleware(options.getAdminToken));
    }
    if (options.useErrorMiddleware) {
      client.use(errorMiddleware);
    }
    return client;
  }
  function adminTokenMiddleware(getAdminToken) {
    return {
      onRequest({ request }) {
        const token = getAdminToken()?.trim();
        if (token) {
          request.headers.set("Authorization", `Bearer ${token}`);
        }
        return request;
      }
    };
  }
  function createSlabApiQueryHooks(options = {}) {
    return createClient2(createSlabApiFetchClient({
      ...options,
      useErrorMiddleware: options.useErrorMiddleware ?? true
    }));
  }
  var apiClient = createSlabApiFetchClient();
  var api = createSlabApiQueryHooks();

  // src/permissions.ts
  var SLAB_API_PERMISSIONS = {
    modelsRead: "models:read",
    modelsLoad: "models:load",
    ffmpegConvert: "ffmpeg:convert",
    audioTranscribe: "audio:transcribe",
    subtitleRender: "subtitle:render",
    chatComplete: "chat:complete",
    tasksRead: "tasks:read",
    tasksCancel: "tasks:cancel"
  };
  var SLAB_API_PERMISSION_LABELS = {
    [SLAB_API_PERMISSIONS.modelsRead]: {
      title: "Read models",
      description: "List available models and read their metadata.",
      severity: "low"
    },
    [SLAB_API_PERMISSIONS.modelsLoad]: {
      title: "Load models",
      description: "Load (and download) models into the local runtime, which uses disk, memory, and compute.",
      severity: "high"
    },
    [SLAB_API_PERMISSIONS.ffmpegConvert]: {
      title: "Run FFmpeg conversions",
      description: "Convert and process media files through the FFmpeg tool runtime.",
      severity: "medium"
    },
    [SLAB_API_PERMISSIONS.audioTranscribe]: {
      title: "Transcribe audio",
      description: "Run audio transcription, which can consume significant compute for long files.",
      severity: "medium"
    },
    [SLAB_API_PERMISSIONS.subtitleRender]: {
      title: "Render subtitles",
      description: "Render and write subtitle assets to disk.",
      severity: "medium"
    },
    [SLAB_API_PERMISSIONS.chatComplete]: {
      title: "Run chat completions",
      description: "Send prompts to the local model and read generated responses.",
      severity: "high"
    },
    [SLAB_API_PERMISSIONS.tasksRead]: {
      title: "Read tasks",
      description: "Inspect background task status and results.",
      severity: "low"
    },
    [SLAB_API_PERMISSIONS.tasksCancel]: {
      title: "Cancel tasks",
      description: "Cancel running background tasks, including model downloads.",
      severity: "medium"
    }
  };
  var UNKNOWN_PERMISSION_LABEL = {
    title: "Unknown permission",
    description: "This permission is not part of the recognized plugin Slab API surface. Grant it only if you trust the plugin author.",
    severity: "high"
  };
  function isKnownSlabApiPermission(permission) {
    return Object.hasOwn(SLAB_API_PERMISSION_LABELS, permission);
  }
  function describeSlabApiPermission(permission) {
    return isKnownSlabApiPermission(permission) ? SLAB_API_PERMISSION_LABELS[permission] : { ...UNKNOWN_PERMISSION_LABEL, title: `Unknown permission: ${permission}` };
  }
  function requiredSlabApiPermission(method, path) {
    const normalizedMethod = method.toUpperCase();
    const normalizedPath = path.split("?").at(0) ?? path;
    switch (normalizedMethod) {
      case "GET":
        if (pathMatches(normalizedPath, "/v1/models")) {
          return SLAB_API_PERMISSIONS.modelsRead;
        }
        if (pathMatches(normalizedPath, "/v1/tasks")) {
          return SLAB_API_PERMISSIONS.tasksRead;
        }
        return null;
      case "POST":
        if (normalizedPath === "/v1/models/load") {
          return SLAB_API_PERMISSIONS.modelsLoad;
        }
        if (normalizedPath === "/v1/ffmpeg/convert") {
          return SLAB_API_PERMISSIONS.ffmpegConvert;
        }
        if (normalizedPath === "/v1/audio/transcriptions") {
          return SLAB_API_PERMISSIONS.audioTranscribe;
        }
        if (normalizedPath === "/v1/subtitles/render") {
          return SLAB_API_PERMISSIONS.subtitleRender;
        }
        if (normalizedPath === "/v1/chat/completions") {
          return SLAB_API_PERMISSIONS.chatComplete;
        }
        if (normalizedPath.startsWith("/v1/tasks/") && normalizedPath.endsWith("/cancel")) {
          return SLAB_API_PERMISSIONS.tasksCancel;
        }
        return null;
      default:
        return null;
    }
  }
  function assertSlabPluginApiSurface(method, path) {
    const requiredPermission = requiredSlabApiPermission(method, path);
    if (requiredPermission) {
      return requiredPermission;
    }
    throw new Error(`Plugin API request ${method.toUpperCase()} ${path} is not part of the allowed plugin API surface.`);
  }
  function pathMatches(path, base) {
    return path === base || path.startsWith(`${base}/`);
  }

  // src/index.ts
  var SLAB_THEME_TOKENS = [
    "background",
    "foreground",
    "card",
    "card-foreground",
    "popover",
    "popover-foreground",
    "primary",
    "primary-foreground",
    "secondary",
    "secondary-foreground",
    "muted",
    "muted-foreground",
    "accent",
    "accent-foreground",
    "destructive",
    "destructive-foreground",
    "border",
    "input",
    "ring",
    "radius",
    "app-canvas",
    "surface-1",
    "surface-2",
    "surface-soft",
    "surface-selected",
    "surface-input",
    "brand-teal",
    "brand-teal-foreground",
    "brand-gold",
    "success",
    "success-foreground",
    "status-success-bg",
    "status-info-bg",
    "status-danger-bg",
    "status-neutral-bg"
  ];
  var JSON_HEADERS = { "content-type": "application/json" };
  var THEME_EVENT_NAME = "plugin://host/theme";

  class SlabPluginApiError extends Error {
    response;
    data;
    constructor(message, response, data) {
      super(message);
      this.name = "SlabPluginApiError";
      this.response = response;
      this.data = data;
    }
  }
  function resolveWindow(target) {
    return target ?? window;
  }
  function requireCore(target) {
    const core = resolveWindow(target)["__TAURI__"]?.core;
    if (!core || typeof core.invoke !== "function") {
      throw new Error("Slab plugin host bridge is not available in this webview.");
    }
    return core;
  }
  function resolveCore(target) {
    const core = resolveWindow(target)["__TAURI__"]?.core;
    return core && typeof core.invoke === "function" ? core : null;
  }
  function resolveEventApi(target) {
    const eventApi = resolveWindow(target)["__TAURI__"]?.event;
    return eventApi && typeof eventApi.listen === "function" ? eventApi : null;
  }
  function serializeJsonRequest(request) {
    const headers = { ...request.headers };
    let body = null;
    if (request.body !== undefined && request.body !== null) {
      body = typeof request.body === "string" ? request.body : JSON.stringify(request.body);
      const hasContentType = Object.keys(headers).some((name) => name.toLowerCase() === "content-type");
      if (!hasContentType) {
        headers["content-type"] = JSON_HEADERS["content-type"];
      }
    }
    return {
      method: request.method,
      path: request.path,
      headers,
      body,
      timeoutMs: request.timeoutMs
    };
  }
  function fetchPluginApi(request) {
    const endpoint = `${normalizeApiBaseUrl(SERVER_BASE_URL)}${request.path}`;
    const signal = request.timeoutMs ? AbortSignal.timeout(request.timeoutMs) : undefined;
    return fetch(endpoint, {
      method: request.method,
      headers: request.headers,
      body: request.body ?? undefined,
      signal
    });
  }
  async function parseResponseBody(response) {
    const text = await response.text();
    if (!text) {
      return null;
    }
    try {
      return JSON.parse(text);
    } catch {
      return text;
    }
  }
  function extractErrorMessage(data) {
    if (typeof data === "string" && data.trim()) {
      return data;
    }
    if (!data || typeof data !== "object") {
      return null;
    }
    const record = data;
    const nestedError = record.error;
    if (nestedError && typeof nestedError === "object") {
      const message = nestedError.message;
      if (typeof message === "string" && message.trim()) {
        return message;
      }
    }
    if (typeof record.message === "string" && record.message.trim()) {
      return record.message;
    }
    return null;
  }
  function applySlabThemeToDocument(snapshot, targetDocument = document) {
    const root = targetDocument.documentElement;
    root.classList.toggle("dark", snapshot.mode === "dark");
    for (const [token, value] of Object.entries(snapshot.tokens)) {
      if (typeof value === "string" && value.trim().length > 0) {
        root.style.setProperty(`--${token}`, value);
      }
    }
  }
  function createSlabPluginSdk(target) {
    const apiClient2 = createSlabApiFetchClient({ baseUrl: SERVER_BASE_URL });
    apiClient2.use({
      async onRequest({ request }) {
        const url = new URL(request.url);
        assertSlabPluginApiSurface(request.method, `${url.pathname}${url.search}`);
        return request;
      }
    });
    return {
      host: {
        isAvailable: () => Boolean(resolveCore(target)),
        invoke: (command, args) => requireCore(target).invoke(command, args)
      },
      api: {
        client: apiClient2,
        requestJson: async (request) => {
          assertSlabPluginApiSurface(request.method, request.path);
          const response = await fetchPluginApi(serializeJsonRequest(request));
          const data = await parseResponseBody(response);
          if (!response.ok) {
            throw new SlabPluginApiError(extractErrorMessage(data) ?? `Plugin API request failed with HTTP ${response.status}`, response, data);
          }
          return data;
        }
      },
      files: {
        pickVideo: () => requireCore(target).invoke("plugin_pick_file")
      },
      events: {
        listen: async (pluginId, handler) => {
          const eventApi = resolveEventApi(target);
          if (!eventApi) {
            return () => {};
          }
          return eventApi.listen(`plugin://${pluginId}/event`, (event) => handler(event.payload));
        }
      },
      theme: {
        getSnapshot: () => requireCore(target).invoke("plugin_theme_snapshot"),
        subscribe: async (handler) => {
          const eventApi = resolveEventApi(target);
          if (!eventApi) {
            return () => {};
          }
          return eventApi.listen(THEME_EVENT_NAME, (event) => handler(event.payload));
        },
        applyToDocument: (snapshot, targetDocument) => {
          const resolvedDocument = targetDocument ?? target?.document ?? document;
          applySlabThemeToDocument(snapshot, resolvedDocument);
        }
      }
    };
  }
  function getSlabPluginSdk(target) {
    return createSlabPluginSdk(target);
  }
  function mountPluginUI(pluginId, entry, container) {
    const targetWindow = resolveWindow(container.ownerDocument.defaultView ?? window);
    const tauriCore = targetWindow["__TAURI__"]?.core;
    const hasTrustedTauriContext = Boolean(targetWindow["__TAURI_INTERNALS__"]);
    if (hasTrustedTauriContext && tauriCore && typeof tauriCore.invoke === "function") {
      const bounds = container.getBoundingClientRect();
      const handle = { kind: "tauri", pluginId, _targetWindow: targetWindow };
      tauriCore.invoke("plugin_mount_view", {
        request: {
          pluginId,
          bounds: {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height
          }
        }
      }).then((response) => {
        if (handle.kind === "tauri") {
          handle.webviewLabel = response.webviewLabel;
        }
      });
      return handle;
    }
    const iframe = container.ownerDocument.createElement("iframe");
    iframe.setAttribute("sandbox", "allow-scripts allow-forms");
    iframe.src = entry;
    iframe.style.width = "100%";
    iframe.style.height = "100%";
    iframe.style.border = "0";
    container.appendChild(iframe);
    return { kind: "browser", pluginId, iframe };
  }
  function unmountPluginUI(handle) {
    if (handle.kind === "tauri") {
      const targetWindow = handle._targetWindow;
      const tauriCore = targetWindow["__TAURI__"]?.core;
      const hasTrustedTauriContext = Boolean(targetWindow["__TAURI_INTERNALS__"]);
      if (hasTrustedTauriContext && tauriCore && typeof tauriCore.invoke === "function") {
        tauriCore.invoke("plugin_unmount_view", { request: { pluginId: handle.pluginId } });
      }
      return;
    }
    handle.iframe.remove();
  }
})();
