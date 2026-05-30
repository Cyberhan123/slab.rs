改造总原则

不动 vendor 目录中的第三方代码。
先在 slab-utils 提供统一 API，再迁移调用方。
优先替换重复逻辑，最后再清理旧依赖。
所有 checksum 比较继续保持大小写不敏感、支持 sha256: 前缀。
A. 依赖层清单

在 workspace 依赖中新增 ring
文件：Cargo.toml:110（同级依赖区）
动作：添加 ring（workspace 统一版本）

替换 crate 级 sha2 依赖为 ring（若迁移后不再需要 sha2）
文件：
Cargo.toml:19
Cargo.toml:19
Cargo.toml:20
Cargo.toml:85
Cargo.toml:21

检查并删除无用 sha2 workspace 声明
文件：Cargo.toml:152

B. slab-utils 统一能力建设（核心）

新增 hash 能力（基于 ring）
建议位置：在 slab-utils 下新增 hash 模块并在 lib.rs:1 导出
建议 API：
sha256_hex_bytes
sha256_hex_reader
sha256_hex_file
verify_sha256_hex_expected（支持 sha256: 前缀）
迁移现有 fsops 哈希实现到 ring
文件：fsops.rs:83
动作：hash_reader 和 sha256_file 改为调用新 hash 模块，保持签名不变（降低调用方改动）

新增临时文件原子写能力（基于 tempfile）
建议 API（放 slab-utils 的 fs/io 模块，或 cab/fsops 扩展）：

atomic_write_bytes
atomic_write_json_pretty（可选）
关键行为：
在目标目录创建 NamedTempFile
写入、flush、sync_all
persist 到目标路径
出错自动清理
保留 Unix 权限控制钩子（当前 provider 有 600/700 语义）
C. 业务代码迁移清单（SHA256）

libfetch 校验逻辑切 ring
文件：verify.rs:1
动作：去掉 sha2::{Digest, Sha256}，改用 slab-utils hash 校验函数
测试：保留并通过现有用例（含前缀、空白、大小写不敏感）

plugin registry 文件哈希切 ring/统一 helper
文件：registry.rs:477
动作：compute_file_sha256 改为调用 slab-utils 哈希函数
兼容：保持返回 String hex，小写存储逻辑不变

app-core plugin 侧 hash_bytes_hex 和 hash_file_hex 收敛
文件：plugin.rs:1259
动作：删本地 hash_bytes_hex/hash_file_hex，改为 slab-utils 调用

app-core model_packs manifest 哈希收敛
文件：model_packs.rs:836
动作：manifest_sha256_from_pack_bytes 与本地 hash_bytes_hex 改为 slab-utils 调用

app-core workspace 内容哈希收敛
文件：workspace.rs:685
动作：content_hash 改为 slab-utils 哈希函数

windows-full-installer copy_file_with_hash 切 ring
文件：bundle.rs:311
动作：sha2::Sha256::new 与 Digest::update/finalize 改为 ring 上下文或直接调用 slab-utils::hash::sha256_hex_reader

slab-utils cab payload 中 digest 迁移 ring
文件：payload.rs:317
动作：Sha256::digest 改为 slab-utils hash 函数，避免模块内混用两套实现

D. 业务代码迁移清单（临时文件与原子写）

config provider 原子写切 tempfile
文件：provider.rs:219
动作：
temp_path + Uuid 方案改为 NamedTempFile::new_in(parent)
保留 replace_file 与 sync_parent_dir 逻辑（尤其 Windows MoveFileEx 语义）
优先改为调用 slab-utils 原子写 API，provider 仅保留错误包装
app-core model_packs 原子写切 tempfile
文件：model_packs.rs:320
动作：
temp_path + create_new + rename 模式改为 NamedTempFile + persist
path.exists + remove_file 分支可由 persist 的覆盖策略统一处理
尽量复用 slab-utils 原子写 API
统一清理零散 temp_path 拼接模式
重点搜索模式：
.tmp- + Uuid::new_v4
OpenOptions::new().create_new(true).write(true)
目前命中主点已是上述两处，可全部清零为统一实现。
E. 测试与回归清单

哈希一致性回归
libfetch 现有 verify 测试通过
文件：verify.rs:24
plugin integrity 校验相关测试通过
文件：plugin.rs:1517
model_packs manifest_sha256 相关测试通过
文件：model_packs.rs:1501
原子写行为回归
配置写入失败时不污染目标文件
文件：provider.rs:219
model pack 覆盖写稳定
文件：model_packs.rs:320
现有 tempfile 测试无需改动
文件：smoke.rs:10
F. 完工判定清单

全仓业务代码不再直接 use sha2。
SHA256 入口仅保留 slab-utils 统一实现（底层 ring）。
temp_path + Uuid::new_v4 + OpenOptions create_new 原子写模式从业务 crate 消失。
所有相关单测通过。
Cargo 依赖中 sha2 若无剩余用途则移除。