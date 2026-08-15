import { useCallback, useEffect, useRef, useState } from "react"

export type MicRecorderState = "idle" | "recording" | "error"

export interface UseMicRecorder {
  state: MicRecorderState
  error: string | null
  /** Request microphone access and begin recording. */
  start: () => Promise<void>
  /** Stop recording and resolve the captured audio Blob (or `null` on failure). */
  stop: () => Promise<Blob | null>
}

/**
 * Microphone recorder built on the browser `MediaRecorder` API. Captures audio
 * to an in-memory Blob. The host (Tauri) stages the Blob to a temp file for the
 * path-based transcription endpoint — see `useVoiceInput`.
 *
 * No-ops gracefully when `MediaRecorder` / `getUserMedia` are unavailable (e.g.
 * non-browser test environments).
 */
export function useMicRecorder(): UseMicRecorder {
  const [state, setState] = useState<MicRecorderState>("idle")
  const [error, setError] = useState<string | null>(null)
  const recorderRef = useRef<MediaRecorder | null>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const chunksRef = useRef<BlobPart[]>([])
  const stopResolveRef = useRef<((blob: Blob | null) => void) | null>(null)

  const cleanup = useCallback(() => {
    recorderRef.current = null
    if (streamRef.current) {
      for (const track of streamRef.current.getTracks()) track.stop()
      streamRef.current = null
    }
  }, [])

  // Stop any active stream on unmount.
  useEffect(() => () => cleanup(), [cleanup])

  const start = useCallback(async () => {
    setError(null)
    const mediaDevices = navigator.mediaDevices
    if (!mediaDevices?.getUserMedia || typeof MediaRecorder === "undefined") {
      setError("Microphone capture is not available in this environment.")
      setState("error")
      return
    }
    try {
      const stream = await mediaDevices.getUserMedia({ audio: true })
      streamRef.current = stream
      chunksRef.current = []
      const recorder = new MediaRecorder(stream)
      recorder.ondataavailable = (event) => {
        if (event.data.size > 0) chunksRef.current.push(event.data)
      }
      recorder.onstop = () => {
        const blob = new Blob(chunksRef.current, {
          type: recorder.mimeType || "audio/webm",
        })
        stopResolveRef.current?.(blob)
        stopResolveRef.current = null
        cleanup()
        setState("idle")
      }
      recorderRef.current = recorder
      recorder.start()
      setState("recording")
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      setState("error")
      cleanup()
    }
  }, [cleanup])

  const stop = useCallback(async () => {
    const recorder = recorderRef.current
    if (!recorder || recorder.state === "inactive") {
      cleanup()
      setState("idle")
      return null
    }
    return new Promise<Blob | null>((resolve) => {
      stopResolveRef.current = resolve
      recorder.stop()
    })
  }, [cleanup])

  return { state, error, start, stop }
}
