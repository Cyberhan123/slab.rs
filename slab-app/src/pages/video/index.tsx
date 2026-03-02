import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Film } from 'lucide-react';

export default function Video() {
  return (
    <div className="container mx-auto px-4 py-8 space-y-8">
      <div className="text-center space-y-4">
        <h1 className="text-3xl font-bold text-center">Video</h1>
        <p className="text-muted-foreground max-w-2xl mx-auto">
          Video processing features are coming soon
        </p>
      </div>

      <Card className="max-w-2xl mx-auto">
        <CardHeader>
          <CardTitle>Video Processing</CardTitle>
          <CardDescription>Process and transcribe video files</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex flex-col items-center justify-center py-16 space-y-4 text-muted-foreground">
            <Film className="h-16 w-16 opacity-30" />
            <p className="text-sm">Video processing is not yet available</p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}