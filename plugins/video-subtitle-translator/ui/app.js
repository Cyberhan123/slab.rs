(function () {
  "use strict";

  const PLUGIN_ID = "video-subtitle-translator";
  const TASK_DONE = new Set(["succeeded"]);
  const TASK_FAILED = new Set(["failed", "cancelled", "interrupted"]);
  const JSON_HEADERS = { "content-type": "application/json" };

  const state = {
    videoPath: "",
    running: false,
    cancelRequested: false,
    activeTaskId: "",
    sourceSegments: [],
    translatedSegments: [],
    modelMetaById: new Map(),
  };

  const el = {
    runStatus: document.getElementById("runStatus"),
    pickVideo: document.getElementById("pickVideo"),
    videoPath: document.getElementById("videoPath"),
    refreshModels: document.getElementById("refreshModels"),
    whisperModel: document.getElementById("whisperModel"),
    sourceLanguage: document.getElementById("sourceLanguage"),
    detectLanguage: document.getElementById("detectLanguage"),
    vadEnabled: document.getElementById("vadEnabled"),
    vadModel: document.getElementById("vadModel"),
    vadModelPath: document.getElementById("vadModelPath"),
    vadThreshold: document.getElementById("vadThreshold"),
    minSilence: document.getElementById("minSilence"),
    speechPad: document.getElementById("speechPad"),
    translateEnabled: document.getElementById("translateEnabled"),
    targetLanguage: document.getElementById("targetLanguage"),
    llamaModel: document.getElementById("llamaModel"),
    batchSize: document.getElementById("batchSize"),
    maxTokens: document.getElementById("maxTokens"),
    runPipeline: document.getElementById("runPipeline"),
    cancelPipeline: document.getElementById("cancelPipeline"),
    audioOutput: document.getElementById("audioOutput"),
    sourceOutput: document.getElementById("sourceOutput"),
    translatedOutput: document.getElementById("translatedOutput"),
    logList: document.getElementById("logList"),
    clearLog: document.getElementById("clearLog"),
    sourcePreview: document.getElementById("sourcePreview"),
    translatedPreview: document.getElementById("translatedPreview"),
    sourceCount: document.getElementById("sourceCount"),
    translatedCount: document.getElementById("translatedCount"),
  };

  function invoke(command, args) {
    const core = window.__TAURI__ && window.__TAURI__.core;
    if (!core || typeof core.invoke !== "function") {
      throw new Error("Tauri invoke bridge is not available in this webview.");
    }
    return core.invoke(command, args);
  }

  function setStatus(label, kind) {
    el.runStatus.textContent = label;
    el.runStatus.className = "status-pill";
    if (kind) {
      el.runStatus.classList.add(kind);
    }
  }

  function setRunning(running) {
    state.running = running;
    el.runPipeline.disabled = running;
    el.cancelPipeline.disabled = !running;
    el.pickVideo.disabled = running;
    el.refreshModels.disabled = running;
  }

  function log(message, kind) {
    const li = document.createElement("li");
    li.textContent = message;
    if (kind) {
      li.className = kind;
    }
    el.logList.appendChild(li);
    el.logList.scrollTop = el.logList.scrollHeight;
  }

  function resetOutputs() {
    state.sourceSegments = [];
    state.translatedSegments = [];
    el.audioOutput.textContent = "Not generated";
    el.sourceOutput.textContent = "Not generated";
    el.translatedOutput.textContent = "Not generated";
    renderPreview("source", []);
    renderPreview("translated", []);
  }

  function renderPreview(kind, segments) {
    const lines = segments.slice(0, 10).map((segment, index) => {
      const id = segment.id == null ? index + 1 : segment.id;
      return `${id}. [${segment.start_ms} -> ${segment.end_ms}] ${segment.text}`;
    });
    const preview = lines.length > 0 ? lines.join("\n") : kind === "source" ? "No transcript yet." : "Translation is optional.";
    if (kind === "source") {
      el.sourcePreview.textContent = preview;
      el.sourceCount.textContent = `${segments.length} segments`;
    } else {
      el.translatedPreview.textContent = preview;
      el.translatedCount.textContent = `${segments.length} segments`;
    }
  }

  async function apiRequest(method, path, body, timeoutMs) {
    const response = await invoke("plugin_api_request", {
      request: {
        method,
        path,
        headers: body === undefined ? {} : JSON_HEADERS,
        body: body === undefined ? null : JSON.stringify(body),
        timeoutMs: timeoutMs || 60000,
      },
    });

    let data = null;
    if (response.body) {
      try {
        data = JSON.parse(response.body);
      } catch {
        data = response.body;
      }
    }

    if (response.status < 200 || response.status >= 300) {
      const message = extractErrorMessage(data) || `API request failed with HTTP ${response.status}`;
      throw new Error(message);
    }

    return data;
  }

  function extractErrorMessage(data) {
    if (!data) return "";
    if (typeof data === "string") return data;
    if (data.error && typeof data.error.message === "string") return data.error.message;
    if (typeof data.message === "string") return data.message;
    return "";
  }

  function selectedValue(select) {
    const value = select.value.trim();
    return value.length > 0 ? value : null;
  }

  function numberValue(input, fallback) {
    const value = Number(input.value);
    return Number.isFinite(value) ? value : fallback;
  }

  function buildModelOption(model) {
    const option = document.createElement("option");
    option.value = model.id;
    option.textContent = model.display_name ? `${model.display_name} (${model.id})` : model.id;
    return option;
  }

  async function loadModels() {
    log("Loading model lists...");
    const [whisperModels, vadModels, chatModels] = await Promise.all([
      apiRequest("GET", "/v1/models?capability=audio_transcription"),
      apiRequest("GET", "/v1/models?capability=audio_vad"),
      apiRequest("GET", "/v1/models?capability=chat_generation"),
    ]);

    state.modelMetaById.clear();
    for (const model of [...whisperModels, ...vadModels, ...chatModels]) {
      state.modelMetaById.set(model.id, model);
    }

    replaceOptions(el.whisperModel, "Use active/default Whisper model", whisperModels);
    replaceOptions(el.vadModel, "Select or enter VAD model path below", vadModels);
    replaceOptions(el.llamaModel, "Use active/default chat model", chatModels);
    log("Model lists refreshed.", "ok");
  }

  function replaceOptions(select, placeholder, models) {
    const current = select.value;
    select.innerHTML = "";
    const empty = document.createElement("option");
    empty.value = "";
    empty.textContent = placeholder;
    select.appendChild(empty);

    for (const model of Array.isArray(models) ? models : []) {
      select.appendChild(buildModelOption(model));
    }

    if ([...select.options].some((option) => option.value === current)) {
      select.value = current;
    }
  }

  function localModelPath(modelId) {
    if (!modelId) return "";
    const model = state.modelMetaById.get(modelId);
    return model && model.spec && typeof model.spec.local_path === "string" ? model.spec.local_path : "";
  }

  async function pickVideo() {
    const response = await invoke("plugin_pick_file");
    if (!response || !response.path) {
      log("Video selection cancelled.");
      return;
    }
    state.videoPath = response.path;
    el.videoPath.textContent = response.path;
    resetOutputs();
    log(`Selected video: ${response.path}`, "ok");
  }

  async function submitTask(label, method, path, body) {
    assertNotCancelled();
    log(`${label}: submitting task...`);
    const accepted = await apiRequest(method, path, body);
    const taskId = accepted && (accepted.operation_id || accepted.task_id || accepted.id);
    if (!taskId) {
      throw new Error(`${label}: response did not include operation_id.`);
    }
    const result = await waitForTask(label, taskId);
    state.activeTaskId = "";
    return result;
  }

  async function waitForTask(label, taskId) {
    state.activeTaskId = taskId;
    let delayMs = 800;

    while (true) {
      assertNotCancelled();
      // eslint-disable-next-line no-await-in-loop
      const task = await apiRequest("GET", `/v1/tasks/${encodeURIComponent(taskId)}`);
      const status = String(task.status || "").toLowerCase();
      const progress = formatProgress(task.progress);
      log(`${label}: ${status || "unknown"}${progress}`);

      if (TASK_DONE.has(status)) {
        return apiRequest("GET", `/v1/tasks/${encodeURIComponent(taskId)}/result`);
      }

      if (TASK_FAILED.has(status)) {
        throw new Error(`${label} failed: ${task.error_msg || status}`);
      }

      // eslint-disable-next-line no-await-in-loop
      await sleep(delayMs);
      delayMs = Math.min(delayMs + 350, 2500);
    }
  }

  function formatProgress(progress) {
    if (!progress) return "";
    const parts = [];
    if (progress.label) parts.push(progress.label);
    if (typeof progress.current === "number" && typeof progress.total === "number" && progress.total > 0) {
      parts.push(`${progress.current}/${progress.total}${progress.unit ? ` ${progress.unit}` : ""}`);
    }
    if (typeof progress.step === "number" && typeof progress.step_count === "number") {
      parts.push(`step ${progress.step}/${progress.step_count}`);
    }
    return parts.length > 0 ? ` (${parts.join(", ")})` : "";
  }

  async function runPipeline() {
    if (!state.videoPath) {
      throw new Error("Choose a video file first.");
    }

    resetOutputs();
    setRunning(true);
    state.cancelRequested = false;
    setStatus("Running", "running");

    try {
      const audioResult = await submitTask("FFmpeg", "POST", "/v1/ffmpeg/convert", {
        source_path: state.videoPath,
        output_format: "wav",
      });
      const audioPath = requireString(audioResult.output_path, "FFmpeg result did not include output_path.");
      el.audioOutput.textContent = audioPath;
      log(`FFmpeg output: ${audioPath}`, "ok");

      const whisperResult = await submitTask("Whisper", "POST", "/v1/audio/transcriptions", buildWhisperRequest(audioPath));
      const segments = normalizeSegments(whisperResult.segments);
      if (segments.length === 0) {
        throw new Error("Whisper completed but returned no timed segments. Enable timestamps/VAD and try again.");
      }
      state.sourceSegments = segments;
      renderPreview("source", segments);
      log(`Whisper produced ${segments.length} timed segments.`, "ok");

      const sourceSrt = await renderSubtitle("source", segments);
      el.sourceOutput.textContent = sourceSrt.output_path;
      log(`Source SRT written: ${sourceSrt.output_path}`, "ok");

      if (el.translateEnabled.checked) {
        const translated = await translateSegments(segments);
        state.translatedSegments = translated;
        renderPreview("translated", translated);
        const translatedSrt = await renderSubtitle("translated", translated);
        el.translatedOutput.textContent = translatedSrt.output_path;
        log(`Translated SRT written: ${translatedSrt.output_path}`, "ok");
      } else {
        el.translatedOutput.textContent = "Skipped";
        log("Translation skipped.");
      }

      setStatus("Done", "done");
      log("Pipeline completed.", "ok");
    } catch (error) {
      setStatus("Failed", "failed");
      log(error instanceof Error ? error.message : String(error), "error");
      throw error;
    } finally {
      state.activeTaskId = "";
      setRunning(false);
    }
  }

  function buildWhisperRequest(audioPath) {
    const language = el.detectLanguage.checked ? null : el.sourceLanguage.value.trim();
    const request = {
      model_id: selectedValue(el.whisperModel),
      path: audioPath,
      language: language || null,
      detect_language: el.detectLanguage.checked,
      decode: {
        no_timestamps: false,
        token_timestamps: false,
      },
    };

    if (el.vadEnabled.checked) {
      const selectedVadPath = localModelPath(selectedValue(el.vadModel));
      const modelPath = el.vadModelPath.value.trim() || selectedVadPath;
      if (!modelPath) {
        throw new Error("VAD is enabled, but no VAD model path is selected or entered.");
      }
      request.vad = {
        enabled: true,
        model_path: modelPath,
        threshold: numberValue(el.vadThreshold, 0.5),
        min_silence_duration_ms: Math.round(numberValue(el.minSilence, 500)),
        speech_pad_ms: Math.round(numberValue(el.speechPad, 200)),
      };
    }

    return pruneNulls(request);
  }

  async function renderSubtitle(variant, segments) {
    assertNotCancelled();
    return apiRequest("POST", "/v1/subtitles/render", {
      source_path: state.videoPath,
      variant,
      format: "srt",
      entries: segments.map((segment) => ({
        start_ms: segment.start_ms,
        end_ms: segment.end_ms,
        text: segment.text,
      })),
      output_path: null,
      overwrite: true,
    });
  }

  function normalizeSegments(rawSegments) {
    if (!Array.isArray(rawSegments)) return [];
    return rawSegments
      .map((segment, index) => ({
        id: index + 1,
        start_ms: asNonNegativeInteger(segment.start_ms),
        end_ms: asNonNegativeInteger(segment.end_ms),
        text: typeof segment.text === "string" ? segment.text.trim() : "",
      }))
      .filter((segment) => Number.isInteger(segment.start_ms) && Number.isInteger(segment.end_ms) && segment.end_ms > segment.start_ms && segment.text);
  }

  async function translateSegments(segments) {
    const model = selectedValue(el.llamaModel) || "";
    const targetLanguage = el.targetLanguage.value.trim();
    if (!targetLanguage) {
      throw new Error("Target language is required when translation is enabled.");
    }

    const batchSize = Math.max(1, Math.min(50, Math.round(numberValue(el.batchSize, 20))));
    const translated = [];
    for (let offset = 0; offset < segments.length; offset += batchSize) {
      assertNotCancelled();
      const batch = segments.slice(offset, offset + batchSize);
      const batchNumber = Math.floor(offset / batchSize) + 1;
      log(`Llama: translating batch ${batchNumber} (${batch.length} segments)...`);
      // eslint-disable-next-line no-await-in-loop
      const translatedBatch = await translateBatchWithRetry(batch, targetLanguage, model);
      translated.push(...translatedBatch);
    }
    log(`Llama translated ${translated.length} segments.`, "ok");
    return translated;
  }

  async function translateBatchWithRetry(batch, targetLanguage, model) {
    let lastError = null;
    for (let attempt = 1; attempt <= 3; attempt += 1) {
      try {
        // eslint-disable-next-line no-await-in-loop
        return await translateBatch(batch, targetLanguage, model);
      } catch (error) {
        lastError = error;
        log(`Translation attempt ${attempt} failed: ${error instanceof Error ? error.message : String(error)}`, "error");
        if (attempt < 3) {
          // eslint-disable-next-line no-await-in-loop
          await sleep(600 * attempt);
        }
      }
    }
    throw lastError || new Error("Translation failed.");
  }

  async function translateBatch(batch, targetLanguage, model) {
    const payload = batch.map((segment) => ({
      id: segment.id,
      start_ms: segment.start_ms,
      end_ms: segment.end_ms,
      text: segment.text,
    }));

    const schema = {
      type: "object",
      additionalProperties: false,
      properties: {
        segments: {
          type: "array",
          items: {
            type: "object",
            additionalProperties: false,
            properties: {
              id: { type: "integer" },
              start_ms: { type: "integer" },
              end_ms: { type: "integer" },
              text: { type: "string" },
            },
            required: ["id", "start_ms", "end_ms", "text"],
          },
        },
      },
      required: ["segments"],
    };

    const response = await apiRequest("POST", "/v1/chat/completions", {
      model,
      stream: false,
      temperature: 0.1,
      max_tokens: Math.round(numberValue(el.maxTokens, 2048)),
      response_format: {
        type: "json_schema",
        json_schema: {
          name: "subtitle_translation_batch",
          strict: true,
          schema,
        },
      },
      messages: [
        {
          role: "system",
          content: [
            "You translate subtitle segments.",
            `Translate only the text field into ${targetLanguage}.`,
            "Preserve id, start_ms, and end_ms exactly.",
            "Return only JSON matching the requested schema.",
          ].join(" "),
        },
        {
          role: "user",
          content: JSON.stringify({ segments: payload }),
        },
      ],
    });

    const text = extractAssistantText(response);
    const parsed = parseTranslationJson(text);
    return validateTranslatedBatch(batch, parsed);
  }

  function extractAssistantText(response) {
    const choice = response && response.choices && response.choices[0];
    const content = choice && choice.message && choice.message.content;
    if (typeof content === "string") return content;
    if (Array.isArray(content)) {
      return content
        .map((part) => {
          if (part && typeof part.text === "string") return part.text;
          if (part && part.value !== undefined) return JSON.stringify(part.value);
          return "";
        })
        .join("");
    }
    throw new Error("Llama response did not include assistant text.");
  }

  function parseTranslationJson(text) {
    const trimmed = text.trim();
    try {
      const parsed = JSON.parse(trimmed);
      return Array.isArray(parsed) ? parsed : parsed.segments;
    } catch (error) {
      const firstBrace = trimmed.indexOf("{");
      const lastBrace = trimmed.lastIndexOf("}");
      if (firstBrace >= 0 && lastBrace > firstBrace) {
        const parsed = JSON.parse(trimmed.slice(firstBrace, lastBrace + 1));
        return Array.isArray(parsed) ? parsed : parsed.segments;
      }
      const firstBracket = trimmed.indexOf("[");
      const lastBracket = trimmed.lastIndexOf("]");
      if (firstBracket >= 0 && lastBracket > firstBracket) {
        return JSON.parse(trimmed.slice(firstBracket, lastBracket + 1));
      }
      throw new Error("Llama response was not valid JSON.", { cause: error });
    }
  }

  function validateTranslatedBatch(sourceBatch, translatedBatch) {
    if (!Array.isArray(translatedBatch)) {
      throw new Error("Translation JSON must be an array or { segments: [...] }.");
    }
    if (translatedBatch.length !== sourceBatch.length) {
      throw new Error(`Translation batch length mismatch: expected ${sourceBatch.length}, got ${translatedBatch.length}.`);
    }

    return translatedBatch.map((item, index) => {
      const source = sourceBatch[index];
      if (item.id !== source.id || item.start_ms !== source.start_ms || item.end_ms !== source.end_ms) {
        throw new Error(`Translation segment ${index + 1} changed id or timestamps.`);
      }
      if (typeof item.text !== "string" || !item.text.trim()) {
        throw new Error(`Translation segment ${item.id} has empty text.`);
      }
      return {
        id: source.id,
        start_ms: source.start_ms,
        end_ms: source.end_ms,
        text: item.text.trim(),
      };
    });
  }

  async function cancelPipeline() {
    state.cancelRequested = true;
    setStatus("Cancelling", "failed");
    if (state.activeTaskId) {
      try {
        await apiRequest("POST", `/v1/tasks/${encodeURIComponent(state.activeTaskId)}/cancel`, {});
        log(`Cancel requested for task ${state.activeTaskId}.`);
      } catch (error) {
        log(`Task cancel request failed: ${error instanceof Error ? error.message : String(error)}`, "error");
      }
    }
  }

  function assertNotCancelled() {
    if (state.cancelRequested) {
      throw new Error("Pipeline cancelled.");
    }
  }

  function asNonNegativeInteger(value) {
    if (typeof value !== "number" || !Number.isFinite(value) || value < 0) return null;
    return Math.round(value);
  }

  function requireString(value, message) {
    if (typeof value !== "string" || value.trim().length === 0) {
      throw new Error(message);
    }
    return value;
  }

  function pruneNulls(value) {
    if (Array.isArray(value)) {
      return value.map(pruneNulls);
    }
    if (value && typeof value === "object") {
      return Object.fromEntries(
        Object.entries(value)
          .filter((entry) => entry[1] !== null && entry[1] !== undefined && entry[1] !== "")
          .map(([key, child]) => [key, pruneNulls(child)]),
      );
    }
    return value;
  }

  function sleep(ms) {
    return new Promise((resolve) => {
      window.setTimeout(resolve, ms);
    });
  }

  el.pickVideo.addEventListener("click", () => {
    pickVideo().catch((error) => {
      log(error instanceof Error ? error.message : String(error), "error");
    });
  });

  el.refreshModels.addEventListener("click", () => {
    loadModels().catch((error) => {
      log(error instanceof Error ? error.message : String(error), "error");
    });
  });

  el.vadModel.addEventListener("change", () => {
    const path = localModelPath(selectedValue(el.vadModel));
    if (path) {
      el.vadModelPath.value = path;
    }
  });

  el.detectLanguage.addEventListener("change", () => {
    el.sourceLanguage.disabled = el.detectLanguage.checked;
  });

  el.clearLog.addEventListener("click", () => {
    el.logList.innerHTML = "";
  });

  el.cancelPipeline.addEventListener("click", () => {
    cancelPipeline().catch((error) => {
      log(error instanceof Error ? error.message : String(error), "error");
    });
  });

  el.runPipeline.addEventListener("click", () => {
    runPipeline().catch(() => {
      // Error is already rendered in the step log.
    });
  });

  el.sourceLanguage.disabled = el.detectLanguage.checked;
  setStatus("Idle");
  loadModels().catch((error) => {
    log(`Model list unavailable: ${error instanceof Error ? error.message : String(error)}`, "error");
  });

  window.__SLAB_VIDEO_SUBTITLE_TRANSLATOR__ = {
    pluginId: PLUGIN_ID,
    runPipeline,
    loadModels,
  };
})();
