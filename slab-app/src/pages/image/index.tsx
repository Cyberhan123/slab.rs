import { useImageGeneration } from './hooks/use-image-generation';
import { ImageWorkbench } from './components/image-workbench';

export default function ImagePage() {
  const state = useImageGeneration();
  return <ImageWorkbench {...state} />;
}
