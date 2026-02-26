import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarMenuItem,
    SidebarHeader,
    SidebarTrigger,
    SidebarMenu,
    SidebarMenuButton
} from "@/components/ui/sidebar"
import { Link } from "react-router-dom"
import { BotMessageSquare } from "lucide-react"

export function AppSidebar() {
    return (
        <Sidebar variant="inset" collapsible="icon">
            <SidebarHeader className="text-center">
                <SidebarTrigger className="-ml-1 h-9 w-9" />
            </SidebarHeader>
            <SidebarContent>
                <SidebarMenu>
                    <SidebarMenuItem>
                        <SidebarMenuButton asChild>
                            <Link to="/">
                               <BotMessageSquare />
                                <span>Chat</span>
                            </Link>
                        </SidebarMenuButton>
                    </SidebarMenuItem>
                </SidebarMenu>
            </SidebarContent>
            <SidebarFooter />
        </Sidebar>
    )
}