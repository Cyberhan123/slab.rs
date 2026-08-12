import { useCallback, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { apiClient } from "@slab/api"
import { toast } from "sonner"

import { getAudioTranscription } from "@/lib/media-task-api"

import { useMicRecorder } from "./use-mic-recorder"

/** Poll ceiling for the async transcription task (~60 s at 500 ms). */
const POLL_INTERVAL_MS = 500
const MAX_POLL_ATTEMPTS = 120

export type VoiceInputState = "idle" | "recording" | "transcribing"

export interface UseVoiceInputOptions {
  /** Receives the recognized text (the sender appends it to the composer). */
  onTranscript: (text: string) => void
}

/**
 * Desktop-only voice input: capture speech via the mic, stage it as a temp file
 * (host-only Tauri command), transcribe via the existing `/v1/audio/transcriptions`
 * path (ffmpeg-decoded server-side), poll to completion, then hand the text back.
 *
 * Calls the imperative `apiClient` (not the `useMutation` hook) so it does not
 * pull a query-client context into the sender's render — the mic button is only
 * rendered on Tauri anyway. Web is unsupported: the transcription endpoint is
 * path-only and the recorder needs the Tauri host to stage the bytes.
 */
export function useVoiceInput({ onTranscript }: UseVoiceInputOptions) {
  const recorder = useMicRecorder()
  const [transcribing, setTranscribing] = useState(false)

  const busy = recorder.state === "recording" || transcribing
  const state: VoiceInputState = transcribing
    ? "transcribing"
    : recorder.state === "recording"
      ? "recording"
      : "idle"

  const toggle = useCallback(async () => {
    if (recorder.state === "recording") {
      setTranscribing(true)
      let tempPath: string | null = null
      try {
        const blob = await recorder.stop()
        if (!blob || blob.size === 0) return
        const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()))
        tempPath = await invoke<string>("write_temp_audio", { bytes, extension: "webm" })
        const { data, error } = await apiClient.POST("/v1/audio/transcriptions", {
          body: { path: tempPath },
        })
        if (error || !data?.operation_id) {
          throw new Error("transcription request was not accepted")
        }
        const text = await pollTranscript(data.operation_id)
        if (text) onTranscript(text)
      } catch (error) {
        toast.error(
          `Voice input failed: ${error instanceof Error ? error.message : String(error)}`,
        )
      } finally {
        if (tempPath) {
          await invoke("remove_temp_audio", { path: tempPath }).catch(() => {})
        }
        setTranscribing(false)
      }
      return
    }
    await recorder.start()
  }, [recorder, onTranscript])

  return { state, busy, error: recorder.error, toggle }
}

/** Poll the transcription task until it reaches a terminal state. */
async function pollTranscript(operationId: string): Promise<string | null> {
  for (let i = 0; i < MAX_POLL_ATTEMPTS; i += 1) {
    const task = await getAudioTranscription(operationId)
    if (task.status !== "pending" && task.status !== "running") {
      if (task.status === "succeeded") return task.transcript_text?.trim() || null
      return null
    }
    await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS))
  }
  return null
}
