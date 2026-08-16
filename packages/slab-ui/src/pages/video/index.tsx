import { useVideoGeneration } from './hooks/use-video-generation';
import { VideoWorkbench } from './components/video-workbench';

export default function VideoPage() {
  const state = useVideoGeneration();

  return <VideoWorkbench {...state} />;
}
