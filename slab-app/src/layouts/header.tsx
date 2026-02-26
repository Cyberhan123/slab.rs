export default function Header() {
    return (
        <header className="flex h-20 items-center gap-4 border-b bg-background px-4">
           
            <div className="h-6 w-px bg-border md:block hidden" />
            <div className="flex items-center gap-3 py-2">
                <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary">
                    <span className="text-sm font-bold text-primary-foreground">Icon</span>
                </div>
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