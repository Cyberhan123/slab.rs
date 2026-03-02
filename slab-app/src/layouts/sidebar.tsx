import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarMenuItem,
    SidebarHeader,
    SidebarMenu,
    SidebarMenuButton
} from "@/components/ui/sidebar"
import { Link } from "react-router-dom"
import { BotMessageSquare, ImageIcon, Mic, Film, Package, ClipboardList, Settings } from "lucide-react"

export function AppSidebar() {
    return (
        <Sidebar variant="inset" collapsible="icon">
            <SidebarHeader className="text-center">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary">
                    <span className="text-sm font-bold text-primary-foreground">Slab</span>
                </div>
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
                    <SidebarMenuItem>
                        <SidebarMenuButton asChild>
                            <Link to="/image">
                                <ImageIcon />
                                <span>Image</span>
                            </Link>
                        </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                        <SidebarMenuButton asChild>
                            <Link to="/audio">
                                <Mic />
                                <span>Audio</span>
                            </Link>
                        </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                        <SidebarMenuButton asChild>
                            <Link to="/video">
                                <Film />
                                <span>Video</span>
                            </Link>
                        </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                        <SidebarMenuButton asChild>
                            <Link to="/hub">
                                <Package />
                                <span>Hub</span>
                            </Link>
                        </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                        <SidebarMenuButton asChild>
                            <Link to="/task">
                                <ClipboardList />
                                <span>Tasks</span>
                            </Link>
                        </SidebarMenuButton>
                    </SidebarMenuItem>
                </SidebarMenu>
            </SidebarContent>
            <SidebarFooter>
                <SidebarMenu>
                    <SidebarMenuItem>
                        <SidebarMenuButton asChild>
                            <Link to="/settings">
                                <Settings />
                                <span>Settings</span>
                            </Link>
                        </SidebarMenuButton>
                    </SidebarMenuItem>
                </SidebarMenu>
            </SidebarFooter>
        </Sidebar>
    )
}