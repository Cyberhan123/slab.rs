# 专项执行计划 · 安全与正确性止损 (Security & Correctness Stop-Loss)

| 字段 | 值 |
|---|---|
| Plan ID | A |
| 关联根因 | R3（安全与正确性 P0） |
| 上游审计 | [slab-deskotp-audits-2026-6-19.md](../../audits/slab-deskotp-audits-2026-6-19.md)（§1.4 P0 #1/#2/#3/#7、§3.1.3、Plugins/Workspace 域） |
| 负责域 | Plugins · Workspace · Task · Tauri Host |
| 状态 | Draft / Pending Review |
| 预估总工作量 | M+L（≈ 4.5 人日：P0 ×4 ≈ 3 人日，P1 ≈ 1 人日，P2 ≈ 0.5 人日） |

---

## 1. 目标与边界 (Scope)

- **北极星**：在 1 个迭代周期内消除 Slab Desktop 当前所有可被利用的越权 / 数据丢失 / 必报 400 类 P0 正确性缺陷，使 Plugins / Workspace / Task 三条核心链路"在最坏输入下也不丢数据、不越权、不产生确定性失败"。
- **In scope**：
  - 插件 `/v1/plugins/{id}/api-request` 的两条调用路径（React 裸 HTTP + Tauri 子 WebView）的 caller-id 派生与授权闭环。
  - `authorize_*_from_caller(None)` 系列的"静默放行 → 硬拒"语义翻转。
  - Workspace 未保存变更守卫的单一可信源（Monaco working-copy service）与 AntD Modal 化。
  - Task Restart 按钮按 `task_type==='model_download'` 门控。
  - 插件权限评审 UX（导入期 manifest 解析 + 运行时首拒弹授权框）的脚手架。
  - 停止插件时不再用 `lastError:null` 覆盖诊断态。
- **Out of scope（移交他计划）**：
  - 插件 uninstall / install-from-URL / `/rpc` `/events` 消费闭环 → **移交 Plan C（Plugins 能力释放）**，依赖 T-A-1/T-A-2 完成后接入。
  - Workspace HTTP `recent`/`config` 恒空（状态源割裂）→ **移交 Plan B（状态对齐）**，与本计划的"守卫"正交。
  - Assistant 流式 `AbortController` / `turn_failed` 内容保留 → **移交 Plan B**（属 R2，非 R3）。
  - 全域 `prefers-reduced-motion` → **移交 Plan D（设计系统）**。
- **Definition of Done**：
  - [ ] 所有 4 个 P0（T-A-1/2/3/4）合并并通过 `bun run check:rust` + `bun run check:frontend` + `bun run test:frontend`。
  - [ ] T-A-1/T-A-2 新增/更新 Rust 单测覆盖"无 caller 必拒"、"跨插件调用必拒"、"宿主裸 HTTP 必拒"三个分支。
  - [ ] T-A-3 新增组件测试覆盖"dirty 时关闭标签页弹出 AntD Modal"与"取消保留内容"两条路径。
  - [ ] 安全侧试：以宿主 React 上下文或任意 HTTP 客户端调用 `/v1/plugins/{victim}/api-request` 被拒（403/401），不再以任意插件身份执行 slabApi。
  - [ ] 非下载类 Task 详情不再出现可点击的 Restart 按钮；点击即 400 的回归用例入库。
  - [ ] 插件停止后 `lastError` 仍能反映真实失败态（不再被 null 覆盖）。
  - [ ] 计划内所有 schema 变更执行 `bun run gen:api` 并提交生成的契约文件。

---

## 2. 任务卡 (Task Cards)

### T-A-1 · 下线/加固插件 `/v1/plugins/{id}/api-request` HTTP 路由的 caller-id 派生

