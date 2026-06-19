# 专项执行计划 · 设计系统重构 (Design System Reconstruction)

| 字段 | 值 |
|---|---|
| Plan ID | E |
| 关联根因 | R5（设计系统碎片化） |
| 上游审计 | [slab-deskotp-audits-2026-6-19.md](../../audits/slab-deskotp-audits-2026-6-19.md) §3.2 |
| 负责域 | slab-components · layouts · 各 workbench · Monaco |
| 状态 | Draft / Pending Review |
| 预估总工作量 | L |

## 1. 目标与边界 (Scope)

- **北极星**：让 Slab Desktop 的视觉系统从「Token 基础扎实但散落魔法值、1px 硬线割裂、三态原语分裂、无动效编排」收敛为**一套单一、自描述、自适应（dark/light + reduced-motion）的 Token + 组件类体系**，在 halo 渐变背景上呈现高级通透感、无边界、轻引导的统一观感。
- **In scope**：
  - 补齐三类缺口 Token（elevation / glass / motion）+ 扩展 text/tracking/radius 尺度，并经 `@theme inline` 暴露给 Tailwind（T-E-1）。
  - sidebar / topbar / footer / header 的 1px 硬分割线 → `--hairline-soft` 软渐变 fade（T-E-2）。
  - 三套分裂的 empty/loading/error 原语 → 统一 `<StateSurface>`；焦点环收敛为 `--focus-ring` + `.focus-ring`（T-E-3）。
  - 全局 `prefers-reduced-motion` 守卫（T-E-4，P0）。
  - Skeleton shimmer 化 + 新增 SkeletonText/Circle + 统一 ~180ms `ease-out-expo` "soft in"（T-E-5）。
  - Monaco ↔ app 主题 Token 映射 + 编辑器面板圆角协调（T-E-6）。
  - 清除 ~200 处 magic-px / raw-hex / 散乱透明度的分批迁移（T-E-7）。
- **Out of scope（移交他计划）**：
  - 媒体 `progress` 进度条 UI 实现细节（属于能力释放）→ **Plan B / T-B-1**（本计划只提供 `--elevation-*` + `.glass-surface` 给进度浮层使用）。
  - 统一错误包络 `{code,message,data,i18n}` 与 `getLocalizedErrorMessage`（属错误层）→ **Plan B / T-B-7**。
  - `MutationCache.onError` / `QueryClient` retry 策略（属 Infra 状态）→ **Plan C / T-C-6**。
  - 媒体历史参数重跑 / 视频 A/B（属交互闭环）→ **Plan D**。
  - 组件库新增业务组件（如 `<Progress>` 卡片）→ 各业务域计划；本计划只提供基础样式原语。
- **Definition of Done**：
  - [ ] T-E-1..T-E-7 全部 AC 通过，`bun run check:frontend && bun run lint` 全绿，`bun run test:components` 与 `bun run test:browser` 快照更新后无回归。
  - [ ] grep 守卫：`shadow-\[0_` 内联阴影计数下降至 ≤ 5（保留 `<canvas>` 像素级特殊阴影）；`text-\[1[0-9]px\]` 与 `rounded-\[(24|28|30|32|34)px\]` 计数归零；`#00685f` 等 raw hex 在 `packages/` 下归零。
  - [ ] dark/light 双主题下，sidebar / topbar / footer / header / 卡片 / empty-state 的分割线均为软渐变（无可见 1px 实线），焦点环视觉一致。
  - [ ] 操作系统层启用 `reduce motion` 后，所有 `animate-pulse`/`animate-spin`/`hover:scale-*` 非必要动画全局停止；功能性过渡保留但 ≤ 1ms。
  - [ ] Monaco 编辑器 chrome（滚动条/边线/选中色/gutter）与外围 Slab 主题在 dark/light 下无可见色差漂移；编辑器面板圆角策略单一（外方内 6px 或外圆内方，二者择一并文档化）。

## 2. 任务卡 (Task Cards)

