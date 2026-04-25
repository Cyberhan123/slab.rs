import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Checkbox,
  Input,
  Label,
  Progress,
  ScrollArea,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Separator,
  Spinner,
  Switch,
  Textarea,
  cn,
} from "@slab/plugin-ui";
import { getSlabPluginSdk } from "@slab/plugin-sdk";

const PLUGIN_ID = "video-subtitle-translator";
const TASK_DONE = new Set(["succeeded"]);
const TASK_FAILED = new Set(["failed", "cancelled", "interrupted"]);
const EMPTY_SELECT_VALUE = "__slab_default__";

type ModelInfo = {
  id: string;
  display_name?: string;
  spec?: {
    local_path?: string;
  };
};

type Segment = {
  id: number;
  start_ms: number;
  end_ms: number;
  text: string;
};

type LogEntry = {
  id: number;
  message: string;
  kind?: "ok" | "error";
};

type PipelineStepId = "ffmpeg" | "whisper" | "source-srt" | "translate" | "translated-srt";
type PipelineStepStatus = "pending" | "running" | "done" | "failed" | "skipped";

type PipelineStep = {
  id: PipelineStepId;
  label: string;
  status: PipelineStepStatus;
  detail?: string;
  progress?: TaskProgress | null;
};

type TaskAccepted = {
  operation_id?: string;
  task_id?: string;
  id?: string;
};

type TaskProgress = {
  label?: string;
  message?: string;
  current?: number;
  total?: number;
  unit?: string;
  step?: number;
  step_count?: number;
  logs?: string[];
};

type TaskState = {
  status?: string;
  progress?: TaskProgress;
  error_msg?: string;
};

type RenderSubtitleResponse = {
  output_path?: string;
};

type FfmpegResponse = {
  output_path?: string;
};

type WhisperResponse = {
  segments?: unknown[];
};

type ChatResponse = {
  choices?: Array<{
    message?: {
      content?: string | Array<{ text?: string; value?: unknown }>;
    };
  }>;
};

declare global {
  interface Window {
    __SLAB_VIDEO_SUBTITLE_TRANSLATOR__?: {
      pluginId: string;
      runPipeline: () => Promise<void>;
      loadModels: () => Promise<void>;
    };
  }
}

const sdk = getSlabPluginSdk(window);

