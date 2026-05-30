
高优先级（建议先处理）：云模型 legacy ID shim、设置文件 v1 迁移链、模型表 provider 回退链。
中优先级：模型包持久化状态迁移链、插件 manifest 旧字段兼容链、任务 payload 旧格式回退链。
低优先级：runtime CLI 的旧别名兼容、已退役参数拦截（这是防回退保护，不是兼容实现本身）。
1) 设置文件 legacy v1 迁移链（仍在启动主路径）

入口（server 启动）：main.rs:344
入口（desktop 启动）：state.rs:35
迁移实现：settings.rs:9
真正转换逻辑：provider.rs:130
旧格式识别规则：provider.rs:370
映射表（legacy path -> current path）：provider.rs:326
处理建议：

移除 is_legacy_settings_file 分支和映射表。
2) 云模型 legacy ID shim（cloud/provider/model）

主路径调用：cloud.rs:298
shim 命中点：cloud.rs:363
旧 ID 解析：cloud.rs:392
兼容匹配逻辑：cloud.rs:403
处理建议：

保留 warning 的同时补 metric 维度（provider、legacy_model_id）。
删除 parse_legacy_cloud_option_id 和 model_matches_legacy_cloud_option 分支。

3) 模型表 provider 旧字段回退链（读写都仍在用）

写入时仍生成 legacy provider：catalog.rs:588
legacy provider 组装：catalog.rs:602
读库时 kind 失败回退到 provider：model.rs:107
回退函数：model.rs:38
处理建议：
移除 derive_kind_from_legacy_provider 和 provider 写回逻辑。
4) 模型包持久化状态迁移链
每次读/构建都会先迁移旧状态：model_packs.rs:118
迁移入口：model_packs.rs:172
状态版本迁移实现：model_packs.rs:468
旧选择导入逻辑（legacy_selection_to_import）：pack.rs:212
处理建议：

删 persisted.pack_selection 回退分支。
5) 插件 legacy 兼容链

旧 manifest 入口兼容（缺 runtime 时接受 legacy ui）：plugin.rs:33
错误文案明确 legacy ui：plugin.rs:45
旧 source_kind market_pack 仍被视为可管理包：plugin.rs:33
判断分支：plugin.rs:1248
处理建议：
移除 raw.ui 回退入口。
清理 market_pack 常量，仅保留 import_pack/package_url。
6) 任务 payload 旧格式回退链

新 envelope 编码：task.rs:222
旧格式回退（无法解 envelope 时直接原样返回）：task.rs:236
处理建议：

为严格 envelope 并做一次离线修复。
7) runtime load spec 的 legacy 转换层

模型包桥接仍走 from_legacy：runtime_bridge.rs:98
类型层转换 API：runtime.rs:288
具体映射实现：load_config.rs:152
处理建议：

让 runtime_bridge 直接产出 RuntimeBackendLoadSpec。
再收口 slab-types 中 from_legacy/to_legacy_spec API。
8) 已退役链路的防回退保护（不是兼容逻辑）
启动参数 legacy override 拒绝：main.rs:144
启动时执行校验：main.rs:196
处理建议：
这部分应保留，属于“防旧参数重新进入系统”的护栏。
建议的处理顺序（可直接按这个开工）
云模型 legacy ID shim
模型表 provider 回退链
设置文件 legacy v1 迁移。
模型包/任务 payload 的读时兼容迁移。
插件 manifest/source_kind 历史兼容清理。
runtime load spec 的 legacy API 收口。
如果你要，我下一步可以按这个清单给你出一版“可执行工单拆分”（每条含改动点、风险、测试清单、回滚策略），直接对应到 PR 粒度。