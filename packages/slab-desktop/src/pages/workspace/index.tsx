import { WorkspaceWorkbench } from "./components/workspace-workbench"
import { useWorkspacePage } from "./hooks/use-workspace-page"

export default function WorkspacePage() {
  const state = useWorkspacePage()

  return <WorkspaceWorkbench {...state} />
}
