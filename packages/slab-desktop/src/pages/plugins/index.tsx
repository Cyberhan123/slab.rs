import { PluginsWorkbench } from './components/plugins-workbench';
import { usePluginsPage } from './hooks/use-plugins-page';

export default function Plugins() {
  const state = usePluginsPage();
  return <PluginsWorkbench {...state} />;
}
