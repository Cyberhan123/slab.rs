import { useAudio } from './hooks/use-audio';
import { AudioWorkbench } from './components/audio-workbench';

export default function Audio() {
  const state = useAudio();
  return <AudioWorkbench {...state} />;
}
