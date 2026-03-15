import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  CheckCircle2,
  Circle,
  Download,
  Loader2,
  XCircle,
} from 'lucide-react';
import { SERVER_BASE_URL } from '@/lib/config';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Progress } from '@/components/ui/progress';

// ── Constants ────────────────────────────────────────────────────────────────

/** How often (ms) we poll for background task completion. */
const TASK_POLL_INTERVAL_MS = 2000;

/** Simulated progress step per tick while a download is in flight (0-100). */
const PROGRESS_STEP = 3;

/** Maximum simulated progress before the real completion signal arrives. */
const PROGRESS_MAX_SIMULATED = 90;

// ── Types ─────────────────────────────────────────────────────────────────────

interface ComponentStatus {
  name: string;
  installed: boolean;
  version?: string | null;
}

interface SetupStatus {
  initialized: boolean;
  ffmpeg: ComponentStatus;
  backends: ComponentStatus[];
}

interface OperationAccepted {
  operation_id: string;
}

interface TaskRecord {
  id: string;
  status: 'pending' | 'running' | 'succeeded' | 'failed' | string;
  error_msg?: string | null;
}

// ── API helpers ───────────────────────────────────────────────────────────────

const BASE = SERVER_BASE_URL;

async function fetchSetupStatus(): Promise<SetupStatus> {
  const res = await fetch(`${BASE}/v1/setup/status`);
  if (!res.ok) throw new Error(`setup/status failed: ${res.status}`);
  return res.json() as Promise<SetupStatus>;
}

async function triggerFfmpegDownload(): Promise<OperationAccepted> {
  const res = await fetch(`${BASE}/v1/setup/ffmpeg/download`, { method: 'POST' });
  if (!res.ok) throw new Error(`ffmpeg/download failed: ${res.status}`);
  return res.json() as Promise<OperationAccepted>;
}

async function fetchTask(id: string): Promise<TaskRecord> {
  const res = await fetch(`${BASE}/v1/tasks/${id}`);
  if (!res.ok) throw new Error(`tasks/${id} failed: ${res.status}`);
  return res.json() as Promise<TaskRecord>;
}

async function completeSetup(): Promise<SetupStatus> {
  const res = await fetch(`${BASE}/v1/setup/complete`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ initialized: true }),
  });
  if (!res.ok) throw new Error(`setup/complete failed: ${res.status}`);
  return res.json() as Promise<SetupStatus>;
}

// ── Custom hooks ──────────────────────────────────────────────────────────────

function useTaskPoller(
  taskId: string | null,
  onDone: (task: TaskRecord) => void,
) {
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    if (!taskId) return;
    const poll = async () => {
      try {
        const task = await fetchTask(taskId);
        if (task.status === 'succeeded' || task.status === 'failed') {
          if (intervalRef.current) clearInterval(intervalRef.current);
          onDone(task);
        }
      } catch {
        /* network hiccup – keep polling */
      }
    };
    void poll();
    intervalRef.current = setInterval(() => { void poll(); }, TASK_POLL_INTERVAL_MS);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [taskId, onDone]);
}

// ── Sub-components ────────────────────────────────────────────────────────────

function StatusIcon({ installed }: { installed: boolean }) {
  if (installed)
    return <CheckCircle2 className="h-5 w-5 shrink-0 text-green-500" />;
  return <Circle className="h-5 w-5 shrink-0 text-muted-foreground" />;
}

