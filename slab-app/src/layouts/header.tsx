import { SidebarTrigger } from "@/components/ui/sidebar"
export default function Header() {
    return (
        <header className="flex h-20 items-center gap-4 border-b bg-background px-4">
            <div className="flex items-center gap-3 py-2">
              <SidebarTrigger className="-ml-1 h-9 w-9" />
                <div className="flex flex-col justify-center overflow-hidden">
                    <h2 className="line-clamp-1 text-sm font-semibold leading-none tracking-tight text-foreground">
                        项目标题
                    </h2>
                    <p className="line-clamp-1 text-xs text-muted-foreground mt-1">
                        这里是详细的描述文字
                    </p>
                </div>
            </div>
        </header>
    )
}