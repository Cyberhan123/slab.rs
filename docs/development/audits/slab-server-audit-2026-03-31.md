# slab-server 代码审计与治理

## P1-1 任务子系统“万能字符串/JSON”泥潭（契约不清晰）

位置：slab-server/src/domain/services/task.rs、slab-server/src/infra/db/entities/task.rs

问题描述：任务状态用字符串字面量判断，result_data/input_data 用 `Option<String>` 承载 JSON，再在运行时兜底解析；失败时回退为文本。这种契约会造成隐式行为分叉，难以静态校验，容易产生“看似成功、实际语义错位”的数据腐烂。

影响程度：高

## P1-2 配置与数据路径传递链路分裂（端口/URL 多源硬编码）
位置：

slab-app/src-tauri/src/setup/sidecar.rs（sidecar 固定 127.0.0.1:3000）

slab-app/src-tauri/src/lib.rs（SLAB_API_URL）

slab-app/src/lib/config.ts（VITE_API_BASE_URL）

slab-app/src/lib/tauri-api.ts（VITE_API_URL）

问题描述：同一“API 基址”存在多个来源与变量名（SLAB_API_URL / VITE_API_BASE_URL / VITE_API_URL），且 sidecar 启动端口硬编码。这是典型配置漂移问题：环境切换、端口调整、测试环境并行实例都容易错连。

影响程度：高

## P2-1 插件 API 通道与 CSP 对本地 API 端口硬编码，绕过统一路由
位置：slab-app/src-tauri/src/plugins/runtime.rs、slab-app/src-tauri/src/plugins/protocol.rs

问题描述：插件运行时固定 DEFAULT_API_BASE_URL = http://127.0.0.1:3000，插件 CSP connect-src 也固定包含 localhost:3000。若 sidecar 端口/协议变更，插件通道会失配；并且插件 API 请求路径未挂到统一“宿主 API endpoint 分发器”，形成并行入口，运维与审计成本高。
另外 blocked 网络模式下依然允许本地 API 访问是否符合产品策略，需与业务方确认。

影响程度：中高

## P3-1 云模型 legacy ID 兼容分支可能已成“历史死逻辑”
位置：slab-server/src/domain/services/chat/cloud.rs（parse_legacy_cloud_option_id 分支）

问题描述：仍在主路径内处理 cloud/{provider}/{model} 历史 ID，并执行列表扫描匹配。客户端已升级，该逻辑属于高噪音兼容残留；可以直接删除，并拆除相关链路，避免陷入泥潭

影响程度：中