<div align="center">
  中文 / <a href="./README.md">English</a>
</div>
<br>

# Slab

Slab 是一个本地优先的 AI 桌面工作台，帮助你把聊天、语音转写、图像生成、视频相关处理和模型管理整合到一个统一的应用里。它更关注能不能顺手完成工作，而不是把用户暴露给复杂的技术细节。

## 目录

- [简介](#简介)
- [为什么选择 Slab](#为什么选择-slab)
- [核心能力](#核心能力)
- [项目结构](#项目结构)
- [开发指南](#开发指南)
  - [安装](#安装)
  - [开发](#开发)
  - [构建](#构建)
- [Slab 文档](#slab-文档)
- [贡献者](#贡献者)
- [许可证](#许可证)

## 简介

Slab 适合希望在本地电脑上完成 AI 任务的个人开发者、研究者、创作者和团队。你可以把它理解成一个统一入口：在同一个桌面应用里完成模型下载与管理、发起聊天、处理音频、生成图片和跟踪任务进度。

## 为什么选择 Slab

- 一个应用覆盖多种 AI 任务，不需要在聊天、转写、图像生成和模型管理之间来回切换。
- 更适合重视隐私、离线能力和本地控制的人群，很多工作可以直接在设备上完成。
- 对日常使用更友好，长任务有任务队列，模型有集中管理，未来也会继续扩展可扩展性能力。
- 既能作为桌面应用使用，也能作为统一接口衔接你的其他工具和流程。

## 核心能力

### 当前可用

- **智能聊天**  
  在统一的聊天界面中与本地模型对话，适合写作辅助、问答、总结整理和日常思考。
- **音频转写**  
  把语音或音频快速转成文本，适合会议记录、采访整理、课程笔记和素材归档。
- **图像生成**  
  在本地生成图像内容，适合概念草图、视觉探索、营销素材尝试和创作实验。
- **视频相关处理**  
  把视频处理任务纳入同一工作台，便于结合字幕、音频和其他媒体流程统一管理。
- **模型中心**  
  下载、浏览、切换和管理模型资源时更集中，不需要手动维护一堆零散入口。
- **任务队列**  
  长时间运行的任务可以后台排队和跟踪，不会打断你继续做别的事情。
- **硬件兼容更省心**  
  Windows 目前是支持最完整的一条路径：在完整安装包中，Slab 基于 `ggml` 和我们自己的运行时封装，会在初始化阶段自动选择更合适的本地变体，NVIDIA 显卡优先使用 CUDA，AMD 显卡优先使用 HIP；如果专用 GPU 路径不可用，则会回到自带的基础运行时，而这个基础运行时已经包含 Vulkan 和 CPU 后端。对于 macOS 用户，Slab 也会借助 `ggml` 的本地加速路径，在 Apple Silicon 上尽量利用系统原生的加速能力完成本地推理。Linux 也很可能是可支持的，仓库里已经准备了 Linux 目标产物，但由于维护者目前没有 Linux 机器，兼容适配和验证还不算完整。如果你对推进 Linux 支持有兴趣，非常欢迎一起参与。
- **统一设置体验**  
  在同一处管理运行环境、模型偏好和应用设置，降低日常维护成本。

### 即将到来

- **即将到来的插件支持**  
  插件扩展是 Slab 的产品方向之一，但目前还不是已经交付的用户能力。当前仓库已经开始准备这一部分的基础设施，更完整的插件支持会在后续版本到来。

## 项目结构

下面的目录结构是从实际仓库中提炼出来的高层视图，重点帮助你快速理解每一部分在整个产品中的角色，而不是陷入实现细节。

```text
.
|-- bin/
|   |-- slab-app/                      桌面宿主应用与 Tauri 打包入口
|   |-- slab-server/                   产品 API 的本地服务入口
|   |-- slab-runtime/                  AI 任务执行运行时
|   `-- slab-windows-full-installer/   Windows 全量安装器
|-- crates/
|   |-- slab-app-core/                 共享应用逻辑
|   |-- slab-agent/                    Agent 与编排能力
|   |-- slab-hub/                      模型中心抽象层
|   |-- slab-proto/                    共享协议定义
|   |-- slab-runtime-core/             运行时调度与任务核心
|   |-- slab-types/                    共享数据契约与设置类型
|   `-- ...                            引擎绑定与配套基础库
|-- packages/
|   |-- slab-desktop/                  桌面前端应用
|   |-- slab-components/               共享 UI 组件库
|   `-- slab-i18n/                     共享国际化包
|-- docs/                              文档站点与使用指南
|-- models/                            模型打包脚本与资源
|-- plugins/                           面向后续可扩展能力的插件工作区
|-- testdata/                          样例媒体与测试数据
`-- vendor/                            随仓库分发的第三方运行资源
```

- `packages/slab-desktop` 是用户每天直接看到和操作的桌面界面。
- `bin/slab-app`、`bin/slab-server`、`bin/slab-runtime` 共同支撑本地应用、任务执行和服务入口。
- `crates/` 目录是主要的共享能力层，承载模型、任务、协议和通用逻辑。
- `plugins/` 用于承接后续插件和可扩展能力相关的建设。
- `docs/`、`models/`、`testdata/`、`vendor/` 分别承担文档、模型打包资源、样例数据和依赖资源等辅助角色。

## 开发指南

下面只保留最常用、最直接的开发入口。更完整的工程说明可以查看项目文档。

### 安装

- 安装 Rust 稳定版工具链。
- 安装 `cargo-make`：`cargo install cargo-make`
- 安装 Bun。
- 如果要运行服务兼容性测试，额外安装 Python。

```sh
# 在仓库根目录执行
bun install
```

### 开发

从仓库根目录启动最常用的开发流程。

```sh
# 启动主开发流程
cargo make dev

# 检查桌面前端构建
cd packages/slab-desktop
bun run build
```

### 构建

下面这些命令适合做常规构建、检查和测试。

```sh
# Rust 工作区
cargo build --workspace
cargo test --workspace
cargo check --workspace

# 重点模块检查
cargo check -p slab-server
cargo check -p slab-runtime
cargo check -p slab-windows-full-installer

# 桌面前端
cd packages/slab-desktop
bun run build

# Windows 全量安装器
cd ../..
cargo make build-windows-full-installer

# 服务兼容性测试
python -m pip install -r bin/slab-server/tests/requirements.txt
pytest bin/slab-server/tests
```

## Slab 文档

- 快速开始：https://slab.reorgix.com/guide/getting-started
- 文档首页：https://slab.reorgix.com/

## 贡献者

欢迎提交 Issue、文档修订、功能建议和 Pull Request，一起把 Slab 打造成更实用的本地 AI 工作台。

- 贡献者列表：https://github.com/Cyberhan123/slab.rs/graphs/contributors

## 许可证

本项目采用 [GNU Affero General Public License v3.0](./LICENSE) 授权。`testdata/` 中的第三方材料保留其原始许可证。