### T-E-1 · 新增设计 Token（elevation / glass / hairline-soft / radius / text / tracking / dur / ease）+ Tailwind `@theme inline` 扩展映射
- **严重度** P1 · **类型** refactor · **预估** M
- **证据** [slab-components/src/styles/globals.css](packages/slab-components/src/styles/globals.css)（当前 `:root` 仅有 `--shell-*` / `--surface-*`，无 elevation/glass/motion/tracking/text 尺度）、[tailwind.config.js](packages/slab-desktop/tailwind.config.js)（`theme: {}` 空壳，主题全靠 globals.css 的 `@theme inline`）；审计 §3.2.1 的缺口表与 §3.2.6 的提案。
- **问题**：当前 Token 层已有 halo/teal+gold 径向渐变与 `--shell-elevation`（单层），但缺三类关键 Token：(1) 多层 elevation（~30 处内联 `shadow-[0_…]` 各自为政）；(2) glass（`/45 /55 /72 /80 /85 /92 /95` 七档散乱）；(3) motion（83 处 transition/animation 裸 Tailwind 默认 ease，无 `--dur-*`/`--ease-*`）。同时缺 text/tracking/radius 的命名尺度（138 处字号 magic-px、5 档字距 magic-em、30+ 处圆角 magic-px）。这是 T-E-2/3/5/6/7 的前置。
- **方案**：
  1. 在 `globals.css` 的 `@layer base { :root { … } }` 末尾（紧接 `--media-canvas:` 之后、闭合 `}` 之前）追加以下 Token 块（直接采用审计 §3.2.6 的 oklch/color-mix 版本，已核对与现有 `--shell-card` / `--foreground` / `--shell-line` 引用一致）：

     ```css
     /* ── Elevation（替换 ~30 处内联阴影；保留单层 --shell-elevation 不动） ── */
     --elevation-1: 0 1px 2px oklch(0% 0 0 / 0.04);
     --elevation-2: 0 18px 44px -30px color-mix(in oklab, var(--foreground) 28%, transparent);
     --elevation-3: 0 32px 80px -48px color-mix(in oklab, var(--foreground) 40%, transparent);

     /* ── Glass（替换 /45 /55 /72 /80 /85 /92 /95 散乱透明度） ── */
     --glass-bg:        color-mix(in oklab, var(--shell-card) 62%, transparent);
     --glass-bg-strong: color-mix(in oklab, var(--shell-card) 80%, transparent);
     --glass-border:    color-mix(in oklab, var(--shell-card) 32%, transparent);
     --glass-blur:      14px;
     --glass-highlight: inset 0 1px 0 color-mix(in oklab, var(--shell-card) 55%, transparent);

     /* ── 软发丝线（替换 1px 实线，T-E-2 直接消费） ── */
     --hairline-soft: linear-gradient(var(--shell-line), transparent);

     /* ── 圆角尺度扩展（消除 rounded-[24/28/30/32/34px]） ── */
     --radius-2xl: 1.5rem;   /* 24px */
     --radius-3xl: 2rem;     /* 32px */

     /* ── 字号尺度（消除 138 处 text-[10/11/12/13/17px]） ── */
     --text-micro:   10px;
     --text-caption: 11px;
     --text-label:   12px;
     --text-body:    13px;

     /* ── 字距尺度（消除 tracking-[-0.025/-0.04/-0.045/-0.05/-0.055em] 与 [0.12/0.16/0.22em]） ── */
     --tracking-display: -0.05em;  /* 大标题光学收紧 */
     --tracking-eyebrow: 0.16em;   /* uppercase eyebrow 拉开 */

     /* ── 动效（消除裸 Tailwind ease；T-E-4/T-E-5 消费） ── */
     --dur-120: 120ms;
     --dur-180: 180ms;
     --dur-240: 240ms;
     --ease-out-expo: cubic-bezier(0.16, 1, 0.3, 1);
     --ease-soft:     cubic-bezier(0.22, 0.61, 0.36, 1);

     /* ── 焦点环（T-E-3 统一 Button/sidebar/卡片/行） ── */
     --focus-ring: 0 0 0 2px var(--background), 0 0 0 4px color-mix(in oklab, var(--brand-teal) 55%, transparent);
     ```
     注意：**不重复定义 `--shell-elevation`**（保留单层老 Token 兼容存量引用），elevation-1/2/3 是新增梯度。`--glass-bg-strong` 在审计 §3.2.6 基础上补充（header pills 需要 92% 强透明，旧代码 `bg-[var(--shell-card)]/92` 直接对应）。

  2. 在 `@theme inline { … }` 块（globals.css:517）末尾追加 Tailwind 映射，使上述 Token 可作为 Tailwind 工具类使用（`shadow-elevation-2`、`bg-glass-bg`、`text-micro`、`tracking-display`、`rounded-2xl`、`duration-180`、`ease-out-expo`）：

     ```css
     /* Elevation → shadow-elevation-{1,2,3} */
     --shadow-elevation-1: var(--elevation-1);
     --shadow-elevation-2: var(--elevation-2);
     --shadow-elevation-3: var(--elevation-3);

     /* Glass → bg-glass-bg / bg-glass-bg-strong / border-glass-border */
     --color-glass-bg: var(--glass-bg);
     --color-glass-bg-strong: var(--glass-bg-strong);
     --color-glass-border: var(--glass-border);

     /* Hairline（工具类 .border-hairline-soft 见 components 层） */
     --color-hairline: var(--shell-line);

     /* Radius 尺度（补 2xl/3xl，与 --radius-xl 并列） */
     --radius-2xl: 1.5rem;
     --radius-3xl: 2rem;

     /* Text 尺度（Tailwind v4 原生支持 --text-* → text-micro/caption/label/body） */
     --text-micro:   10px;
     --text-caption: 11px;
     --text-label:   12px;
     --text-body:    13px;

     /* Tracking → tracking-display / tracking-eyebrow */
     --tracking-display: -0.05em;
     --tracking-eyebrow: 0.16em;

     /* 动效 → duration-180 / ease-out-expo */
     --duration-120: var(--dur-120);
     --duration-180: var(--dur-180);
     --duration-240: var(--dur-240);
     --ease-out-expo: cubic-bezier(0.16, 1, 0.3, 1);
     --ease-soft:     cubic-bezier(0.22, 0.61, 0.36, 1);
     ```
     （Tailwind v4 中 `--text-*` / `--tracking-*` / `--radius-*` / `--shadow-*` / `--ease-*` / `--duration-*` 均为命名空间 Token，写入 `@theme` 即自动生成对应工具类，无需改 `tailwind.config.js`。）

  3. 在 `@layer components` 内新增两个组件类（消费 glass 与 hairline Token，供 T-E-2/T-E-5 与业务代码直接用类名替代内联）：

     ```css
     /* 玻璃面（header pills / hub 过滤栏 / image·video 浮动工具栏 / StageEmptyState / Dialog） */
     .glass-surface {
       background: var(--glass-bg);
       border: 1px solid var(--glass-border);
       box-shadow: var(--glass-highlight), var(--elevation-2);
       backdrop-filter: blur(var(--glass-blur));
       -webkit-backdrop-filter: blur(var(--glass-blur));
     }
     .glass-surface-strong {
       background: var(--glass-bg-strong);
       border: 1px solid var(--glass-border);
       box-shadow: var(--glass-highlight), var(--elevation-2);
       backdrop-filter: blur(var(--glass-blur));
       -webkit-backdrop-filter: blur(var(--glass-blur));
     }

     /* 软发丝分割线（垂直方向：sidebar inset / header 竖线 span；水平方向：topbar border-bottom / footer border-top） */
     .hairline-v {
       /* 用于竖线 span：width 1px，背景是上下 fade */
       background: linear-gradient(to bottom, transparent, var(--shell-line) 20%, var(--shell-line) 80%, transparent);
     }
     .hairline-h-top {
       border-top: none;
       background-image: var(--hairline-soft);
       background-repeat: no-repeat;
       background-position: top center;
       background-size: 100% 6px;
     }
     .hairline-h-bottom {
       border-bottom: none;
       background-image: var(--hairline-soft);
       background-repeat: no-repeat;
       background-position: bottom center;
       background-size: 100% 6px;
     }
     .hairline-inset-right {
       box-shadow: inset -1px 0 0 transparent;
       background-image: linear-gradient(to right, transparent, var(--shell-line) 20%, var(--shell-line) 80%, transparent);
       background-position: right;
       background-size: 6px 100%;
       background-repeat: no-repeat;
     }

     /* 焦点环（T-E-3 覆盖 Button/sidebar/卡片/行） */
     .focus-ring {
       outline: none;
     }
     .focus-ring:focus-visible {
       box-shadow: var(--focus-ring);
       border-color: transparent;
     }
     ```
     注意 `.hairline-h-*` 用 `background-image` 叠加在原元素背景上，需保留元素本身背景色（topbar 的 `linear-gradient + var(--shell-topbar-bg)` 复合背景会被覆盖，因此 T-E-2 采用更稳妥的「inset shadow fade」替代方案，见 T-E-2 方案）。

  4. **Tailwind config 不动**：保持 `tailwind.config.js` 为兼容 stub（注释已说明主题全在 globals.css），避免维护第二份发散的 JS 主题源（审计 §3.2 已确认这是有意设计）。

- **验收标准 (AC)**：
  - [ ] `globals.css` 的 `:root` 包含上述全部新增 Token；dark 主题（`.dark`）下 elevation/glass 因 `--foreground`/`--shell-card` 自动取暗色变体，无需重复定义（手动验证 dark 模式无破相）。
  - [ ] `@theme inline` 块暴露了 `shadow-elevation-{1,2,3}`、`bg-glass-bg`、`text-micro/caption/label/body`、`tracking-display/eyebrow`、`rounded-2xl/3xl`、`duration-180`、`ease-out-expo` 工具类（在临时 demo 页面写 `className="shadow-elevation-2 bg-glass-bg text-label tracking-eyebrow rounded-3xl duration-180 ease-out-expo"` 编译通过且应用生效）。
  - [ ] 新增 `.glass-surface` / `.glass-surface-strong` / `.hairline-*` / `.focus-ring` 组件类，浏览器 DevTools 计算样式显示其引用了对应 CSS 变量。
  - [ ] `--shell-elevation`（老 Token）保留未被删除，存量引用不报错（grep `var(--shell-elevation)` 命中数不变）。
  - [ ] `bun run check:frontend` 无类型错误（globals.css 非 TS，但 Tailwind 工具类需在 `cn()` 调用处不报未知类名，若项目有 tailwind-lint 则全绿）。
- **依赖**：无（本卡是其余多数卡的前置）。建议与 T-E-4 同 PR 落地（T-E-4 消费 `--dur-*`/`--ease-*`）。