export function App() {
  const [videoPath, setVideoPath] = useState("");
  const [running, setRunning] = useState(false);
  const [activeTaskId, setActiveTaskId] = useState("");
  const [status, setStatus] = useState<"idle" | "running" | "failed" | "done" | "cancelling">(
    "idle",
  );
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [whisperModels, setWhisperModels] = useState<ModelInfo[]>([]);
  const [vadModels, setVadModels] = useState<ModelInfo[]>([]);
  const [chatModels, setChatModels] = useState<ModelInfo[]>([]);
  const [whisperModel, setWhisperModel] = useState("");
  const [vadModel, setVadModel] = useState("");
  const [llamaModel, setLlamaModel] = useState("");
  const [sourceLanguage, setSourceLanguage] = useState("");
  const [detectLanguage, setDetectLanguage] = useState(true);
  const [vadEnabled, setVadEnabled] = useState(true);
  const [vadModelPath, setVadModelPath] = useState("");
  const [vadThreshold, setVadThreshold] = useState("0.5");
  const [minSilence, setMinSilence] = useState("500");
  const [speechPad, setSpeechPad] = useState("200");
  const [translateEnabled, setTranslateEnabled] = useState(false);
  const [targetLanguage, setTargetLanguage] = useState("Chinese");
  const [batchSize, setBatchSize] = useState("20");
  const [maxTokens, setMaxTokens] = useState("2048");
  const [audioOutput, setAudioOutput] = useState("Not generated");
  const [sourceOutput, setSourceOutput] = useState("Not generated");
  const [translatedOutput, setTranslatedOutput] = useState("Not generated");
  const [sourceSegments, setSourceSegments] = useState<Segment[]>([]);
  const [translatedSegments, setTranslatedSegments] = useState<Segment[]>([]);
  const [pipelineSteps, setPipelineSteps] = useState<PipelineStep[]>(() =>
    createPipelineSteps(false),
  );
  const cancelRequestedRef = useRef(false);
  const activeTaskIdRef = useRef("");
  const taskLogOffsetsRef = useRef(new Map<string, number>());

  const modelMetaById = useMemo(() => {
    const rows = new Map<string, ModelInfo>();
    for (const model of [...whisperModels, ...vadModels, ...chatModels]) {
      rows.set(model.id, model);
    }
    return rows;
  }, [chatModels, vadModels, whisperModels]);

  const addLog = (message: string, kind?: LogEntry["kind"]) => {
    setLogs((current) => [...current, { id: Date.now() + current.length, message, kind }]);
  };

  const addLogs = (messages: string[], kind?: LogEntry["kind"]) => {
    if (messages.length === 0) {
      return;
    }
    setLogs((current) => [
      ...current,
      ...messages.map((message, index) => ({
        id: Date.now() + current.length + index,
        message,
        kind,
      })),
    ]);
  };

  const updatePipelineStep = (id: PipelineStepId, patch: Partial<PipelineStep>) => {
    setPipelineSteps((current) =>
      current.map((step) => (step.id === id ? { ...step, ...patch } : step)),
    );
  };

  const resetOutputs = () => {
    setSourceSegments([]);
    setTranslatedSegments([]);
    setAudioOutput("Not generated");
    setSourceOutput("Not generated");
    setTranslatedOutput("Not generated");
  };

  const apiRequest = async <T,>(
    method: string,
    path: string,
    body?: unknown,
    timeoutMs = 60_000,
  ): Promise<T> => {
    return sdk.api.requestJson<T>({ method, path, body, timeoutMs });
  };

  const localModelPath = useCallback(
    (modelId: string) => {
      if (!modelId) {
        return "";
      }
      return modelMetaById.get(modelId)?.spec?.local_path ?? "";
    },
    [modelMetaById],
  );

  const loadModels = async () => {
    addLog("Loading model lists...");
    const [nextWhisperModels, nextVadModels, nextChatModels] = await Promise.all([
      apiRequest<ModelInfo[]>("GET", "/v1/models?capability=audio_transcription"),
      apiRequest<ModelInfo[]>("GET", "/v1/models?capability=audio_vad"),
      apiRequest<ModelInfo[]>("GET", "/v1/models?capability=chat_generation"),
    ]);

    setWhisperModels(Array.isArray(nextWhisperModels) ? nextWhisperModels : []);
    setVadModels(Array.isArray(nextVadModels) ? nextVadModels : []);
    setChatModels(Array.isArray(nextChatModels) ? nextChatModels : []);
    addLog("Model lists refreshed.", "ok");
  };

  const pickVideo = async () => {
    const response = await sdk.files.pickVideo();
    if (!response.path) {
      addLog("Video selection cancelled.");
      return;
    }
    setVideoPath(response.path);
    resetOutputs();
    addLog(`Selected video: ${response.path}`, "ok");
  };

  const appendTaskLogs = (label: string, taskId: string, progress?: TaskProgress) => {
    const progressLogs = Array.isArray(progress?.logs) ? progress.logs : [];
    if (progressLogs.length === 0) {
      return;
    }

    const previousOffset = taskLogOffsetsRef.current.get(taskId) ?? 0;
    const nextOffset = previousOffset > progressLogs.length ? 0 : previousOffset;
    const nextLogs = progressLogs.slice(nextOffset).map((line) => `${label}: ${line}`);
    taskLogOffsetsRef.current.set(taskId, progressLogs.length);
    addLogs(nextLogs);
  };

  const submitTask = async <T,>(
    stepId: PipelineStepId,
    label: string,
    method: string,
    path: string,
    body: unknown,
  ) => {
    assertNotCancelled(cancelRequestedRef.current);
    addLog(`${label}: submitting task...`);
    updatePipelineStep(stepId, {
      status: "running",
      detail: "Submitting task",
      progress: null,
    });
    const accepted = await apiRequest<TaskAccepted>(method, path, body);
    const taskId = accepted.operation_id ?? accepted.task_id ?? accepted.id;
    if (!taskId) {
      throw new Error(`${label}: response did not include operation_id.`);
    }
    updatePipelineStep(stepId, {
      status: "running",
      detail: `Task ${taskId}`,
      progress: null,
    });
    const result = await waitForTask<T>(stepId, label, taskId);
    activeTaskIdRef.current = "";
    setActiveTaskId("");
    return result;
  };

  const waitForTask = async <T,>(stepId: PipelineStepId, label: string, taskId: string) => {
    activeTaskIdRef.current = taskId;
    setActiveTaskId(taskId);
    const pollTask = async (delayMs: number): Promise<T> => {
      assertNotCancelled(cancelRequestedRef.current);
      const task = await apiRequest<TaskState>("GET", `/v1/tasks/${encodeURIComponent(taskId)}`);
      const taskStatus = String(task.status ?? "").toLowerCase();
      appendTaskLogs(label, taskId, task.progress);
      updatePipelineStep(stepId, {
        status: TASK_FAILED.has(taskStatus) ? "failed" : "running",
        detail: formatTaskDetail(taskStatus, task.progress),
        progress: task.progress ?? null,
      });
      addLog(`${label}: ${taskStatus || "unknown"}${formatProgress(task.progress)}`);

      if (TASK_DONE.has(taskStatus)) {
        updatePipelineStep(stepId, {
          status: "done",
          detail: "Completed",
          progress: completeTaskProgress(task.progress),
        });
        return apiRequest<T>("GET", `/v1/tasks/${encodeURIComponent(taskId)}/result`);
      }

      if (TASK_FAILED.has(taskStatus)) {
        updatePipelineStep(stepId, {
          status: "failed",
          detail: task.error_msg || taskStatus,
          progress: task.progress ?? null,
        });
        throw new Error(`${label} failed: ${task.error_msg || taskStatus}`);
      }

      await sleep(delayMs);
      return pollTask(Math.min(delayMs + 350, 2_500));
    };

    return pollTask(800);
  };

  const buildWhisperRequest = (audioPath: string) => {
    const language = detectLanguage ? null : sourceLanguage.trim();
    const request: Record<string, unknown> = {
      model_id: selectedValue(whisperModel),
      path: audioPath,
      language: language || null,
      detect_language: detectLanguage,
      decode: {
        no_timestamps: false,
        token_timestamps: false,
      },
    };

    if (vadEnabled) {
      const selectedVadPath = localModelPath(selectedValue(vadModel) ?? "");
      const modelPath = vadModelPath.trim() || selectedVadPath;
      if (!modelPath) {
        throw new Error("VAD is enabled, but no VAD model path is selected or entered.");
      }
      request.vad = {
        enabled: true,
        model_path: modelPath,
        threshold: numberValue(vadThreshold, 0.5),
        min_silence_duration_ms: Math.round(numberValue(minSilence, 500)),
        speech_pad_ms: Math.round(numberValue(speechPad, 200)),
      };
    }

    return pruneNulls(request);
  };

  const renderSubtitle = async (variant: string, segments: Segment[]) => {
    assertNotCancelled(cancelRequestedRef.current);
    return apiRequest<RenderSubtitleResponse>("POST", "/v1/subtitles/render", {
      source_path: videoPath,
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
  };

  const translateSegments = async (segments: Segment[]) => {
    const model = selectedValue(llamaModel) ?? "";
    const language = targetLanguage.trim();
    if (!language) {
      throw new Error("Target language is required when translation is enabled.");
    }

    const size = Math.max(1, Math.min(50, Math.round(numberValue(batchSize, 20))));
    const batchCount = Math.max(1, Math.ceil(segments.length / size));
    updatePipelineStep("translate", {
      status: "running",
      detail: `Batch 0/${batchCount}`,
      progress: {
        label: "Llama translation",
        current: 0,
        total: batchCount,
        unit: "batch",
      },
    });
    const translateRemaining = async (offset: number, translated: Segment[]): Promise<Segment[]> => {
      if (offset >= segments.length) {
        return translated;
      }
      assertNotCancelled(cancelRequestedRef.current);
      const batch = segments.slice(offset, offset + size);
      const batchNumber = Math.floor(offset / size) + 1;
      addLog(`Llama: translating batch ${batchNumber} (${batch.length} segments)...`);
      const nextBatch = await translateBatchWithRetry(batch, language, model);
      updatePipelineStep("translate", {
        status: "running",
        detail: `Batch ${Math.min(batchNumber, batchCount)}/${batchCount}`,
        progress: {
          label: "Llama translation",
          current: Math.min(batchNumber, batchCount),
          total: batchCount,
          unit: "batch",
        },
      });
      return translateRemaining(offset + size, [...translated, ...nextBatch]);
    };
    const translated = await translateRemaining(0, []);
    updatePipelineStep("translate", {
      status: "done",
      detail: `${translated.length} segments translated`,
      progress: {
        label: "Llama translation",
        current: batchCount,
        total: batchCount,
        unit: "batch",
      },
    });
    addLog(`Llama translated ${translated.length} segments.`, "ok");
    return translated;
  };

  const translateBatchWithRetry = async (
    batch: Segment[],
    language: string,
    model: string,
  ) => {
    const attemptTranslate = async (attempt: number, lastError: unknown): Promise<Segment[]> => {
      if (attempt > 3) {
        throw lastError || new Error("Translation failed.");
      }
      try {
        return await translateBatch(batch, language, model);
      } catch (error) {
        addLog(
          `Translation attempt ${attempt} failed: ${
            error instanceof Error ? error.message : String(error)
          }`,
          "error",
        );
        if (attempt < 3) {
          await sleep(600 * attempt);
        }
        return attemptTranslate(attempt + 1, error);
      }
    };

    return attemptTranslate(1, null);
  };

  const translateBatch = async (batch: Segment[], language: string, model: string) => {
    const payload = batch.map((segment) => ({
      id: segment.id,
      start_ms: segment.start_ms,
      end_ms: segment.end_ms,
      text: segment.text,
    }));

    const response = await apiRequest<ChatResponse>("POST", "/v1/chat/completions", {
      model,
      stream: false,
      temperature: 0.1,
      max_tokens: Math.round(numberValue(maxTokens, 2048)),
      response_format: {
        type: "json_schema",
        json_schema: {
          name: "subtitle_translation_batch",
          strict: true,
          schema: translationSchema,
        },
      },
      messages: [
        {
          role: "system",
          content: [
            "You translate subtitle segments.",
            `Translate only the text field into ${language}.`,
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

    return validateTranslatedBatch(batch, parseTranslationJson(extractAssistantText(response)));
  };

  const cancelPipeline = async () => {
    cancelRequestedRef.current = true;
    setStatus("cancelling");
    setPipelineSteps((current) =>
      current.map((step) =>
        step.status === "running" ? { ...step, detail: "Cancelling..." } : step,
      ),
    );
    const taskId = activeTaskIdRef.current || activeTaskId;
    if (!taskId) {
      return;
    }
    try {
      await apiRequest("POST", `/v1/tasks/${encodeURIComponent(taskId)}/cancel`, {});
      addLog(`Cancel requested for task ${taskId}.`);
    } catch (error) {
      addLog(
        `Task cancel request failed: ${error instanceof Error ? error.message : String(error)}`,
        "error",
      );
    }
  };

  const runPipeline = async () => {
    if (!videoPath) {
      throw new Error("Choose a video file first.");
    }

    resetOutputs();
    setRunning(true);
    cancelRequestedRef.current = false;
    taskLogOffsetsRef.current.clear();
    setStatus("running");
    setPipelineSteps(createPipelineSteps(translateEnabled));

    try {
      const audioResult = await submitTask<FfmpegResponse>(
        "ffmpeg",
        "FFmpeg",
        "POST",
        "/v1/ffmpeg/convert",
        {
          source_path: videoPath,
          output_format: "wav",
        },
      );
      const audioPath = requireString(
        audioResult.output_path,
        "FFmpeg result did not include output_path.",
      );
      setAudioOutput(audioPath);
      addLog(`FFmpeg output: ${audioPath}`, "ok");

      const whisperResult = await submitTask<WhisperResponse>(
        "whisper",
        "Whisper",
        "POST",
        "/v1/audio/transcriptions",
        buildWhisperRequest(audioPath),
      );
      const nextSourceSegments = normalizeSegments(whisperResult.segments);
      if (nextSourceSegments.length === 0) {
        throw new Error(
          "Whisper completed but returned no timed segments. Enable timestamps/VAD and try again.",
        );
      }
      setSourceSegments(nextSourceSegments);
      addLog(`Whisper produced ${nextSourceSegments.length} timed segments.`, "ok");

      updatePipelineStep("source-srt", {
        status: "running",
        detail: "Rendering source SRT",
        progress: { label: "Source SRT", current: 0, total: 1, unit: "file" },
      });
      const sourceSrt = await renderSubtitle("source", nextSourceSegments);
      setSourceOutput(requireString(sourceSrt.output_path, "Source subtitle path is missing."));
      updatePipelineStep("source-srt", {
        status: "done",
        detail: "Source SRT written",
        progress: { label: "Source SRT", current: 1, total: 1, unit: "file" },
      });
      addLog(`Source SRT written: ${sourceSrt.output_path}`, "ok");

      if (translateEnabled) {
        const translated = await translateSegments(nextSourceSegments);
        setTranslatedSegments(translated);
        updatePipelineStep("translated-srt", {
          status: "running",
          detail: "Rendering translated SRT",
          progress: { label: "Translated SRT", current: 0, total: 1, unit: "file" },
        });
        const translatedSrt = await renderSubtitle("translated", translated);
        setTranslatedOutput(
          requireString(translatedSrt.output_path, "Translated subtitle path is missing."),
        );
        updatePipelineStep("translated-srt", {
          status: "done",
          detail: "Translated SRT written",
          progress: { label: "Translated SRT", current: 1, total: 1, unit: "file" },
        });
        addLog(`Translated SRT written: ${translatedSrt.output_path}`, "ok");
      } else {
        setTranslatedOutput("Skipped");
        updatePipelineStep("translate", {
          status: "skipped",
          detail: "Translation disabled",
          progress: null,
        });
        updatePipelineStep("translated-srt", {
          status: "skipped",
          detail: "Translation disabled",
          progress: null,
        });
        addLog("Translation skipped.");
      }

      setStatus("done");
      addLog("Pipeline completed.", "ok");
    } catch (error) {
      setStatus("failed");
      addLog(error instanceof Error ? error.message : String(error), "error");
      throw error;
    } finally {
      activeTaskIdRef.current = "";
      setActiveTaskId("");
      setRunning(false);
    }
  };

  useEffect(() => {
    if (!sdk.host.isAvailable()) {
      addLog("Slab plugin host bridge is not available in this webview.", "error");
      return undefined;
    }

    sdk.theme
      .getSnapshot()
      .then((snapshot) => sdk.theme.applyToDocument(snapshot))
      .catch((error) => {
        addLog(`Theme snapshot unavailable: ${error instanceof Error ? error.message : String(error)}`);
      });

    let cleanup: (() => void) | undefined;
    sdk.theme.subscribe((snapshot) => sdk.theme.applyToDocument(snapshot)).then((unlisten) => {
      cleanup = unlisten;
    });

    loadModels().catch((error) => {
      addLog(`Model list unavailable: ${error instanceof Error ? error.message : String(error)}`, "error");
    });

    return () => cleanup?.();
  }, []);

  useEffect(() => {
    window.__SLAB_VIDEO_SUBTITLE_TRANSLATOR__ = {
      pluginId: PLUGIN_ID,
      runPipeline,
      loadModels,
    };
    return () => {
      delete window.__SLAB_VIDEO_SUBTITLE_TRANSLATOR__;
    };
  });

  useEffect(() => {
    const nextPath = localModelPath(selectedValue(vadModel) ?? "");
    if (nextPath) {
      setVadModelPath(nextPath);
    }
  }, [localModelPath, vadModel]);

  return (
    <main className="plugin-shell">
      <section className="hero-card">
        <div className="space-y-3">
          <Badge variant="chip" className="w-fit">
            Slab WebView Plugin
          </Badge>
          <div className="space-y-2">
            <h1>Video Subtitle Translator</h1>
            <p className="max-w-3xl text-sm leading-6 text-muted-foreground">
              Extract audio with FFmpeg, transcribe timed segments with Whisper + VAD,
              then optionally translate and render SRT files.
            </p>
          </div>
        </div>
        <StatusBadge status={status} />
      </section>

      <Card variant="elevated" className="gap-4">
        <CardHeader className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
          <div className="space-y-1">
            <CardTitle>Video file</CardTitle>
            <CardDescription className="break-all">
              {videoPath || "No video selected yet."}
            </CardDescription>
          </div>
          <Button variant="cta" onClick={() => pickVideo().catch(reportError(addLog))} disabled={running}>
            Choose Video
          </Button>
        </CardHeader>
      </Card>

      <section className="plugin-grid">
        <Card variant="soft">
          <CardHeader className="flex-row items-start justify-between gap-3">
            <div>
              <CardTitle>Transcription</CardTitle>
              <CardDescription>Whisper, VAD and language detection settings.</CardDescription>
            </div>
            <Button
              variant="pill"
              size="sm"
              onClick={() => loadModels().catch(reportError(addLog))}
              disabled={running}
            >
              Refresh models
            </Button>
          </CardHeader>
          <CardContent className="space-y-5">
            <ModelSelect
              label="Whisper model"
              placeholder="Use active/default Whisper model"
              value={whisperModel}
              models={whisperModels}
              onChange={setWhisperModel}
            />
            <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_auto]">
              <Field label="Source language">
                <Input
                  value={sourceLanguage}
                  onChange={(event) => setSourceLanguage(event.target.value)}
                  placeholder="auto, en, zh, ja..."
                  disabled={detectLanguage}
                />
              </Field>
              <ToggleRow label="Auto detect" checked={detectLanguage} onChange={setDetectLanguage} />
            </div>

            <Separator />

            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-sm font-semibold">VAD</p>
                <p className="text-xs text-muted-foreground">Improve segment timing before Whisper.</p>
              </div>
              <Switch checked={vadEnabled} onCheckedChange={setVadEnabled} />
            </div>
            <ModelSelect
              label="VAD model"
              placeholder="Select or enter VAD model path below"
              value={vadModel}
              models={vadModels}
              onChange={setVadModel}
            />
            <Field label="VAD model path">
              <Input
                value={vadModelPath}
                onChange={(event) => setVadModelPath(event.target.value)}
                placeholder="Absolute VAD model path"
              />
            </Field>
            <div className="grid gap-4 md:grid-cols-3">
              <Field label="Threshold">
                <Input
                  type="number"
                  min="0"
                  max="1"
                  step="0.01"
                  value={vadThreshold}
                  onChange={(event) => setVadThreshold(event.target.value)}
                />
              </Field>
              <Field label="Min silence ms">
                <Input
                  type="number"
                  min="0"
                  step="50"
                  value={minSilence}
                  onChange={(event) => setMinSilence(event.target.value)}
                />
              </Field>
              <Field label="Speech pad ms">
                <Input
                  type="number"
                  min="0"
                  step="50"
                  value={speechPad}
                  onChange={(event) => setSpeechPad(event.target.value)}
                />
              </Field>
            </div>
          </CardContent>
        </Card>

        <Card variant="soft">
          <CardHeader>
            <div className="flex items-start justify-between gap-4">
              <div>
                <CardTitle>Translation</CardTitle>
                <CardDescription>Optional chat-model translation pass.</CardDescription>
              </div>
              <ToggleRow label="Enable" checked={translateEnabled} onChange={setTranslateEnabled} />
            </div>
          </CardHeader>
          <CardContent className="space-y-5">
            <Field label="Target language">
              <Input
                value={targetLanguage}
                onChange={(event) => setTargetLanguage(event.target.value)}
              />
            </Field>
            <ModelSelect
              label="Llama model"
              placeholder="Use active/default chat model"
              value={llamaModel}
              models={chatModels}
              onChange={setLlamaModel}
            />
            <div className="grid gap-4 md:grid-cols-2">
              <Field label="Batch size">
                <Input
                  type="number"
                  min="1"
                  max="50"
                  step="1"
                  value={batchSize}
                  onChange={(event) => setBatchSize(event.target.value)}
                />
              </Field>
              <Field label="Max tokens">
                <Input
                  type="number"
                  min="128"
                  step="128"
                  value={maxTokens}
                  onChange={(event) => setMaxTokens(event.target.value)}
                />
              </Field>
            </div>
            <div className="flex flex-wrap gap-3 pt-1">
              <Button
                variant="cta"
                onClick={() => runPipeline().catch(() => undefined)}
                disabled={running || !videoPath}
              >
                {running ? <Spinner /> : null}
                Run Pipeline
              </Button>
              <Button variant="destructive" onClick={() => cancelPipeline()} disabled={!running}>
                Cancel
              </Button>
            </div>
            <PipelineProgress steps={pipelineSteps} />
          </CardContent>
        </Card>
      </section>

      <section className="plugin-grid">
        <Card>
          <CardHeader>
            <CardTitle>Outputs</CardTitle>
          </CardHeader>
          <CardContent>
            <dl className="output-list">
              <dt>Audio</dt>
              <dd>{audioOutput}</dd>
              <dt>Source SRT</dt>
              <dd>{sourceOutput}</dd>
              <dt>Translated SRT</dt>
              <dd>{translatedOutput}</dd>
            </dl>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex-row items-center justify-between">
            <CardTitle>Step Log</CardTitle>
            <Button variant="quiet" size="sm" onClick={() => setLogs([])}>
              Clear
            </Button>
          </CardHeader>
          <CardContent>
            <ScrollArea className="h-[240px] rounded-2xl border bg-[var(--surface-soft)] p-4">
              <ol className="space-y-2 text-sm">
                {logs.length === 0 ? (
                  <li className="text-muted-foreground">No logs yet.</li>
                ) : (
                  logs.map((entry) => (
                    <li key={entry.id} className={cn("break-words", logClassName(entry.kind))}>
                      {entry.message}
                    </li>
                  ))
                )}
              </ol>
            </ScrollArea>
          </CardContent>
        </Card>
      </section>

      <section className="plugin-grid">
        <PreviewCard title="Source Preview" segments={sourceSegments} empty="No transcript yet." />
        <PreviewCard
          title="Translation Preview"
          segments={translatedSegments}
          empty="Translation is optional."
        />
      </section>
    </main>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="space-y-2">
      <Label className="text-xs font-semibold uppercase tracking-[0.14em] text-muted-foreground">
        {label}
      </Label>
      {children}
    </div>
  );
}

function ToggleRow({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex items-center gap-2 text-sm font-medium">
      <Checkbox checked={checked} onCheckedChange={(value) => onChange(value === true)} />
      {label}
    </label>
  );
}

function ModelSelect({
  label,
  placeholder,
  value,
  models,
  onChange,
}: {
  label: string;
  placeholder: string;
  value: string;
  models: ModelInfo[];
  onChange: (value: string) => void;
}) {
  return (
    <Field label={label}>
      <Select
        value={value || EMPTY_SELECT_VALUE}
        onValueChange={(nextValue) =>
          onChange(nextValue === EMPTY_SELECT_VALUE ? "" : nextValue)
        }
      >
        <SelectTrigger className="w-full" variant="soft">
          <SelectValue placeholder={placeholder} />
        </SelectTrigger>
        <SelectContent variant="soft">
          <SelectItem value={EMPTY_SELECT_VALUE}>{placeholder}</SelectItem>
          {models.map((model) => (
            <SelectItem key={model.id} value={model.id}>
              {model.display_name ? `${model.display_name} (${model.id})` : model.id}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </Field>
  );
}

function PreviewCard({
  title,
  segments,
  empty,
}: {
  title: string;
  segments: Segment[];
  empty: string;
}) {
  const preview =
    segments.length > 0
      ? segments
          .slice(0, 10)
          .map(
            (segment) =>
              `${segment.id}. [${segment.start_ms} -> ${segment.end_ms}] ${segment.text}`,
          )
          .join("\n")
      : empty;

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle>{title}</CardTitle>
        <Badge variant="status">{segments.length} segments</Badge>
      </CardHeader>
      <CardContent>
        <Textarea className="min-h-[240px] font-mono text-xs" value={preview} readOnly />
      </CardContent>
    </Card>
  );
}

function StatusBadge({ status }: { status: "idle" | "running" | "failed" | "done" | "cancelling" }) {
  const labels = {
    idle: "Idle",
    running: "Running",
    failed: "Failed",
    done: "Done",
    cancelling: "Cancelling",
  } as const;

  return (
    <Badge
      variant="status"
      data-status={status === "done" ? "success" : status === "failed" ? "danger" : "info"}
      className="px-4 py-2 text-xs uppercase tracking-[0.16em]"
    >
      {labels[status]}
    </Badge>
  );
}

function PipelineProgress({ steps }: { steps: PipelineStep[] }) {
  const completedSteps = steps.filter((step) =>
    step.status === "done" || step.status === "skipped",
  ).length;

  return (
    <div className="pipeline-progress">
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm font-semibold">Pipeline progress</p>
        <span className="text-xs font-medium text-muted-foreground">
          {completedSteps}/{steps.length}
        </span>
      </div>
      <div className="pipeline-step-list">
        {steps.map((step) => {
          const percent = pipelineStepPercent(step);
          return (
            <div key={step.id} className="pipeline-step">
              <div className="pipeline-step-header">
                <span className="text-sm font-medium">{step.label}</span>
                <Badge
                  variant="status"
                  data-status={
                    step.status === "done"
                      ? "success"
                      : step.status === "failed"
                        ? "danger"
                        : "info"
                  }
                  className="px-2 py-1 text-[0.65rem] uppercase tracking-[0.12em]"
                >
                  {pipelineStatusLabel(step.status)}
                </Badge>
              </div>
              {step.detail ? (
                <p className="line-clamp-2 text-xs text-muted-foreground">{step.detail}</p>
              ) : null}
              {percent === null ? (
                <p className="text-xs text-muted-foreground">Waiting for progress data</p>
              ) : (
                <div className="pipeline-meter">
                  <Progress value={percent} />
                  <span>{Math.round(percent)}%</span>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function formatProgress(progress: TaskProgress | undefined): string {
  if (!progress) {
    return "";
  }

  const parts = [];
  if (progress.label) {
    parts.push(progress.label);
  }
  if (progress.message && parts.length === 0) {
    parts.push(progress.message);
  }
  if (
    typeof progress.current === "number" &&
    typeof progress.total === "number" &&
    progress.total > 0
  ) {
    parts.push(`${progress.current}/${progress.total}${progress.unit ? ` ${progress.unit}` : ""}`);
  }
  if (typeof progress.step === "number" && typeof progress.step_count === "number") {
    parts.push(`step ${progress.step}/${progress.step_count}`);
  }

  return parts.length > 0 ? ` (${parts.join(", ")})` : "";
}

function formatTaskDetail(status: string, progress?: TaskProgress): string {
  const percent = taskProgressPercent(progress);
  if (percent !== null) {
    return `${status || "running"} - ${Math.round(percent)}%`;
  }
  return progress?.message || status || "running";
}

function completeTaskProgress(progress: TaskProgress | undefined): TaskProgress {
  if (progress?.total && progress.total > 0) {
    return { ...progress, current: progress.total };
  }

  if (typeof progress?.current === "number" && progress.current > 0) {
    return { ...progress, total: progress.current };
  }

  const fallback = progress ?? {};
  return {
    ...fallback,
    current: 1,
    total: 1,
    unit: progress?.unit ?? "task",
  };
}

function taskProgressPercent(progress: TaskProgress | null | undefined): number | null {
  if (
    typeof progress?.current !== "number" ||
    typeof progress?.total !== "number" ||
    progress.total <= 0
  ) {
    return null;
  }

  return clamp((progress.current / progress.total) * 100, 0, 100);
}

function pipelineStepPercent(step: PipelineStep): number | null {
  if (step.status === "done") {
    return 100;
  }
  if (step.status === "pending") {
    return 0;
  }
  if (step.status === "skipped") {
    return null;
  }
  return taskProgressPercent(step.progress);
}

function pipelineStatusLabel(status: PipelineStepStatus): string {
  const labels = {
    pending: "Pending",
    running: "Running",
    done: "Done",
    failed: "Failed",
    skipped: "Skipped",
  } as const;
  return labels[status];
}

function createPipelineSteps(includeTranslation: boolean): PipelineStep[] {
  return [
    {
      id: "ffmpeg",
      label: "FFmpeg audio extraction",
      status: "pending",
      detail: "Waiting",
      progress: null,
    },
    {
      id: "whisper",
      label: "Whisper transcription",
      status: "pending",
      detail: "Waiting",
      progress: null,
    },
    {
      id: "source-srt",
      label: "Source SRT render",
      status: "pending",
      detail: "Waiting",
      progress: null,
    },
    {
      id: "translate",
      label: "Translation batches",
      status: includeTranslation ? "pending" : "skipped",
      detail: includeTranslation ? "Waiting" : "Translation disabled",
      progress: null,
    },
    {
      id: "translated-srt",
      label: "Translated SRT render",
      status: includeTranslation ? "pending" : "skipped",
      detail: includeTranslation ? "Waiting" : "Translation disabled",
      progress: null,
    },
  ];
}

function normalizeSegments(rawSegments: unknown[] | undefined): Segment[] {
  if (!Array.isArray(rawSegments)) {
    return [];
  }

  return rawSegments
    .map((rawSegment, index): Segment | null => {
      const segment = rawSegment as Record<string, unknown>;
      const startMs = asNonNegativeInteger(segment.start_ms);
      const endMs = asNonNegativeInteger(segment.end_ms);
      const text = typeof segment.text === "string" ? segment.text.trim() : "";
      if (startMs === null || endMs === null || endMs <= startMs || !text) {
        return null;
      }
      return {
        id: index + 1,
        start_ms: startMs,
        end_ms: endMs,
        text,
      };
    })
    .filter((segment): segment is Segment => segment !== null);
}

function extractAssistantText(response: ChatResponse): string {
  const content = response.choices?.[0]?.message?.content;
  if (typeof content === "string") {
    return content;
  }
  if (Array.isArray(content)) {
    return content
      .map((part) => {
        if (typeof part.text === "string") {
          return part.text;
        }
        if (part.value !== undefined) {
          return JSON.stringify(part.value);
        }
        return "";
      })
      .join("");
  }
  throw new Error("Llama response did not include assistant text.");
}

function parseTranslationJson(text: string): unknown {
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

function validateTranslatedBatch(sourceBatch: Segment[], translatedBatch: unknown): Segment[] {
  if (!Array.isArray(translatedBatch)) {
    throw new Error("Translation JSON must be an array or { segments: [...] }.");
  }
  if (translatedBatch.length !== sourceBatch.length) {
    throw new Error(
      `Translation batch length mismatch: expected ${sourceBatch.length}, got ${translatedBatch.length}.`,
    );
  }

  return translatedBatch.map((item, index) => {
    const source = sourceBatch[index];
    const row = item as Record<string, unknown>;
    if (
      row.id !== source.id ||
      row.start_ms !== source.start_ms ||
      row.end_ms !== source.end_ms
    ) {
      throw new Error(`Translation segment ${index + 1} changed id or timestamps.`);
    }
    if (typeof row.text !== "string" || !row.text.trim()) {
      throw new Error(`Translation segment ${source.id} has empty text.`);
    }
    return {
      id: source.id,
      start_ms: source.start_ms,
      end_ms: source.end_ms,
      text: row.text.trim(),
    };
  });
}

function asNonNegativeInteger(value: unknown): number | null {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    return null;
  }
  return Math.round(value);
}

function selectedValue(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function numberValue(value: string, fallback: number): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function requireString(value: unknown, message: string): string {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(message);
  }
  return value;
}

function pruneNulls(value: unknown): unknown {
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

function assertNotCancelled(cancelRequested: boolean): void {
  if (cancelRequested) {
    throw new Error("Pipeline cancelled.");
  }
}

function reportError(addLog: (message: string, kind?: LogEntry["kind"]) => void) {
  return (error: unknown) => {
    addLog(error instanceof Error ? error.message : String(error), "error");
  };
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

function logClassName(kind: LogEntry["kind"]): string {
  if (kind === "ok") {
    return "text-[var(--brand-teal)]";
  }
  if (kind === "error") {
    return "text-destructive";
  }
  return "text-muted-foreground";
}

const translationSchema = {
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
