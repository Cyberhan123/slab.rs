import { SetupWorkbench } from './components/setup-workbench';
import { useSetup } from './hooks/use-setup';

export default function SetupPage() {
  const state = useSetup();

  return <SetupWorkbench {...state} />;
}