### T-E-2 · 硬 1px 分割线 → `--hairline-soft` 软渐变（sidebar inset / topbar·footer border / header 竖线）
- **严重度** P1 · **类型** refactor · **预估** S
- **证据** [sidebar.tsx:117](packages/slab-desktop/src/layouts/sidebar.tsx#L117)（`shadow-[inset_-1px_0_0_var(--shell-line)]`）、globals.css:662（`.shell-topbar { border-bottom: 1px solid var(--shell-line); }`）、globals.css:684（`.shell-footer-bar { border-top: 1px solid var(--shell-line); }`）、[header.tsx:95](packages/slab-desktop/src/layouts/header.tsx#L95)/[130](packages/slab-desktop/src/layouts/header.tsx#L130)/[137](packages/slab-desktop/src/layouts/header.tsx#L137)（`<span className="h-4 w-px bg-[var(--shell-divider)]">`）。
- **问题**：上述 1px 实线在 halo 渐变背景上显得沉重、割裂，与"通透/无边界"设计方向冲突（审计 §3.2.2 评定为"通透感最大单点收益"）。`--shell-line` 在 dark 模式透明度仅 0.08，实线几乎不可见，反而暴露"此处有一条线但看不清"的廉价感。
- **方案**（采用 inset shadow fade，避免破坏 topbar 的复合渐变背景）：
  1. **sidebar inset 竖线**（[sidebar.tsx:117](packages/slab-desktop/src/layouts/sidebar.tsx#L117)）：将
     ```tsx
     !isChatVariant && "shadow-[inset_-1px_0_0_var(--shell-line)]"
     ```
     改为
     ```tsx
     !isChatVariant && "shadow-[inset_-6px_0_6px_-6px_var(--shell-line)]"
     ```
     （6px 宽的 inset 软阴影，从右侧向内 fade，视觉上是"渗入"而非"撞上"。若 T-E-1 的 `.hairline-inset-right` 类已落地，可直接换为 `!isChatVariant && "hairline-inset-right"`，但 sidebar 已有 `shadow-[var(--shell-elevation)]` 等其它 shadow，叠加需用复合 shadow 字符串，故优先用内联 arbitrary 保证不冲突。）

  2. **topbar border-bottom**（globals.css:662 `.shell-topbar`）：删除 `border-bottom: 1px solid var(--shell-line);`，改为
     ```css
     .shell-topbar {
       /* … 保留原 background / backdrop-filter / app-region … */
       border-bottom: 1px solid transparent;
       box-shadow: inset 0 -6px 6px -6px var(--shell-line);
     }
     ```
     （保留 `border-bottom: 1px solid transparent` 占位以维持盒模型高度不变；inset 软阴影从底部向上 fade。）

  3. **footer border-top**（globals.css:684 `.shell-footer-bar`）：同理，
     ```css
     .shell-footer-bar {
       background: var(--shell-footer-bg);
       border-top: 1px solid transparent;
       box-shadow: inset 0 6px 6px -6px var(--shell-line);
     }
     ```

  4. **header 竖线 span ×3**（[header.tsx:95/130/137](packages/slab-desktop/src/layouts/header.tsx#L137)）：将
     ```tsx
     <span className="hidden h-4 w-px shrink-0 bg-[var(--shell-divider)] sm:block" />
     ```
     改为（三处统一）
     ```tsx
     <span
       aria-hidden="true"
       className="hidden h-4 w-px shrink-0 sm:block"
       style={{
         background:
           "linear-gradient(to bottom, transparent, var(--shell-divider) 25%, var(--shell-divider) 75%, transparent)",
       }}
     />
     ```
     或更简洁地使用 T-E-1 落地的 `.hairline-v` 类：
     ```tsx
     <span aria-hidden="true" className="hairline-v hidden h-4 w-px shrink-0 sm:block" />
     ```
     （`.hairline-v` 的 gradient 已含上下 fade，`--shell-divider` 是 audit 指定的竖线色 Token，保持语义一致。）

  5. **dark 主题验证**：`--shell-line` dark 透明度 0.08 偏弱，软渐变后可能几乎不可见。若 dark 下视觉过弱，在 `.dark` 块追加 `--shell-line` 提升至 0.12（或新增 `--shell-line-strong` 仅供 hairline 使用）。决策点：优先保持单 Token，调暗色透明度；若影响其它 `--shell-line` 引用（如 sidebar-border），再拆分。

- **验收标准 (AC)**：
  - [ ] dark/light 双主题下，sidebar 与内容区交界、topbar 底部、footer 顶部、header 三处竖线分隔均为 ~6px 渐变 fade，无可见 1px 实线硬边。
  - [ ] sidebar/topbar/footer 盒模型高度不变（`border-bottom/top: transparent` 占位），布局无位移回归。
  - [ ] header 三处竖线在 `sm:` 断点以下仍隐藏（`hidden … sm:block` 保留）。
  - [ ] grep `1px solid var(--shell-line)` 在 globals.css 仅剩（如有）`--sidebar-border` 定义处，shell chrome 三处已迁移。
- **依赖**：T-E-1（提供 `--hairline-soft` 与 `.hairline-v` 组件类；若 T-E-1 未合并，可先用内联 `linear-gradient(...)` 临时实现，但建议合并）。

### T-E-3 · 统一 `<StateSurface variant="empty|loading|error">` + `--focus-ring` Token 与 `.focus-ring` 工具类
- **严重度** P1 · **类型** refactor · **预估** M
- **证据** [empty.tsx](packages/slab-components/src/empty.tsx)（`Empty` 组件，`border-dashed` + `text-balance`，无 halo/玻璃）、[empty-panel.tsx](packages/slab-desktop/src/pages/plugins/components/empty-panel.tsx)（`EmptyPanel`，`rounded-[24px]` + 独立 shadow + `/45` 玻璃）、[workspace.tsx:176](packages/slab-components/src/workspace.tsx#L176)（`StageEmptyState`，`workspace-surface workspace-halo` + `rounded-[32px]` + `min-h-[360px]`）、[button.tsx:8](packages/slab-components/src/button.tsx#L8)（`focus-visible:ring-ring/50 focus-visible:ring-[3px]`）、[sidebar.tsx:94](packages/slab-desktop/src/layouts/sidebar.tsx#L94)（`focus-visible:ring-2 focus-visible:ring-[color-mix(in_oklab,var(--brand-teal)_28%,transparent)]`）；hub/plugin 卡片与历史行无可见焦点环（审计 §3.2.4）。
- **问题**：(1) 三套空/载/错原语视觉分裂：`Empty` 散文式 dashed 边、`EmptyPanel` 中型卡片、`StageEmptyState` 大型 halo 卡片，加载态分别为散文块 / spinner / skeleton grid 各异，且均不支持 `variant="loading|error"`。(2) 焦点环不统一：Button 用 `ring-[3px] ring-ring/50`，sidebar 用 `ring-2 color-mix 28%`，卡片/行无焦点环 → 键盘可达性与视觉一致性双输。
- **方案**：
  1. **新增 `<StateSurface>`**（在 `packages/slab-components/src/state-surface.tsx`）：

     ```tsx
     import { type ComponentProps, type ReactNode, type ComponentType } from "react"
     import { cva, type VariantProps } from "class-variance-authority"
     import { cn } from "./lib/utils"
     import { Loader2, TriangleAlert, Inbox } from "lucide-react"

     const stateSurfaceVariants = cva(
       "glass-surface flex flex-col items-center justify-center gap-5 px-6 py-12 text-center text-balance",
       {
         variants: {
           size: {
             compact: "min-h-[160px] rounded-2xl",   /* 对应旧 EmptyPanel */
             default: "min-h-[240px] rounded-3xl",   /* 对应旧 Empty 中态 */
             stage:   "min-h-[360px] rounded-3xl",   /* 对应旧 StageEmptyState */
           },
         },
         defaultVariants: { size: "default" },
       },
     )

     type StateSurfaceProps = ComponentProps<"div"> &
       VariantProps<typeof stateSurfaceVariants> & {
         variant: "empty" | "loading" | "error"
         icon?: ComponentType<{ className?: string }>
         title: ReactNode
         description?: ReactNode
         action?: ReactNode
       }

     const defaultIcons = {
       empty: Inbox,
       loading: Loader2,
       error: TriangleAlert,
     } as const

     export function StateSurface({
       variant,
       size,
       icon: Icon = defaultIcons[variant],
       title,
       description,
       action,
       className,
       ...props
     }: StateSurfaceProps) {
       const isLoading = variant === "loading"
       return (
         <div
           data-slot="state-surface"
           data-variant={variant}
           className={cn(stateSurfaceVariants({ size }), className)}
           role={variant === "loading" ? "status" : undefined}
           aria-live={variant === "error" ? "assertive" : "polite"}
           {...props}
         >
           <div className="flex size-16 shrink-0 items-center justify-center rounded-2xl bg-[var(--glass-bg-strong)] text-muted-foreground">
             <Icon className={cn("size-7", isLoading && "animate-spin")} />
           </div>
           <div className="space-y-2">
             <h3 className="text-lg font-semibold tracking-tight text-foreground">{title}</h3>
             {description ? (
               <p className="mx-auto max-w-md text-sm leading-6 text-muted-foreground">{description}</p>
             ) : null}
           </div>
           {action ? <div className="mt-1">{action}</div> : null}
         </div>
       )
     }
     ```
     （统一消费 T-E-1 的 `.glass-surface` + `--elevation-2`；`animate-spin` 受 T-E-4 守卫；尺寸三档覆盖旧三组件的全部用例。）

  2. **旧组件保留为薄包装**（避免一次性破坏全部调用点）：在 `empty.tsx` 内将 `Empty` 标注 `@deprecated`，新增 re-export `export { StateSurface } from "./state-surface"`；将 `EmptyPanel`、`StageEmptyState` 改为 `<StateSurface size="compact" variant="empty" … />` 与 `<StateSurface size="stage" variant="empty" … />` 的薄包装（保留 props 签名，内部委托）。这样调用方可渐进迁移，grep 老组件名得到收敛路线图。

  3. **`.focus-ring` 全量覆盖**：在 `button.tsx` 的 `buttonVariants` 基础串（:8）删除 `focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]`，改为 `focus-ring` 类（T-E-1 定义）。在 `sidebar.tsx` 的 Link `className`（:94）删除 `focus-visible:ring-2 focus-visible:ring-[color-mix…] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--shell-rail-bg)]`，改为 `focus-ring`。对所有 hub/plugin 卡片、历史行（image/video/audio/task 历史 row）补 `focus-ring` 类（grep `<button` / `role="row"` / `tabIndex={0}` 批量补，作为 T-E-7 的一个子批次）。

  4. **`--focus-ring` Token 设计**：采用 `0 0 0 2px var(--background), 0 0 0 4px color-mix(brand-teal 55%, transparent)` 双层（外层 brand-teal 半透明环 + 内层背景色 gap），既保证暗色下可见，又不在浅色 halo 上过重。`.focus-ring:focus-visible { box-shadow: var(--focus-ring); }` 已在 T-E-1 定义；注意 Button 等 cva 基础串已含 `outline-none`，与新类不冲突。

- **验收标准 (AC)**：
  - [ ] `packages/slab-components/src/state-surface.tsx` 存在并导出 `StateSurface`，三 size × 三 variant 组合视觉自洽（dark/light）。
  - [ ] `Empty` / `EmptyPanel` / `StageEmptyState` 三者均委托 `StateSurface`（薄包装保留旧 API），新增调用点默认用 `StateSurface`。
  - [ ] Button（所有 variant）、sidebar Link、hub/plugin 卡片、历史行在键盘 Tab 聚焦时显示统一的 `--focus-ring` 双层环；暗色下清晰可见。
  - [ ] `role="status"`（loading）与 `aria-live="assertive"`（error）正确设置，屏幕阅读器播报。
  - [ ] grep `focus-visible:ring-\[` 在 `packages/` 下降至 ≤ 2（仅保留 input 等非 Button 场景的特殊环）。
- **依赖**：T-E-1（`.glass-surface` / `.focus-ring` / `--focus-ring`）。可与 T-E-2 并行。

### T-E-4 · 全局 `prefers-reduced-motion` 守卫（禁用 pulse/spin/scale 等非必要动画）
- **严重度** P0 · **类型** infra · **预估** S
- **证据** 审计 §3.2.3 grep 零命中（`slab-desktop/src` 与 `slab-components/src` 全域无 `prefers-reduced-motion`）；P0#9。
- **问题**：全域 `animate-pulse`（Skeleton 等）/ `animate-spin`（Loader2 等）/ `hover:scale-105` / `hover:-translate-y-px`（sidebar）/ `transition-*` 无条件运行，前庭功能敏感用户无缓解选项，且是审计评定的 9 个 P0 之一（无障碍/品牌质量缺陷）。
- **方案**：
  1. 在 `globals.css` 末尾（`@layer base` 内，`body` 规则之后）新增全局守卫：

     ```css
     @layer base {
       @media (prefers-reduced-motion: reduce) {
         *,
         *::before,
         *::after {
           animation-duration: 0.01ms !important;
           animation-iteration-count: 1 !important;
           transition-duration: 0.01ms !important;
           scroll-behavior: auto !important;
         }
         /* 显式归零 Tailwind 的 animate-* 实用类（pulse/spin/bounce/ping） */
         .animate-pulse,
         .animate-spin,
         .animate-bounce,
         .animate-ping {
           animation: none !important;
         }
         /* hover 位移/缩放归零（保留颜色变化作为反馈） */
         .hover\:scale-105:hover,
         .hover\:-translate-y-px:hover,
         .hover\:scale-110:hover {
           transform: none !important;
         }
       }
     }
     ```
     （采用业界标准 `0.01ms` 而非 `0ms`，避免依赖 `transitionend`/`animationend` 事件的组件（如 radix Dialog/Sheet 的 unmount）因事件永不触发而卡死。`!important` 是必要的，因为 Tailwind 工具类的优先级高于媒体查询内普通规则。）

  2. **功能性动画白名单**（可选，第二阶段）：若某些动画是功能必需（如进度条 width 过渡、拖拽反馈），用 `motion-safe:` Tailwind 前缀包裹这些类（`motion-safe:transition-[width]`），确保守卫不影响功能性。本卡只做全局守卫，白名单留作 T-E-5 动效编排时一并处理。

  3. **测试**：在 macOS「系统设置 → 辅助功能 → 显示 → 减少动态效果」与 Windows「设置 → 辅助功能 → 视觉效果 → 动画效果」启用后，人工走查 sidebar / Skeleton / Loader2 / hover 卡片 / Dialog 进出，确认非必要动画停止。

- **验收标准 (AC)**：
  - [ ] grep `prefers-reduced-motion` 在 `packages/` 下命中 ≥ 1（globals.css 守卫）。
  - [ ] OS 启用 reduce motion 后：Skeleton 不 pulse、Loader2 不 spin、卡片 hover 不 scale/translate、Dialog 进出无位移动画（仅瞬时切换）。
  - [ ] OS 未启用时，所有动画行为与改动前一致（无回归）。
  - [ ] radix Dialog/Sheet 关闭路径在 reduce motion 下不卡死（`0.01ms` 保证 `animationend` 仍触发）。
- **依赖**：无（独立可先行；审计 Phase 0.6 已标注）。建议与 T-E-1 同 PR（T-E-1 提供的 `--dur-*` 可在未来让守卫更精细，但本卡不依赖）。

### T-E-5 · Skeleton 升级 shimmer 渐变 + 新增 SkeletonText/Circle；统一动效编排（Dialog/Sheet/Card/EmptyState 的 ~180ms ease-out-expo opacity+translateY "soft in"）
- **严重度** P2 · **类型** refactor · **预估** M
- **证据** [skeleton.tsx:7](packages/slab-components/src/skeleton.tsx#L7)（`bg-accent animate-pulse rounded-md`，flat 无 shimmer）；审计 §3.2.3（83 处 transition/animation 裸 Tailwind 默认 ease，无 enter/exit 编排）。
- **问题**：(1) Skeleton 是 flat `bg-accent animate-pulse`，与通透美学不符（应方向性 shimmer）。(2) 全域无统一的"渐入"编排，Dialog/Sheet/Card/EmptyState 进场各异或无动画，brief 要求的"轻量化视线引导"缺失。
- **方案**：
  1. **Skeleton shimmer 化**：在 `globals.css` 的 `@layer components` 新增 shimmer 关键帧与类：

     ```css
     @layer components {
       @keyframes slab-shimmer {
         0%   { background-position: -200% 0; }
         100% { background-position: 200% 0; }
       }
       .skeleton-shimmer {
         background-image: linear-gradient(
           90deg,
           var(--accent) 0%,
           color-mix(in oklab, var(--accent) 60%, var(--surface-1)) 50%,
           var(--accent) 100%
         );
         background-size: 200% 100%;
         animation: slab-shimmer var(--dur-1800, 1.6s) var(--ease-soft) infinite;
       }
       @media (prefers-reduced-motion: reduce) {
         .skeleton-shimmer { animation: none; }
       }
     }
     ```
     （新增 `--dur-1800: 1.6s` 到 T-E-1 的 motion Token，或直接内联 1.6s；shimmer 慢于 pulse 更高级。reduce motion 下显式停止。）

  2. **Skeleton 组件升级 + 新增 SkeletonText/Circle**（`packages/slab-components/src/skeleton.tsx`）：

     ```tsx
     function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
       return (
         <div
           data-slot="skeleton"
           className={cn("skeleton-shimmer rounded-md", className)}
           {...props}
         />
       )
     }

     function SkeletonText({ lines = 3, className }: { lines?: number; className?: string }) {
       return (
         <div data-slot="skeleton-text" className={cn("flex flex-col gap-2", className)}>
           {Array.from({ length: lines }).map((_, i) => (
             <Skeleton
               key={i}
               className={cn("h-3", i === lines - 1 ? "w-2/3" : "w-full")}
             />
           ))}
         </div>
       )
     }

     function SkeletonCircle({ className, ...props }: React.ComponentProps<"div">) {
       return (
         <div
           data-slot="skeleton-circle"
           className={cn("skeleton-shimmer size-10 rounded-full", className)}
           {...props}
         />
       )
     }

     export { Skeleton, SkeletonText, SkeletonCircle }
     ```
     （删除 `animate-pulse`，用 `.skeleton-shimmer`；`SkeletonText` 覆盖文章/消息骨架，`SkeletonCircle` 覆盖头像/icon 占位。）

  3. **统一 "soft in" 编排**：在 `globals.css` 新增 `.soft-in` 工具类（供 Dialog/Sheet/Card/EmptyState 进场）：

     ```css
     @layer components {
       @keyframes slab-soft-in {
         from { opacity: 0; transform: translateY(8px); }
         to   { opacity: 1; transform: translateY(0); }
       }
       .soft-in {
         animation: slab-soft-in var(--dur-180) var(--ease-out-expo) both;
       }
       @media (prefers-reduced-motion: reduce) {
         .soft-in { animation: none; }
       }
     }
     ```
     在 radix Dialog/Sheet 的 `Content` 组件、Card 进场、`StateSurface`（T-E-3）的根 div 追加 `soft-in` 类。注意 radix 已内置 enter/exit 动画，本类作为 supplement 而非替换；若 radix 动画已足够，仅在 EmptyState/Card 等无内置动画处施加。

  4. **替换裸 Tailwind 默认 ease**（83 处中的高频路径）：将 `transition-all` / `transition-colors` 后跟无 `duration-*`/`ease-*` 的，统一补 `duration-180 ease-out-expo`（消费 T-E-1 Token）。批量 sed 脚本作为 T-E-7 的一个子批次，本卡只定义规范并提供 `.soft-in` 与 shimmer。

- **验收标准 (AC)**：
  - [ ] `Skeleton` / `SkeletonText` / `SkeletonCircle` 三组件存在；`animate-pulse` 在 skeleton.tsx 已移除，改用 `.skeleton-shimmer`。
  - [ ] shimmer 在 dark/light 下均为方向性渐变（左→右扫光），reduce motion 下停止。
  - [ ] `.soft-in` 类存在；`StateSurface` 与至少 2 个高频 Dialog/Sheet 应用后进场为 ~180ms opacity+translateY。
  - [ ] reduce motion 下 `.soft-in` 与 shimmer 均停止（T-E-4 守卫 + 本卡显式 `@media` 双保险）。
- **依赖**：T-E-1（`--ease-out-expo` / `--ease-soft` / `--dur-180`）；T-E-4（reduce motion 守卫；本卡也加了显式 `@media` 作为双保险）。

### T-E-6 · Monaco ↔ app 主题 Token 映射（注册 VS Code theme contribution）+ 编辑器面板圆角协调
- **严重度** P1 · **类型** refactor · **预估** M
- **证据** [use-workspace-page.ts:174](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L174)（`updateEditorTheme` 仅在 `vs` / `vs-dark` 基础切换，监听 `<html>.dark`）；[workspace-workbench.tsx:402](packages/slab-desktop/src/pages/workspace/components/workspace-workbench.tsx#L402)（`<SoftPanel className="… rounded-[18px] p-0">` 内嵌方形 VS Code 编辑器，外圆内方视觉割裂）；审计 §3.2.5。
- **问题**：(1) Monaco 用 VS Code 自带 theme service（Seti 默认），与 Slab `antd-style`/Tailwind 主题仅在 `vs`/`vs-dark` 基础切换同步，token 配色（关键字/字符串/注释/选中色/gutter/滚动条）会与外围漂移。(2) `SoftPanel rounded-[18px]` 外圆内嵌方形编辑器，圆角处露出方形角，视觉割裂。
- **方案**：
  1. **定义 Slab 自定义 Monaco theme**（在 `packages/slab-desktop/src/pages/workspace/lib/monaco-theme.ts` 新增）：

     ```ts
     import type { editor } from "monaco-editor"

     // 将 Slab 设计 Token 映射到 Monaco theme rules
     // 读 CSS 变量需在运行时（document.documentElement 计算），故返回工厂函数
     export function buildSlabMonacoTheme(isDark: boolean): editor.IStandaloneThemeData {
       const root = document.documentElement
       const css = (name: string) => getComputedStyle(root).getPropertyValue(name).trim()
       return {
         base: isDark ? "vs-dark" : "vs",
         inherit: true,
         rules: [
           { token: "comment", foreground: css("--muted-foreground"), fontStyle: "italic" },
           { token: "keyword", foreground: css("--brand-teal") },
           { token: "string", foreground: css("--brand-gold") },
           { token: "number", foreground: css("--chart-2") },
           { token: "type", foreground: css("--chart-4") },
           { token: "function", foreground: css("--primary") },
         ],
       colors: {
           "editor.background": css("--surface-1"),
           "editor.foreground": css("--foreground"),
           "editorLineNumber.foreground": css("--muted-foreground"),
           "editor.selectionBackground": css("--surface-selected"),
           "editor.lineHighlightBackground": css("--surface-soft"),
           "editorCursor.foreground": css("--brand-teal"),
           "editorWhitespace.foreground": css("--shell-line"),
           "editorIndentGuide.background1": css("--shell-line"),
           "scrollbarSlider.background": css("--shell-divider"),
           "editorGutter.background": css("--surface-1"),
         },
       }
     }

     export const SLAB_MONACO_THEME_ID = "slab-light"
     export const SLAB_MONACO_THEME_ID_DARK = "slab-dark"
     ```
     （所有颜色读 CSS 变量，dark/light 自动跟随 Slab 主题切换，无需维护两份硬编码色表。）

  2. **注册并切换**（修改 [use-workspace-page.ts:174](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L174) 的 `updateEditorTheme`）：

     ```ts
     const updateEditorTheme = () => {
       const isDark = document.documentElement.classList.contains("dark")
       const id = isDark ? SLAB_MONACO_THEME_ID_DARK : SLAB_MONACO_THEME_ID
       // monaco 已在 workspace-lsp 初始化时 defineTheme 过；这里只切
       monaco.editor.setTheme(id)
       // 主题 Token 变化时（如未来支持运行时换肤）重定义
       monaco.editor.defineTheme(id, buildSlabMonacoTheme(isDark))
       monaco.editor.setTheme(id)
       setEditorTheme(id)
     }
     ```
     并在 Monaco 首次初始化（`ensureWorkspaceLspServices` 或 `workspace-lsp.ts`）时 `defineTheme` 两个主题。`editorTheme` state 默认值改为 `SLAB_MONACO_THEME_ID`。

  3. **编辑器面板圆角协调**（[workspace-workbench.tsx:402](packages/slab-desktop/src/pages/workspace/components/workspace-workbench.tsx#L402)）：决策采用**外圆内 6px**（保留 SoftPanel 的 `rounded-[18px]` → 改为 `rounded-2xl`（T-E-1 Token，24px）外层，编辑器容器内层加 `overflow-hidden rounded-[6px]`）：

     ```tsx
     <SoftPanel className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-2xl p-0">
       <div className="flex h-9 shrink-0 items-center justify-between gap-3 …">…</div>
       <div className="min-h-0 flex-1 overflow-hidden rounded-[6px]">
         <WorkspaceDiffEditor … />
       </div>
     </SoftPanel>
     ```
     （`overflow-hidden` 在外层 SoftPanel 裁掉编辑器的方形角；内层 `rounded-[6px]` 让编辑器自身也有微圆角，避免完全贴边时的硬感。`rounded-[18px]` → `rounded-2xl` 是 T-E-7 magic-px 清除的一部分，此处一并完成。）

  4. **滚动条/gutter 验证**：dark 模式下 `--shell-line` 0.08 偏弱，Monaco gutter/indent guide 可能几乎不可见；若如此，临时在 monaco-theme 里用 `color-mix(in oklab, var(--shell-line) 60%, var(--foreground))` 增强对比，而非改全局 Token。

- **验收标准 (AC)**：
  - [ ] Monaco 编辑器背景/前景/关键字/字符串/注释/选中/gutter/滚动条在 dark/light 下均取自 Slab CSS 变量，与外围卡片无明显色差。
  - [ ] 切换系统 dark/light（或 Slab 主题切换）时，Monaco 即时跟随（`MutationObserver` 已存在，扩展即可）。
  - [ ] 编辑器面板外层 `rounded-2xl`、内层 `rounded-[6px]`，dark/light 下无方形角露出。
  - [ ] `editorTheme` state 默认值不再硬编码 `"vs"`/`"vs-dark"`，而是 `SLAB_MONACO_THEME_ID`/`SLAB_MONACO_THEME_ID_DARK`。
- **依赖**：T-E-1（`rounded-2xl` 工具类）；不阻塞其它。

### T-E-7 · 清除 magic-px 迁移：138 字号 / 字距 / 间距 / ~30 内联阴影 / 7 档散乱透明度 / raw hex
- **严重度** P2 · **类型** refactor · **预估** L
- **证据** 审计 §3.2.1 缺口表（138 处字号、5 档字距、~30 内联阴影、7 档透明度、`rounded-[24/28/30/32/34px]` 30+ 处）；[setup-workbench.tsx:88](packages/slab-desktop/src/pages/setup/components/setup-workbench.tsx#L88)（`#00685f`）、[221](packages/slab-desktop/src/pages/setup/components/setup-workbench.tsx#L221)（`shadow-[0px_18px_60px_-30px_rgba(25,28,30,0.18)]`）、[278](packages/slab-desktop/src/pages/setup/components/setup-workbench.tsx#L278)（`shadow-[0_12px_32px_-24px_rgba(25,28,30,0.18)]`）；[hub/index.tsx:142](packages/slab-desktop/src/pages/hub/index.tsx#L142)、[video-workbench.tsx:505](packages/slab-desktop/src/pages/video/components/video-workbench.tsx#L505) 等。
- **问题**：~200 处 magic-px / raw-hex / 散乱透明度导致排版节奏紊乱、阴影深度不一、暗色主题破相（raw hex 不随主题变）。这是 R5 的"长尾"，单点工作量小但总量大，需分批策略避免冲突。
- **方案（分批迁移策略 — 按 token 类型垂直切，每批独立可合并）**：

  **批次 1 · raw hex → Token（最高优先，暗色破相修复）** [P2，预估 S]
  - `setup-workbench.tsx:88` `#00685f` → `var(--brand-teal)`（dark 下自动转亮 teal）；`:89` `bg-[#00685f]/10` → `bg-[color-mix(in_oklab,var(--brand-teal)_12%,transparent)]`。
  - grep `#[0-9a-fA-F]{3,8}` 在 `packages/` 下逐个核对，凡与现有 Token（`--brand-teal` / `--brand-gold` / `--foreground` / `--background` / `--destructive`）语义重合的，替换为 Token；语义唯一的（如特定插画色）保留并注释说明。
  - AC：grep `#[0-9a-fA-F]{6}` 在 `packages/` 下仅剩插画/图标专用色（≤ 5 处，有注释）。

  **批次 2 · 内联 `shadow-[0_…]` → `shadow-elevation-{1,2,3}` 或 `.glass-surface`** [P2，预估 M]
  - 清单（审计列举的 ~30 处，按 workbench 分组）：
    - hub：`hub/index.tsx:142`、`hub-catalog-table.tsx` 卡片阴影。
    - image/video：`image-workbench.tsx`、`video-workbench.tsx:505`、浮动工具栏（→ `.glass-surface`）。
    - audio：`audio-workbench.tsx`。
    - assistant：`assistant-bubble-content.tsx`、`assistant-input.tsx`。
    - workspace：`workspace-workbench.tsx`（除 :402 已在 T-E-6 处理）。
    - setup：`setup-workbench.tsx:221` `shadow-[0px_18px_60px_-30px_rgba(25,28,30,0.18)]` → `shadow-elevation-2`；`:278` `shadow-[0_12px_32px_-24px_rgba(25,28,30,0.18)]` → `shadow-elevation-1`。
    - layouts：sidebar `shadow-[var(--shell-elevation)]` 保留（是 Token，非 magic）；header pills（`shadow-[var(--shell-elevation)]` 同理保留）。
  - 映射规则：`-30px` 以下 spread 的中等阴影 → `elevation-2`；`-24px` 以下小阴影 → `elevation-1`；大模态/对话框 → `elevation-3`；带 `backdrop-filter` 或 `/92` 透明背景的 → 整体换 `.glass-surface`（连 border 一起收敛）。
  - AC：grep `shadow-\[0` 在 `packages/` 下 ≤ 5（仅 canvas/像素级特殊阴影）。

  **批次 3 · 散乱透明度 `/45 /55 /72 /80 /85 /92 /95` → `--glass-*`** [P2，预估 S]
  - grep `bg-\[var\(--shell-card\)\]/[0-9]+` 与 `bg-\[var\(--surface-1\)\]/[0-9]+`：
    - `/92` / `/95` → `bg-glass-bg-strong`（header pills、StageEmptyState）。
    - `/45` / `/55` → `bg-glass-bg`（EmptyPanel、浮动工具栏弱态）。
    - `/72` / `/80` / `/85` → 视上下文归入 strong 或新增中间档（优先复用现有两档，避免再增 Token）。
  - AC：grep `var\(--shell-card\)\]/[0-9]` 与 `var\(--surface-1\)\]/[0-9]` 命中归零或仅剩 glass-bg 无法覆盖的特殊场景（注释说明）。

  **批次 4 · 字号 `text-[10/11/12/13/17px]` → `text-micro/caption/label/body`** [P2，预估 M]
  - 映射：`text-[10px]` → `text-micro`、`text-[11px]` → `text-caption`、`text-[12px]` → `text-label`、`text-[13px]` → `text-body`。
  - `text-[17px]` / `text-[18px]`（header title）→ 评估是否归入 Tailwind 原生 `text-lg`/`text-xl`，或新增 `--text-section`（17px）；本批优先用原生 `text-lg`（18px，视觉等价）。
  - `text-[1.65rem]`（workspace-markdown h1，globals.css:770）→ 内联在 CSS 非类名，改为 `font-size: var(--text-display)` 并新增 `--text-display: 1.65rem` 到 T-E-1（或保留，因为是 markdown 排版非 UI 尺度，本批不动）。
  - 批量替换用 codemod 脚本（ripgrep + sed）按文件批量改，每文件单独 commit 便于 review。
  - AC：grep `text-\[1[0-3]px\]` 命中归零；`text-\[17px\]` 命中归零（迁 text-lg）。

  **批次 5 · 字距 `tracking-[-0.025/-0.04/-0.045/-0.05/-0.055em]` 与 `[0.12/0.16/0.22em]` → `tracking-display/eyebrow`** [P2，预估 S]
  - `-0.04 ~ -0.055em`（大标题收紧）→ `tracking-display`（-0.05em，视觉等价，±0.005em 不可察）。
  - `0.12 ~ 0.22em`（eyebrow uppercase）→ `tracking-eyebrow`（0.16em）；若个别 0.22em 是刻意更宽（如 hero），保留并注释。
  - `-0.025em`（小标题/sidebar label）→ Tailwind 原生 `tracking-tight`（-0.025em，等价）。
  - AC：grep `tracking-\[-0\.0` 与 `tracking-\[0\.1` 命中归零或仅剩注释豁免。

  **批次 6 · 圆角 `rounded-[24/28/30/32/34px]` → `rounded-2xl/3xl`** [P2，预估 S]
  - 映射：`24px` → `rounded-2xl`、`28px` → `rounded-3xl`（或 `rounded-[1.75rem]` 若 28px 必须精确，优先归 3xl）、`30/32px` → `rounded-3xl`（32px 等价）、`34px` → `rounded-3xl`（视觉等价，±2px 不可察）或新增 `--radius-4xl`。
  - `EmptyPanel rounded-[24px]`、`StageEmptyState rounded-[32px]`（已在 T-E-3 用 size variant 覆盖）、`SoftPanel rounded-[18px]`（已在 T-E-6 处理）、`setup rounded-[32px]/[28px]`、`sidebar size-[52px] rounded-[12px]`（12px → `rounded-xl` 或保留）。
  - AC：grep `rounded-\[(24|28|30|32|34)px\]` 命中归零。

  **批次 7 · 间距 `gap-[18px]` / `p-[17px]` / `p-[5px]` → 4px scale** [P3，预估 S，可延后]
  - `gap-[18px]` → `gap-4`（16px）或 `gap-5`（20px），视视觉密度选；`p-[17px]` → `p-4`（16px）；`p-[5px]` → `p-1`（4px）。
  - 这批最主观，建议每处单独 review，不强行 codemod。
  - AC：grep `-\[1[0-9]px\]`（间距上下文）命中下降 ≥ 80%。

  **批次 8 · `.focus-ring` 全量补齐**（与 T-E-3 联动）[P2，预估 S]
  - grep `<button` / `role="row"` / `role="button"` / `tabIndex={0}` 在 hub/plugin/image/video/audio/task 历史 row 上补 `.focus-ring`。
  - AC：键盘 Tab 走查全域，所有可聚焦元素显示统一焦点环。

- **验收标准 (AC，整卡）**：
  - [ ] 批次 1-6 全部完成（批次 7/8 可延后但建议同期）。
  - [ ] grep 守卫：`text-\[1[0-3]px\]`、`rounded-\[(24|28|30|32|34)px\]`、`shadow-\[0`、`#[0-9a-fA-F]{6}`（除豁免）、`var\(--shell-card\)\]/[0-9]+` 命中数均达 AC 阈值。
  - [ ] dark/light 双主题人工走查 8 个页面（assistant/image/video/audio/hub/task/plugins/workspace/setup），无破相、无透明度异常、无硬边。
  - [ ] 组件快照（`test:components` / `test:browser`）更新后全绿。
- **依赖**：T-E-1（全部 Token）、T-E-3（`.focus-ring` 用于批次 8）、T-E-6（部分 shadow/radius 已在 T-E-6 处理）。建议在 T-E-1/2/3/6 合并后启动，按批次独立 PR。

## 3. 执行顺序 (Sequencing)

- **M1（立即可行，无依赖）**：
  - **T-E-4**（P0，reduce-motion 守卫）— 独立可先行，单文件 globals.css 追加，最高 ROI 的无障碍修复。建议**第一个 PR**。
  - **T-E-1**（P1，Token + `@theme inline` 映射 + `.glass-surface`/`.hairline-*`/`.focus-ring` 组件类）— 阻塞 T-E-2/3/5/6/7，必须尽早。与 T-E-4 同 PR 或紧随其后。

- **M2（T-E-1 合并后并行）**：
  - **T-E-2**（软分割线，S，layouts + globals.css）。
  - **T-E-3**（StateSurface + focus-ring，M，slab-components + 各调用点薄包装）。
  - **T-E-6**（Monaco 主题 + 圆角，M，workspace 域，独立）。
  - 三者互不阻塞，可三人并行或三个独立 PR。

- **M3（T-E-1 + T-E-4 合并后）**：
  - **T-E-5**（Skeleton shimmer + soft-in，M，slab-components + radix 包装）。

- **M4（T-E-1/2/3/6 合并后，长尾）**：
  - **T-E-7**（magic-px 清除，L，按 8 批次独立 PR）。批次 1/2 可与 M2 并行（raw hex 与 shadow 不依赖组件类），批次 3-6 依赖 T-E-1 Token，批次 8 依赖 T-E-3。

- **关键路径**：`T-E-4`（P0 先行）→ `T-E-1`（阻塞链头）→ `{T-E-2, T-E-3, T-E-6 并行}` → `T-E-5` → `T-E-7 分批`。总工期约 L（M1+M2 ≈ 1.5 周含 review，M3+M4 ≈ 1.5 周分批滚动）。

- **可并行**：T-E-2 / T-E-3 / T-E-6（M2 三人并行）；T-E-7 批次 1-2 与 M2 并行；T-E-7 批次 3-8 各自独立 PR 滚动。

## 4. 风险与缓解

| 风险 | 概率×影响 | 缓解 |
|---|---|---|
| **`@theme inline` 新增 Token 后 Tailwind v4 编译产物膨胀**（每个新 Token 生成一组工具类） | 低×中 | 仅暴露实际使用的 Token（elevation/glass/text/tracking/radius/motion），不暴露 `--glass-blur` 等内部值；CI 检查 CSS 产物体积增量 ≤ 5%。 |
| **`.hairline-h-*` 用 `background-image` 会覆盖 topbar 的复合渐变背景**（topbar `background` 是两层 gradient + var） | 高×高 | T-E-2 采用 inset shadow fade 方案（`box-shadow: inset 0 -6px 6px -6px var(--shell-line)`），不碰 `background`，彻底规避冲突。`.hairline-h-*` 类保留供无复合背景的元素使用。 |
| **Monaco `defineTheme` 在 SSR / 首屏未就绪时 `getComputedStyle` 取空值**（Tauri WebView 冷启动） | 中×中 | `buildSlabMonacoTheme` 在 `useEffect` 内调用（client-only），并 fallback 到 `vs`/`vs-dark` 若 CSS 变量为空；首次 `ensureWorkspaceLspServices` 后再重定义。 |
| **`.focus-ring` 的 `box-shadow` 会覆盖 Button 的 `shadow-elevation-*`**（cta/default variant 有 drop shadow） | 高×中 | `.focus-ring:focus-visible` 的 `box-shadow` 用 `var(--focus-ring)`（双层 ring），覆盖 drop shadow 是预期行为（聚焦时强调环优先）；失焦后 drop shadow 自动恢复。若视觉过重，调整 `--focus-ring` 内层 gap 至 3px。 |
| **reduce-motion 守卫的 `0.01ms !important` 可能影响 radix Dialog/Sheet 的退出动画事件**（理论上 `animationend` 仍触发） | 低×高 | `0.01ms`（非 0ms）保证事件触发；守卫显式列出 `.animate-pulse/spin/bounce/ping` 与 hover transform 类，精准命中；radix 自带 `data-[state]` transition 在 0.01ms 下瞬时完成不卡死。CI 加 reduce-motion E2E 用例验证 Dialog 开关 10 次无泄漏。 |
| **magic-px codemod 误伤**（如 `text-[1.65rem]` 在 markdown CSS、`rounded-[6px]` 是刻意小圆角） | 中×低 | 批次 4/6 用白名单（仅替换列举的 magic 值），不用宽泛正则；每文件单独 commit，review 逐处确认；保留豁免注释机制。 |
| **`--shell-line` dark 透明度 0.08 在软渐变后几乎不可见** | 中×中 | T-E-2 方案已预案：dark 下提升 `--shell-line` 至 0.12 或新增 `--shell-line-strong` 仅供 hairline；视觉验证后决策。 |
| **三组件薄包装保留导致长期不收敛**（Empty/EmptyPanel/StageEmptyState 永远存在） | 中×低 | 薄包装加 `@deprecated` JSDoc；T-E-7 批次结束后开一个跟踪 issue，季度内清理存量调用点，最终删除薄包装。 |

## 5. 验证与回归 (Verification)

- **类型**：`bun run check:frontend`（T-E-1 新增 CSS 变量无 TS 影响；T-E-3/5/6 新组件需导出类型正确）。
- **Lint**：`bun run lint`（关注 tailwindcss 类名拼写、未使用 import）。
- **组件快照**：`bun run test:components`（slab-components 的 Skeleton/Empty/Button 新增/变更需更新快照）；`bun run test:browser`（playwright 视觉回归，覆盖 dark/light × sidebar/topbar/footer/header/StateSurface/Monaco）。
- **grep 守卫**（加入 CI 脚本，防止回归）：
  ```
  # magic-px 守卫（阈值随批次推进下调）
  rg -c 'text-\[1[0-3]px\]' packages/ | awk -F: '{s+=$2} END{exit (s>0)?1:0}'
  rg -c 'rounded-\[(24|28|30|32|34)px\]' packages/ | awk -F: '{s+=$2} END{exit (s>0)?1:0}'
  rg -c 'shadow-\[0_0_0_0_var\(' packages/  # 内联 ring 守卫
  rg -c '#00685f|#191C1E' packages/         # raw hex 守卫（除豁免）
  rg 'prefers-reduced-motion' packages/slab-components/src/styles/globals.css  # 必须命中
  ```
- **人工走查清单（dark/light 双主题，每项打勾）**：
  - [ ] sidebar 与内容区交界：软渐变，无 1px 硬线。
  - [ ] topbar 底部 / footer 顶部：软渐变 fade。
  - [ ] header 三处竖线 span：上下 fade。
  - [ ] StateSurface（empty/loading/error × compact/default/stage）：玻璃面 + halo 一致。
  - [ ] Skeleton（含 Text/Circle）：方向性 shimmer，reduce-motion 下停止。
  - [ ] Button 全 variant + sidebar Link + 卡片 + 历史 row：键盘 Tab 焦点环统一。
  - [ ] Monaco：背景/关键字/选中/gutter 与外围卡片无色差；外圆内 6px 圆角。
  - [ ] hub/image/video/audio/task/plugins/workspace/setup 9 页面：无 magic-px 残留视觉、无 raw hex 破相。
  - [ ] OS 启用 reduce-motion：全域非必要动画停止，Dialog 开关正常。
- **回归矩阵**：每次 M2/M3/M4 PR 合并后跑一次完整 `check:frontend && lint && test:components && test:browser`，并执行上述人工走查清单的受影响子集。
