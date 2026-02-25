import AppRoutes from "@/routes";
import './styles/globals.css'
import { TooltipProvider } from "@/components/ui/tooltip"
function App() {
  return (
    <TooltipProvider>
      <AppRoutes />
    </TooltipProvider>
  );
}

export default App;
