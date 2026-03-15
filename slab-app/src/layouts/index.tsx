import { Outlet } from "react-router-dom";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { AppSidebar } from "@/layouts/sidebar";
import { CSSProperties } from "react";
import Header from "@/layouts/header";
import FooterStatusBar from "@/layouts/footer-status-bar";
import { GlobalHeaderProvider } from "@/layouts/global-header-provider";

export default function Layout() {
  return (
    <div className="flex h-screen w-full flex-col overflow-hidden">
      <SidebarProvider
        defaultOpen={false}
        className="min-h-0 flex-1"
        style={
          {
            "--sidebar-width": "12rem",
            "--sidebar-width-icon": "3.25rem",
            "--sidebar-width-mobile": "16rem",
          } as CSSProperties
        }
      >
        <div className="flex min-h-0 w-full flex-1 overflow-hidden">
          <AppSidebar />
          <SidebarInset className="flex h-full min-w-0 flex-col">
            <GlobalHeaderProvider>
              <Header />
              <main className="flex-1 overflow-hidden">
                <Outlet />
              </main>
            </GlobalHeaderProvider>
          </SidebarInset>
        </div>
      </SidebarProvider>
      <FooterStatusBar />
    </div>
  );
}