function ComponentRow({
  status,
  extra,
}: {
  status: ComponentStatus;
  extra?: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between py-2">
      <div className="flex items-center gap-3">
        <StatusIcon installed={status.installed} />
        <span className="text-sm font-medium capitalize">{status.name}</span>
        {status.version && (
          <Badge variant="secondary" className="text-xs">
            {status.version}
          </Badge>
        )}
      </div>
      <div className="flex items-center gap-2">
        {status.installed ? (
          <Badge variant="outline" className="text-green-600">
            Installed
          </Badge>
        ) : (
          <Badge variant="outline" className="text-muted-foreground">
            Not found
          </Badge>
        )}
        {extra}
      </div>
    </div>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────────

type DownloadState = 'idle' | 'downloading' | 'done' | 'error';

export default function SetupPage() {
  const navigate = useNavigate();

  const [status, setStatus] = useState<SetupStatus | null>(null);
  const [checking, setChecking] = useState(true);
  const [checkError, setCheckError] = useState<string | null>(null);

  // FFmpeg download state
  const [ffmpegDownload, setFfmpegDownload] = useState<DownloadState>('idle');
  const [ffmpegTaskId, setFfmpegTaskId] = useState<string | null>(null);
  const [ffmpegError, setFfmpegError] = useState<string | null>(null);
  const [ffmpegProgress, setFfmpegProgress] = useState(0); // simulated 0-100

  // Completing setup
  const [completing, setCompleting] = useState(false);

  // ── Initial status check ────────────────────────────────────────────────────
  useEffect(() => {
    let cancelled = false;
    const check = async () => {
      try {
        const s = await fetchSetupStatus();
        if (cancelled) return;
        setStatus(s);
        if (s.initialized) {
          navigate('/', { replace: true });
        }
      } catch (err) {
        if (!cancelled)
          setCheckError(err instanceof Error ? err.message : String(err));
      } finally {
        if (!cancelled) setChecking(false);
      }
    };
    void check();
    return () => { cancelled = true; };
  }, [navigate]);

  // ── FFmpeg task poller ───────────────────────────────────────────────────────
  const handleFfmpegDone = useCallback(
    async (task: TaskRecord) => {
      if (task.status === 'succeeded') {
        setFfmpegDownload('done');
        setFfmpegProgress(100);
        // Refresh status to reflect installation
        try {
          const s = await fetchSetupStatus();
          setStatus(s);
        } catch { /* ignore */ }
      } else {
        setFfmpegDownload('error');
        setFfmpegError(task.error_msg ?? 'Download failed');
      }
    },
    [],
  );

  useTaskPoller(ffmpegTaskId, handleFfmpegDone);

  // Simulate progress while task is running
  useEffect(() => {
    if (ffmpegDownload !== 'downloading') return;
    // Tick every 800 ms to give a smooth feel; actual completion comes from task polling.
    const t = setInterval(() => {
      setFfmpegProgress((p) => Math.min(p + PROGRESS_STEP, PROGRESS_MAX_SIMULATED));
    }, 800);
    return () => clearInterval(t);
  }, [ffmpegDownload]);

  // ── Handlers ─────────────────────────────────────────────────────────────────
  const handleDownloadFfmpeg = async () => {
    setFfmpegDownload('downloading');
    setFfmpegError(null);
    setFfmpegProgress(5);
    try {
      const op = await triggerFfmpegDownload();
      setFfmpegTaskId(op.operation_id);
    } catch (err) {
      setFfmpegDownload('error');
      setFfmpegError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleComplete = async () => {
    setCompleting(true);
    try {
      await completeSetup();
      navigate('/', { replace: true });
    } catch (err) {
      setCheckError(err instanceof Error ? err.message : String(err));
      setCompleting(false);
    }
  };

  // ── Derived values ────────────────────────────────────────────────────────────
  const ffmpegReady =
    status?.ffmpeg.installed || ffmpegDownload === 'done';

  const allBackendsUnavailable =
    status != null && status.backends.every((b) => !b.installed);

  // ── Render ────────────────────────────────────────────────────────────────────

  // Loading / error states
  if (checking) {
    return (
      <div className="flex h-screen items-center justify-center">
        <div className="flex flex-col items-center gap-4">
          <Loader2 className="h-10 w-10 animate-spin text-primary" />
          <p className="text-muted-foreground text-sm">Checking environment…</p>
        </div>
      </div>
    );
  }

  if (checkError) {
    return (
      <div className="flex h-screen items-center justify-center p-8">
        <Alert variant="destructive" className="max-w-md">
          <XCircle className="h-4 w-4" />
          <AlertTitle>Could not reach the server</AlertTitle>
          <AlertDescription>
            {checkError}
            <br />
            Make sure <code>slab-server</code> is running on{' '}
            <code>localhost:3000</code> and reload the app.
          </AlertDescription>
        </Alert>
      </div>
    );
  }

  if (!status) return null;

  return (
    <div className="flex h-screen w-full flex-col items-center justify-center bg-background p-6">
      <div className="w-full max-w-xl space-y-6">
        {/* Header */}
        <div className="space-y-1 text-center">
          <h1 className="text-2xl font-semibold tracking-tight">
            Environment Setup
          </h1>
          <p className="text-muted-foreground text-sm">
            Slab needs a few dependencies before it can process audio, video,
            and AI workloads. This only needs to happen once.
          </p>
        </div>

        {/* FFmpeg card */}
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">FFmpeg</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <ComponentRow
              status={
                ffmpegDownload === 'done'
                  ? { ...status.ffmpeg, installed: true }
                  : status.ffmpeg
              }
              extra={
                !ffmpegReady && ffmpegDownload === 'idle' ? (
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => { void handleDownloadFfmpeg(); }}
                  >
                    <Download className="mr-1.5 h-3.5 w-3.5" />
                    Download
                  </Button>
                ) : null
              }
            />

            {ffmpegDownload === 'downloading' && (
              <div className="space-y-1.5">
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  Downloading FFmpeg…
                </div>
                <Progress value={ffmpegProgress} className="h-1.5" />
              </div>
            )}

            {ffmpegDownload === 'error' && (
              <Alert variant="destructive">
                <XCircle className="h-4 w-4" />
                <AlertTitle>Download failed</AlertTitle>
                <AlertDescription className="flex items-center justify-between gap-2">
                  <span>{ffmpegError}</span>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => { void handleDownloadFfmpeg(); }}
                  >
                    Retry
                  </Button>
                </AlertDescription>
              </Alert>
            )}
          </CardContent>
        </Card>

        {/* AI backends card */}
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-base">AI Backends</CardTitle>
          </CardHeader>
          <CardContent className="divide-y divide-border">
            {status.backends.map((b) => (
              <ComponentRow key={b.name} status={b} />
            ))}
          </CardContent>
        </Card>

        {allBackendsUnavailable && (
          <Alert>
            <AlertTitle>No AI backends available</AlertTitle>
            <AlertDescription>
              You can download backend libraries via{' '}
              <strong>Settings → Backends</strong> after initial setup. The app
              will work in cloud-provider mode in the meantime.
            </AlertDescription>
          </Alert>
        )}

        {/* Action row */}
        <div className="flex justify-end gap-3">
          <Button
            variant="ghost"
            onClick={() => { void handleComplete(); }}
            disabled={completing}
          >
            Skip
          </Button>
          <Button
            onClick={() => { void handleComplete(); }}
            disabled={completing}
          >
            {completing ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                Saving…
              </>
            ) : (
              'Continue to App'
            )}
          </Button>
        </div>
      </div>
    </div>
  );
}
