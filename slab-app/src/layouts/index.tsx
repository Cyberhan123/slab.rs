import { Outlet } from "react-router-dom"

import { WorkspaceStage } from "@/components/ui/workspace"
import FooterStatusBar from "@/layouts/footer-status-bar"
import { GlobalHeaderProvider } from "@/layouts/global-header-provider"
import Header from "@/layouts/header"
import { AppSidebar } from "@/layouts/sidebar"

export default function Layout() {
  return (
    <div className="workspace-shell h-screen overflow-hidden p-3">
      <div className="flex min-h-0 w-full flex-1 gap-3">
        <GlobalHeaderProvider>
          <AppSidebar />
          <div className="flex min-h-0 min-w-0 flex-1 flex-col gap-3">
            <Header />
            <WorkspaceStage className="min-h-0 flex-1">
              <main className="flex min-h-0 flex-1 overflow-hidden p-[var(--shell-content-gutter)]">
                <Outlet />
              </main>
            </WorkspaceStage>
            <FooterStatusBar />
          </div>
        </GlobalHeaderProvider>
      </div>
    </div>
  )
}
