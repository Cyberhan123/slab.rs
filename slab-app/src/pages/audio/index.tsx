import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Spinner } from '@/components/ui/spinner';
import { toast } from 'sonner';
import useFile, { SelectedFile } from '@/hooks/use-file';
import useTranscribe from './hooks/use-transcribe';
import useIsTauri from '@/hooks/use-tauri';

export default function Audio() {
  const navigate = useNavigate();
  const isTauri = useIsTauri();
  // file object or string path (desktop uses string path)
  const [file, setFile] = useState<SelectedFile | null>(null);

  const [taskId, setTaskId] = useState<string | null>(null);
  const { handleFile } = useFile();

  const transcribe = useTranscribe();

  const handleFileChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFile = await handleFile(e);

    if (selectedFile) {
      setFile(selectedFile);
    }
  };

  const handleTranscribe = async () => {
    if (!isTauri) {
      toast.error('Web transcription upload is not implemented yet. Please use the desktop app.');
      return;
    }

    if (!file) {
      toast.error('Please select a file first.');
      return;
    }

    try {
      const result = await transcribe.handleTranscribe(file.file);
      setTaskId(result.task_id);

      toast.success('Transcription task created.', {
        description: `Task ID: ${result.task_id}`,
        action: {
          label: 'View tasks',
          onClick: () => navigate('/task')
        }
      });
    } catch (err: any) {
      toast.error('Failed to create transcription task.', {
        description: err?.message || err?.error || 'Unknown error'
      });
    }
  };

  return (
    <div className="container mx-auto px-4 py-8 space-y-8">
      <div className="text-center space-y-4">
        <h1 className="text-3xl font-bold text-center">Audio Transcription</h1>
        <p className="text-muted-foreground max-w-2xl mx-auto">
          {isTauri
            ? 'Desktop mode: select a local audio/video file path and submit it as a URL path.'
            : 'Web upload is not implemented yet; this will be added later.'}
        </p>
      </div>

      <Card className="max-w-2xl mx-auto">
        <CardHeader>
          <CardTitle>Select File</CardTitle>
          <CardDescription>Supported formats: audio and video files.</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {transcribe?.isError && (
            <Alert variant="destructive">
              <AlertTitle>Error</AlertTitle>
              <AlertDescription>
                {(transcribe?.error as any)?.error || 'Failed to create transcription task. Please retry.'}
              </AlertDescription>
            </Alert>
          )}

          <div className="space-y-2">
            <Label htmlFor="file">File</Label>
            <Input
              id="file"
              type="file"
              accept="audio/*,video/*"
              onChange={handleFileChange}
              disabled={transcribe?.isPending || !isTauri}
            />
            {file && (
              <p className="text-sm text-muted-foreground">
                Selected: {file.name}
              </p>
            )}
          </div>

          {transcribe?.isPending && (
            <div className="flex flex-col items-center space-y-4">
              <Spinner className="h-8 w-8" />
              <p>Processing transcription request...</p>
              {taskId && (
                <p className="text-xs text-muted-foreground">Task ID: {taskId}</p>
              )}
            </div>
          )}
        </CardContent>
        <CardFooter className="flex justify-end">
          <Button
            onClick={handleTranscribe}
            disabled={!isTauri || !file || transcribe?.isPending}
          >
            {transcribe?.isPending ? 'Processing...' : 'Start Transcription'}
          </Button>
        </CardFooter>
      </Card>
    </div>
  );
}