- **严重度** P0 · **类型** bugfix · **预估** M
- **证据**：
  - 前端宿主走裸 HTTP：[plugin-webview-page.tsx:165](packages/slab-desktop/src/pages/plugins/components/plugin-webview-page.tsx#L165)（`apiRequestMutation.mutateAsync({ params: { path: { id: plugin.id } }, body: message.request })`）
  - 后端 HTTP 路由直接信任 path 参数：[handler.rs:208-214](bin/slab-server/src/api/v1/plugins/handler.rs#L208)（`service.plugin_api_request(&params.id, request)`）
  - `slab-app-core` 用 path 里的 `plugin_id` 查 manifest 取权限：[assets.rs:61-76](crates/slab-app-core/src/domain/services/plugin/assets.rs#L61)（`registry.get_plugin(plugin_id)` → `authorize_slab_api_request(&plugin.manifest.permissions.slab_api, ...)`）
- **问题**：`POST /v1/plugins/{id}/api-request` 的 HTTP 实现把 `{id}` 当作"调用者身份"使用，但 `{id}` 是客户端可控的 path 参数。React 宿主（浏览器模式 iframe 桥）、任意 HTTP 客户端、甚至被 XSS 的页面，都可以把 `id` 填成任意受害插件，从而以受害插件声明的 `slabApi` 权限（如 `chat:complete`、`models:load`、`tasks:cancel`）调用 slabApi —— 跨插件越权。Tauri 子 WebView 路径（[mod.rs:123-138](bin/slab-app/src-tauri/src/plugins/mod.rs#L123)）通过 `caller_plugin_id(&webview)` 从 webview label 派生是安全的；问题仅在 HTTP 路由这一侧。
- **方案**：
  1. **区分"目标插件"与"调用者"两个语义**。新增请求字段或 header `X-Slab-Plugin-Caller`（类型 `string`，可选）。HTTP schema（`slab-types` 中 `PluginApiRequest` 或其包装）增加 `caller_plugin_id: Option<String>`；执行 `bun run gen:api` 同步前端契约。
  2. **`slab-app-core` 的 `plugin_api_request` 签名改为 `(caller_plugin_id: Option<&str>, target_plugin_id: &str, request)`**，并在内部强制：`caller_plugin_id` 必须等于 `target_plugin_id`，且必须在 registry 中存在、处于 enabled/running。任何不一致 → `AppCoreError::Forbidden`（403）。
  3. **`bin/slab-server` handler**：从 header `X-Slab-Plugin-Caller` 读取 caller；若缺失或与 path `id` 不一致 → 403。保留 OpenAPI 注解（更新 schema）。
  4. **前端宿主桥（`plugin-webview-page.tsx:165`）**：浏览器 iframe 桥 mutate 时显式注入 `headers: { 'X-Slab-Plugin-Caller': plugin.id }`，与 path `id` 一致；这是自我声明，安全性来自第 2 步的"caller==target"校验（即宿主只能代表它正在渲染的那个插件发请求，且该插件必须真实存在并已授权）。
  5. **Tauri 子 WebView 路径（`mod.rs`）**：保持 `caller_plugin_id(&webview)` 派生，调用同一个 core 接口，caller==target 同样校验（子 WebView 的 label 本就编码了 plugin id，等价语义）。
  6. **回归测试**：扩 `bin/slab-server/src/api/v1/plugins/handler.rs` 测试与 `crates/slab-app-core/src/domain/services/plugin/tests.rs`：新增"caller 缺失必拒"、"caller≠target 必拒"、"caller==target 且权限齐备放行"三个用例。
- **验收标准 (AC)**：
  - [ ] HTTP `POST /v1/plugins/victim/api-request` 在无 `X-Slab-Plugin-Caller` 或 caller≠victim 时返回 403，不触达 slabApi。
  - [ ] React 宿主桥显式携带 caller header，正常插件 UI 的 slabApi 调用功能回归通过。
  - [ ] Tauri 子 WebView 路径行为不变（继续从 webview label 派生）。
  - [ ] `bun run check:rust` + `bun run check:frontend` 通过；schema 变更后 `bun run gen:api` 已执行。
  - [ ] 新增 3 条 Rust 单测（缺失/不一致/合法）全部绿。
- **依赖**：无（可与 T-A-2 并行；T-A-5 在其后做权限评审 UX，但本卡不阻塞 T-A-5）。

---

### T-A-2 · `authorize_*_from_caller(None)` 改为硬拒

- **严重度** P0 · **类型** bugfix · **预估** S
- **证据**：
  - [mod.rs:213-221 `authorize_plugin_call_request`](bin/slab-app/src-tauri/src/plugins/mod.rs#L213)：`if let Some(...) { ensure_same_plugin_call(...) } Ok(())` —— `None` 直落 `Ok(())`。
  - [mod.rs:223-234 `authorize_plugin_api_request_from_caller`](bin/slab-app/src-tauri/src/plugins/mod.rs#L223)：`let Some(caller) = ... else { return Ok(()) }` —— 无 caller 静默放行 slabApi。
  - 既有测试反而锁死了错误行为：[mod.rs:397 `authorize_plugin_call_request(None, &request).is_ok()`](bin/slab-app/src-tauri/src/plugins/mod.rs#L397) 与 [mod.rs:453 `authorize_plugin_api_request_from_caller(None, ...).is_ok()`](bin/slab-app/src-tauri/src/plugins/mod.rs#L453)。
- **问题**：`caller_plugin_id` 派生自 webview label（[mod.rs:209 `caller_plugin_id`](bin/slab-app/src-tauri/src/plugins/mod.rs#L209) → `view::plugin_id_from_webview_label`）。当 webview label 不符合插件编码约定（例如宿主主窗口、或将来出现的非插件 webview）调用 `plugin_call` / `plugin_api_request` 时，caller 解析为 `None`，两个授权函数都静默 `Ok(())` —— 等同于"无标签调用者拥有全部插件权限 + 可跨插件调用"，是特权提升。
- **方案**：
  1. **`authorize_plugin_call_request`**：将 `if let Some(caller) = ... { ensure_same_plugin_call(...) }` 改为 `let caller = caller_plugin_id.ok_or_else(|| "plugin call requires a plugin webview caller".to_string())?; ensure_same_plugin_call(caller, &request.plugin_id)?;`。
  2. **`authorize_plugin_api_request_from_caller`**：将 `let Some(caller) = ... else { return Ok(()) }` 改为 `let caller = caller_plugin_id.ok_or_else(|| "plugin api request requires a plugin webview caller".to_string())?;`，其余 `registry.get_plugin(caller)` / `authorize_slab_api_request` 不变。
  3. **翻新单测**：[mod.rs:397](bin/slab-app/src-tauri/src/plugins/mod.rs#L397) 把 `None` 分支断言改为 `.is_err()`；[mod.rs:453](bin/slab-app/src-tauri/src/plugins/mod.rs#L453) 同理（`None` 必拒）。新增一条"主窗口 webview（label 无 plugin 前缀）调用必拒"的用例。
  4. **核对 `plugin_pick_file`**：[mod.rs:146](bin/slab-app/src-tauri/src/plugins/mod.rs#L146) 当前对 `None` 跳过视频读权限校验直接弹选框 —— 同源风险，本次顺带收紧为"caller 必须存在且持有 `files.read video` 权限"，否则 `Err`。
- **验收标准 (AC)**：
  - [ ] `authorize_plugin_call_request(None, ...)` 返回 `Err`，错误信息含 "requires a plugin webview caller"。
  - [ ] `authorize_plugin_api_request_from_caller(None, ...)` 返回 `Err`，同上语义。
  - [ ] 主窗口（非插件 webview）调用 `plugin_call` / `plugin_api_request` / `plugin_pick_file` 被拒。
  - [ ] 既有合法路径（子 WebView caller==target 且权限齐备）全绿。
  - [ ] `bun run check:rust` 通过。
- **依赖**：无。与 T-A-1 并行；二者合并后构成"无论 HTTP 还是 Tauri 通道，caller 缺失即拒"的完整闭环。

---

### T-A-3 · 统一 Workspace 未保存变更守卫到 Monaco working-copy service + 用 AntD Modal 替换 `window.confirm`

- **严重度** P0 · **类型** bugfix · **预估** M
- **证据**：
  - `selectedFileDirty` 仅派生自 React state：[use-workspace-page.ts:172](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L172)（`Boolean(selectedFile && editorContent !== selectedFile.content)`）
  - 4 处 `window.confirm` 守卫：[use-workspace-page.ts:280](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L280)、[:543](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L543)、[:687](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L687)、[:731](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L731)（还有 [:276 git panel](packages/slab-desktop/src/pages/workspace/components/workspace-git-panel.tsx#L276) 一处 git discard，本卡一并收敛）
  - Monaco working-copy service 已注册但未被消费为 dirty 源：[workspace-lsp.ts:34](packages/slab-desktop/src/pages/workspace/lib/workspace-lsp.ts#L34)、[:437](packages/slab-desktop/src/pages/workspace/lib/workspace-lsp.ts#L437)
- **问题**：真实编辑器是内嵌的 Monaco/VS Code（`openWorkspaceVscodeFile` 路径）。用户在编辑器里直接修改、关闭标签页、切换文件时，Monaco 的 working-copy dirty 状态并不回写到 React 的 `editorContent`/`selectedFile.content` 比较 —— 当 `editorContent` 与磁盘一致而 Monaco buffer 仍有未保存改动时，`selectedFileDirty=false`，4 处守卫全部静默放行，关闭即丢数据。叠加 `window.confirm` 是原生控件，与产品"高级通透感"设计系统冲突。
- **方案**：
  1. **单一可信源**：新增 `useWorkspaceEditorDirty()` hook（位于 `packages/slab-desktop/src/pages/workspace/hooks/`），从 Monaco working-copy service 取当前 active model 的 `isDirty`（通过 `getWorkingCopyService()` / `IEditorService` / 活动编辑器 `input.isDirty()`，或监听 `workingCopy.onDidChangeDirty`）。React state 退化为 fallback（仅浏览器无 Monaco 时）。
  2. **替换 `selectedFileDirty` 计算**：[use-workspace-page.ts:172](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L172) 改为 `const selectedFileDirty = useWorkspaceEditorDirty({ selectedFile, editorContent })` —— hook 内部优先 Monaco，否则回退旧比较。
  3. **AntD Modal 确认器**：新增 `useConfirmDiscardUnsaved()`（基于 AntD `App.useApp()` 的 `modal.confirm` 或 `Modal.useModal`，避免 `App` 未包裹时退化到 `Modal.confirm` 静态）。签名返回 `Promise<boolean>`。
  4. **替换 4+1 处 `window.confirm`**：[use-workspace-page.ts:280](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L280)、[:543](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L543)、[:687](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L687)、[:731](packages/slab-desktop/src/pages/workspace/hooks/use-workspace-page.ts#L731) 与 [workspace-git-panel.tsx:276](packages/slab-desktop/src/pages/workspace/components/workspace-git-panel.tsx#L276) 全部改为 `await confirmDiscard()`，对应回调改 `async`。
  5. **覆盖 Monaco 关闭路径**：当 Monaco 编辑器自身的 tab close（`workbench.action.closeActiveEditor` 或 `IEditorPart` 的 close）触发时，确保走同一守卫（注册 Monaco command handler 或 `IEditorGroupsService.onWillCloseEditor` 钩子），避免用户从编辑器侧关闭绕过 React 守卫。
  6. **组件测试**：在 `pages/workspace/hooks/__tests__/use-workspace-page-dirty.test.tsx` 覆盖：(a) Monaco dirty → 关闭文件标签弹 Modal → 取消 → 内容保留；(b) Monaco dirty → 确认丢弃 → 切换成功；(c) Monaco clean → 无弹窗直接切换。
- **验收标准 (AC)**：
  - [ ] 在 Monaco 编辑器中改动但未触发 React `editorContent` 同步时，关闭标签/切换文件仍弹出 AntD Modal。
  - [ ] 不再出现任何 `window.confirm` 调用（`grep "window.confirm"` 在 `packages/slab-desktop/src/pages/workspace` 下为 0 命中）。
  - [ ] Modal 样式走 AntD 主题（非原生浏览器框）。
  - [ ] 取消确认时 Monaco buffer 内容完整保留，无静默写盘或清空。
  - [ ] 浏览器无 Monaco 模式（fallback）dirty 检测仍工作。
  - [ ] `bun run check:frontend` + `bun run test:frontend` 通过；新增 3 条组件/ hook 测试绿。
- **依赖**：无（Monaco working-copy service 已注册）。若需 T-A-外 的"Workspace 状态源统一"（recent/config 恒空）作为同一 PR 收尾，移交 Plan B。

---

### T-A-4 · Task 详情 Restart 按钮门控 `task_type==='model_download'`

- **严重度** P0 · **类型** bugfix · **预估** S
- **证据**：
  - 前端无条件渲染 Restart：[task-detail-dialog.tsx:174-189](packages/slab-desktop/src/pages/task/components/task-detail-dialog.tsx#L174)（仅看 `status ∈ {failed, cancelled, succeeded}` 与 `!mediaTask`）
  - 后端硬拒：[handler.rs:213-232 `restart_rejects_non_model_download_tasks`](bin/slab-server/src/api/v1/tasks/handler.rs#L213)（`image_generation` 任务 restart 返回 400 "does not support restart"）
  - 实际 restart handler 的门控逻辑（位于 `bin/slab-server/src/api/v1/tasks/handler.rs` 的 `restart_task`，约束 `task_type==='model_download'`）。
- **问题**：任何非 `model_download` 任务（image/video/audio generation、transcription 等）在详情对话框都展示 Restart，用户点击 → 后端必报 400。属"确定性失败"交互硬伤（R1/R4，列为 P0 因其破坏核心链路观感且对失败任务无恢复路径）。
- **方案**：
  1. 在 [task-detail-dialog.tsx:174](packages/slab-desktop/src/pages/task/components/task-detail-dialog.tsx#L174) 的渲染条件增加 `selectedTask.task_type === 'model_download'`：
     ```tsx
     {!mediaTask &&
      selectedTask.task_type === 'model_download' &&
      (selectedTask.status === 'failed' || ...) && (<Button .../>)}
     ```
  2. （可选加固）若 `onRestart` / `restartTaskMutation` 在调用前可做客户端前置校验，加一道 `if (selectedTask.task_type !== 'model_download') return;` 防御性兜底。
  3. 引入 `task_type` 的常量枚举（若 `packages/api` 已生成 `task_type` 联合类型，直接用；否则在 `packages/api/src/permissions.ts` 同级加 `TASK_TYPES` 常量）。
  4. 组件测试：`task-detail-dialog.test.tsx` 新增两条——`model_download` 失败任务显示 Restart；`image_generation` 失败任务不显示 Restart。
- **验收标准 (AC)**：
  - [ ] `image_generation` / `video_generation` / `audio_transcription` 等非下载任务的详情对话框不再出现 Restart 按钮。
  - [ ] `model_download` 失败/取消/成功任务的 Restart 按钮行为不变。
  - [ ] `bun run check:frontend` + `bun run test:frontend` 通过。
  - [ ] 无新增 `restart_rejects_non_model_download_tasks` 之外的后端 400（即前端再不会发出这类无效请求）。
- **依赖**：无。

---

### T-A-5 · 插件权限评审 UX（导入解析 manifest + 运行时首拒弹授权框）

- **严重度** P1 · **类型** feat · **预估** L
- **证据**：
  - 权限常量与 surface：[permissions.ts](packages/api/src/permissions.ts)（`SLAB_API_PERMISSIONS` + `requiredSlabApiPermission`）
  - 后端权限授权与"非允许面"拒绝：[mod.rs:257-296 `authorize_slab_api_request` / `required_slab_api_permission`](bin/slab-app/src-tauri/src/plugins/mod.rs#L257)（含 `is not part of the allowed plugin API surface` 拒绝分支）
  - 导入流程当前无权限预览：[import-plugin-pack-dialog.tsx](packages/slab-desktop/src/pages/plugins/components/import-plugin-pack-dialog.tsx) + [use-plugins-page.ts:208-230 `importPluginPack`](packages/slab-desktop/src/pages/plugins/hooks/use-plugins-page.ts#L208)
  - 后端已有 manifest 校验管线可复用：[validation.rs:14 `validate_plugin_manifest`](crates/slab-app-core/src/domain/services/plugin/validation.rs#L14)
  - TS 侧无权限类型映射到人类可读描述；运行时无 `usePluginRpcCall`、无授权框脚手架（确认不存在）。
- **问题**：用户导入 `.plugin.slab` 时看不到将授予的权限（slabApi/files.read/network/lsp/agent）；运行时插件首次调用未授权 slabApi 时直接被后端拒、前端只 toast 一行英文错误，用户既无法理解也无法主动授权。叠加 T-A-1/T-A-2 收紧后，授权面更显重要。
- **方案**：
  1. **权限字典**：在 `packages/api/src/permissions.ts` 扩展 `SLAB_API_PERMISSION_LABELS: Record<SlabApiPermission, { title, description, severity }>`，补齐 8 项权限的人类可读文案 + 风险等级（如 `chat:complete` = 高，`models:read` = 低）。
  2. **导入期 manifest 预览**：
     - 新增 `GET /v1/plugins/inspect-pack`（或前端在导入前用 `unzip`-wasm 本地解析 `plugin.json`；优先做后端 dry-run 端点复用 [validation.rs:14](crates/slab-app-core/src/domain/services/plugin/validation.rs#L14)），返回 `{ manifest, permissions: { slabApi, files, network, lsp, agent }, integrityFiles, warnings }`。
     - 在 [import-plugin-pack-dialog.tsx](packages/slab-desktop/src/pages/plugins/components/import-plugin-pack-dialog.tsx) 选定文件后调用 inspect，展示 `<PermissionReviewList>`（分组列出权限 + 风险徽标 + 不在 `SLAB_API_PERMISSIONS` 白名单的"未知权限"告警）。
     - 用户必须勾选"I've reviewed the permissions"才允许 `canImport=true`。
  3. **运行时首拒授权框**：
     - 新增 `usePluginAuthorization()`（基于 Zustand），持久化"已授权的 (pluginId × permission) 集合"到 `ui-state`。
     - `plugin-webview-page.tsx` 桥在 mutate 前：根据 `message.request` 计算 `requiredSlabApiPermission`（复用 [permissions.ts:15](packages/api/src/permissions.ts#L15)）；若该 (pluginId × permission) 未授权 → 弹 AntD Modal 列出请求的方法/路径/所需权限/风险 → 用户 Allow/Deny。
     - Allow → 记入已授权集合并发请求；Deny → 不发请求，桥直接回 `{ ok:false, error:'permission denied by user' }`。
     - 新增 Settings 子页"Plugin Permissions"查看/撤销已授权项（撤销后下次调用再次弹框）。
  4. **与 T-A-1 协同**：前端注入 `X-Slab-Plugin-Caller` header 不变；本卡的客户端预授权只是减少首拒打扰，最终授权仍以 manifest 声明（后端 `authorize_slab_api_request`）为准 —— 不削弱安全模型。
  5. **schema 变更**：新增 inspect 端点的响应类型 → `bun run gen:api`。
- **验收标准 (AC)**：
  - [ ] 导入 `.plugin.slab` 后、真正上传前，用户看到 manifest 中声明的全部权限（slabApi/files/network/lsp/agent）与风险徽标。
  - [ ] 未知/不在白名单的 slabApi 权限高亮告警。
  - [ ] 运行时插件首次发起某权限的 slabApi 调用 → 弹 AntD Modal；Allow 后本次及后续放行，Deny 后桥回错误且不触达后端。
  - [ ] Settings 可查看与撤销已授权权限。
  - [ ] `bun run check:rust` + `bun run check:frontend` + `bun run gen:api` 通过。
- **依赖**：T-A-1（caller header 通道）。可与 T-A-6 并行。

---

### T-A-6 · 停止插件不再硬编码 `lastError:null` 覆盖诊断态

- **严重度** P2 · **类型** bugfix · **预估** S
- **证据**：
  - [use-plugins-page.ts:131-132 `stopPluginMutation.mutateAsync({ body: { lastError: null } })`](packages/slab-desktop/src/pages/plugins/hooks/use-plugins-page.ts#L131)
  - [use-plugins-page.ts:164-166 同样的 `body: { lastError: null }`](packages/slab-desktop/src/pages/plugins/hooks/use-plugins-page.ts#L164)（disable 时先 stop）
- **问题**：用户主动停止一个先前启动失败 / 运行中出错的插件时，前端用 `lastError:null` 覆盖后端诊断态。重启后或下次刷新，用户看不到上次失败原因，排障困难。这违背"诊断信息只由产生它的源头写入"原则。
- **方案**：
  1. 检查 `POST /v1/plugins/{id}/stop` 请求 schema（`slab-types` / OpenAPI）是否真要求 `lastError` 字段；若是必填，改为 `Option`/可空并默认 `None`；若已可选，前端直接省略。
  2. [use-plugins-page.ts:127-133](packages/slab-desktop/src/pages/plugins/hooks/use-plugins-page.ts#L127) 与 [:161-167](packages/slab-desktop/src/pages/plugins/hooks/use-plugins-page.ts#L161) 的 `mutateAsync` body 删除 `lastError: null`，改为 `{}` 或不传 body（按 schema 决定）。
  3. 后端 stop handler 应在停止成功时清错、停止失败时写入真实 error；不由前端声明。
  4. 若后端 stop handler 当前依赖客户端传 `lastError`，改为后端从 runtime/registry 取真实状态（与启动失败写入路径对齐）。
  5. 回归：插件启动失败 → stop → UI 显示先前错误；插件运行中出错 → stop → UI 显示运行错误（而非 null）。
- **验收标准 (AC)**：
  - [ ] 前端不再发送 `lastError:null`（`grep "lastError"` 在 `packages/slab-desktop/src/pages/plugins` 仅出现在类型/读取处）。
  - [ ] 启动失败的插件停止后，详情仍显示原失败原因。
  - [ ] 运行中出错的插件停止后，显示真实运行错误。
  - [ ] schema 变更（若有）已 `bun run gen:api`；`bun run check:rust` + `bun run check:frontend` 通过。
- **依赖**：无。可与 T-A-5 并行。

---

## 3. 执行顺序 (Sequencing)

- **M1（第 1 周 · 止损 P0）**：
  - 并行启动 **T-A-1**（HTTP caller 派生）与 **T-A-2**（None 硬拒）—— 二者共同构成插件越权闭环，建议同一工程师或紧密 review 闭环。
  - 并行启动 **T-A-3**（Workspace dirty 守卫）与 **T-A-4**（Restart 门控）—— 前端独立改动，互不干扰。
  - M1 结束验收 4 个 P0 的 DoD 全绿。
- **M2（第 2 周 · 收尾与体验）**：
  - **T-A-5**（权限评审 UX）—— 依赖 T-A-1 的 caller header，M2 启动。
  - **T-A-6**（lastError 不覆盖）—— P2 小修，M2 内完成。
- **可并行**：T-A-1 ∥ T-A-2 ∥ T-A-3 ∥ T-A-4 ∥ T-A-6（五卡两两无文件冲突；T-A-1/T-A-2 建议 review 联动）。
- **关键路径**：**T-A-1 → T-A-5**（T-A-5 的客户端预授权依赖 T-A-1 确立的 caller 通道）。其余卡无强阻塞。
- **建议合并策略**：T-A-1 + T-A-2 同一 PR（安全闭环不可拆）；T-A-3 单独 PR（改动面大、需组件测试）；T-A-4 单独小 PR；T-A-5 拆 2 个 PR（导入预览 / 运行时授权框）；T-A-6 单独小 PR。

---

## 4. 风险与缓解

| 风险 | 概率×影响 | 缓解 |
|---|---|---|
| T-A-1 收紧后，既有合法插件 WebView 的 slabApi 调用被误拒（caller 派生与 target 不一致） | 中×高 | M1 必须跑全量插件烟雾测试（视频字幕翻译器等官方插件）；保留 OpenAPI 与单测覆盖"caller==target 放行"。 |
| T-A-2 翻转语义后，宿主主窗口内未来出现的"合法插件调用入口"（如命令面板调插件）被拒 | 低×中 | 这是预期行为：宿主侧入口应走 `plugin_call` 显式带 plugin_id，而非借用 caller 通道；在 PR 描述记录此约束。 |
| T-A-3 Monaco working-copy service API 版本漂移（`@codingame/monaco-vscode-*`）导致 dirty 取值不稳 | 中×中 | hook 内部封装 + fallback 到 React state 比较；E2E 覆盖 dirty→关闭→保留路径。 |
| T-A-3 `await modal.confirm` 改变了原同步 `window.confirm` 的控制流，引入新竞态（用户未决前已切走） | 中×中 | confirm 期间禁用文件树点击（`isConfirming` state 置灰）；组件测试覆盖。 |
| T-A-5 inspect 端点解开恶意 pack 暴露后端 unzip 漏洞 | 低×高 | inspect 复用既有 [validation.rs:14](crates/slab-app-core/src/domain/services/plugin/validation.rs#L14) 校验管线（已防 path traversal / integrity）；限制解压大小（与 1GB 上限对齐）；不执行任何 entry。 |
| T-A-1 schema 变更（新增 header / `caller_plugin_id` 字段）破坏旧客户端 | 低×中 | header/字段均设为可选 + 默认拒，强制升级前端即可；桌面端 Tauri 自更新。 |
| 全部 P0 同周合并 → review 负载过载 | 中×低 | 按"关键路径"分 3 个 PR（见 §3 合并策略），分批 review/合并。 |

---

## 5. 验证与回归 (Verification)

- **类型/契约**：前端 `bun run check:frontend`；任何 `slab-types` / OpenAPI schema 变更（T-A-1 新增 header、T-A-5 inspect 响应、T-A-6 stop body）须 `bun run gen:api` 重新生成并提交。
- **Rust（Tauri/server）**：`bun run check:rust` —— 覆盖 T-A-1/T-A-2/T-A-5(inspect)/T-A-6 的后端改动；新增单测必须覆盖"无 caller 必拒 / 跨插件必拒 / 宿主裸 HTTP 必拒"三态。
- **单测/组件**：`bun run test:frontend` —— T-A-3 的 dirty Modal 组件/hook 测试、T-A-4 的 Restart 门控测试、T-A-5 的权限评审列表测试。
- **E2E**：`bun run test:browser` —— 至少一条端到端：(a) 插件越权调用被拒；(b) Workspace dirty→关闭弹 Modal→保留。
- **Lint**：`bun run lint`（含 `grep "window.confirm"` 在 workspace 下为 0 命中的人工核对）。
- **安全手测 checklist**（合并前 owner 自测）：
  1. 用 curl / 浏览器 devtools 以插件 A 身份调用 `/v1/plugins/B/api-request` → 期望 403。
  2. 主窗口 webview 通过 Tauri 调 `plugin_call` → 期望被拒。
  3. 在 Monaco 改动 → 直接点编辑器 tab close → 期望弹 AntD Modal。
  4. 非下载类失败任务详情 → 期望无 Restart 按钮。
