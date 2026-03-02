import { useState } from 'react';
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Spinner } from '@/components/ui/spinner';
import { toast } from 'sonner';
import { Loader2 } from 'lucide-react';
import api from '@/lib/api';

export default function Image() {
  const [prompt, setPrompt] = useState('');
  const [model, setModel] = useState('');
  const [numImages, setNumImages] = useState('1');
  const [size, setSize] = useState('512x512');
  const [taskId, setTaskId] = useState<string | null>(null);
  const [isGenerating, setIsGenerating] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [images, setImages] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  // API mutations
  const generateImageMutation = api.useMutation('post', '/v1/images/generations');
  const getTaskMutation = api.useMutation('get', '/v1/tasks/{id}');
  const getTaskResultMutation = api.useMutation('get', '/v1/tasks/{id}/result');

  const handleGenerate = async () => {
    if (!prompt) {
      setError('Please enter a prompt');
      return;
    }

    if (!model) {
      setError('Please select a model');
      return;
    }

    setError(null);
    setImages([]);
    setIsGenerating(true);

    try {
      const data = await generateImageMutation.mutateAsync({
        body: {
          model,
          prompt,
          n: parseInt(numImages),
          size
        }
      }) as { task_id: string };

      setTaskId(data.task_id);
      setIsGenerating(false);
      setIsProcessing(true);
      
      // Poll task status
      pollTaskStatus(data.task_id);
    } catch (err) {
      setError('Generation failed: ' + (err instanceof Error ? err.message : 'Unknown error'));
      setIsGenerating(false);
      toast.error('Image generation failed');
    }
  };

  const pollTaskStatus = async (id: string) => {
    try {
      const task = await getTaskMutation.mutateAsync({
        params: {
          path: { id }
        }
      }) as { status: string };
      
      if (task.status === 'succeeded') {
        // Fetch generated result
        const result = await getTaskResultMutation.mutateAsync({
          params: {
            path: { id }
          }
        }) as any;

        if (result.images && Array.isArray(result.images)) {
          setImages(result.images);
        } else if (result.image) {
          setImages([result.image]);
        } else {
          setImages([result]);
        }
        setIsProcessing(false);
        toast.success('Image generated successfully');
      } else if (task.status === 'failed') {
        setError('Generation task failed');
        setIsProcessing(false);
        toast.error('Generation task failed');
      } else if (task.status === 'cancelled' || task.status === 'interrupted') {
        setError(`Generation task ${task.status}`);
        setIsProcessing(false);
        toast.error(`Generation task ${task.status}`);
      } else {
        // Continue polling for pending/running states
        setTimeout(() => pollTaskStatus(id), 2000);
      }
    } catch (err) {
      setError('Failed to get task status');
      setIsProcessing(false);
      toast.error('Failed to get task status');
    }
  };

  return (
    <div className="container mx-auto px-4 py-8 space-y-8">
      <div className="text-center space-y-4">
        <h1 className="text-3xl font-bold text-center">Image Generation</h1>
        <p className="text-muted-foreground max-w-2xl mx-auto">
          Enter a prompt and the system will generate corresponding images
        </p>
      </div>

      <Card className="max-w-2xl mx-auto">
        <CardHeader>
          <CardTitle>Generation Settings</CardTitle>
          <CardDescription>Configure image generation parameters</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {error && (
            <Alert variant="destructive">
              <AlertTitle>Error</AlertTitle>
              <AlertDescription>{error}</AlertDescription>
            </Alert>
          )}

          <div className="space-y-2">
            <Label htmlFor="prompt">Prompt</Label>
            <Textarea
              id="prompt"
              placeholder="Describe the image you want to generate..."
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              disabled={isGenerating || isProcessing}
              rows={4}
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label htmlFor="model">Model</Label>
              <Select value={model} onValueChange={setModel} disabled={isGenerating || isProcessing}>
                <SelectTrigger id="model">
                  <SelectValue placeholder="Select model" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="stable-diffusion">Stable Diffusion</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <Label htmlFor="numImages">Number of Images</Label>
              <Select value={numImages} onValueChange={setNumImages} disabled={isGenerating || isProcessing}>
                <SelectTrigger id="numImages">
                  <SelectValue placeholder="Select count" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="1">1</SelectItem>
                  <SelectItem value="2">2</SelectItem>
                  <SelectItem value="4">4</SelectItem>
                  <SelectItem value="8">8</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <div className="space-y-2">
            <Label htmlFor="size">Image Size</Label>
            <Select value={size} onValueChange={setSize} disabled={isGenerating || isProcessing}>
              <SelectTrigger id="size">
                <SelectValue placeholder="Select size" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="256x256">256×256</SelectItem>
                <SelectItem value="512x512">512×512</SelectItem>
                <SelectItem value="1024x1024">1024×1024</SelectItem>
                <SelectItem value="1024x1536">1024×1536</SelectItem>
                <SelectItem value="1536x1024">1536×1024</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {(isGenerating || isProcessing) && (
            <div className="flex flex-col items-center space-y-4">
              <Spinner className="h-8 w-8" />
              <p>{isGenerating ? 'Submitting request...' : 'Generating image, please wait...'}</p>
              {taskId && (
                <p className="text-xs text-muted-foreground">Task ID: {taskId}</p>
              )}
            </div>
          )}
        </CardContent>
        <CardFooter className="flex justify-end">
          <Button
            onClick={handleGenerate}
            disabled={!prompt || !model || isGenerating || isProcessing}
          >
            {(isGenerating || isProcessing) ? (
              <>
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                Generating...
              </>
            ) : (
              'Generate Image'
            )}
          </Button>
        </CardFooter>
      </Card>

      {images.length > 0 && (
        <Card className="max-w-4xl mx-auto">
          <CardHeader>
            <CardTitle>Generated Results</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
              {images.map((image, index) => (
                <div key={index} className="space-y-2">
                  <div className="aspect-square rounded-md overflow-hidden border">
                    {typeof image === 'string' && image.startsWith('data:image') ? (
                      <img src={image} alt={`Generated image ${index + 1}`} className="w-full h-full object-cover" />
                    ) : (
                      <div className="w-full h-full flex items-center justify-center bg-muted">
                        <p className="text-sm text-muted-foreground">Unable to display image</p>
                      </div>
                    )}
                  </div>
                  <Button
                    variant="secondary"
                    size="sm"
                    className="w-full"
                    onClick={() => {
                      if (typeof image === 'string' && image.startsWith('data:image')) {
                        const link = document.createElement('a');
                        link.href = image;
                        link.download = `generated-image-${index + 1}.png`;
                        link.click();
                      }
                    }}
                  >
                    Download
                  </Button>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}