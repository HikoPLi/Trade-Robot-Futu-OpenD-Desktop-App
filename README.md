# Trade Robot (Futu OpenD Desktop)

工业化、跨平台（Windows/macOS/Linux）的 Futu OpenAPI（OpenD）桌面交易工作站。  
核心原则：`paper-first`、可审计、可复现构建、默认安全。

## 安全与合规声明

- 默认运行在 `paper`（模拟）模式。
- `live`（实盘）默认锁定，必须通过显式解锁流程。
- 不提供任何“保证收益”承诺。
- 不实现任何市场操纵、规则规避行为。
- 当系统不确定时，优先安全行为：拒单 / 撤单 / 停机。

## 关键能力（当前实现）

- 桌面应用：
  - Tauri v2（Rust Host）+ React/TypeScript UI
  - 页面：Dashboard / Market / Trading / Strategies / Models / Backtest / Audit / Settings / Diagnostics
- 交易与风控：
  - 多 Profile：`paper`、`live`、`research`
  - 预交易风控：下单频率、订单数量、持仓上限、价格带、交易时段、冷却窗口、市场前缀白名单、品种白名单
  - 失效保护：Kill Switch（UI + 全局热键）、崩溃后 Safe Mode
- 策略与模型：
  - 内置策略：MA Crossover、Mean Reversion、Short-Term Momentum Bot
  - 自定义策略脚本：Rhai 沙箱（实盘禁用）
  - 模型插件：内置指标模型 + ONNX（离线评估）
  - 模型 API 路由：OpenAI / DeepSeek / Qwen(千问) / Grok / Ollama（可配置主备路由）
- 回测与评估：
  - 事件驱动回测（含手续费/滑点）
  - 指标：总收益、夏普、最大回撤、命中率、换手率、交易数
  - 报告导出：JSON + HTML
- 可观测性与审计：
  - JSON 结构化日志
  - Trace ID 贯穿事件链路
  - SQLite 审计日志 + 哈希链防篡改
  - Diagnostics 页面实时诊断
- 国际化：
  - `English` / `简体中文` / `繁體中文`

## 技术栈与架构

- Desktop Shell：Tauri v2（`apps/desktop` + `apps/desktop/src-tauri`）
- UI：React + TypeScript + Vite
- Core Engine：Rust（`crates/core`）
- OpenD 连接器：Rust（`packages/connectors/futu`）
- 策略插件：Rust + Rhai（`packages/strategies`）
- 模型插件：Rust（`packages/models`，ONNX 推理由 `crates/core` 执行）
- 共享契约：
  - Rust：`crates/shared`
  - TypeScript：`packages/shared`

详细设计见 `docs/ARCHITECTURE.md` 与 `docs/adr/README.md`。

## 仓库布局

```text
apps/
  desktop/                 # React UI + Tauri host
crates/
  core/                    # 交易引擎（风控/执行/回测/审计/状态）
  shared/                  # Rust 共享类型与契约
packages/
  connectors/futu/         # OpenD 协议客户端 + MockOpenD 测试
  strategies/              # 内置策略 + Rhai 脚本策略
  models/                  # 内置模型目录
  shared/                  # TS 类型与校验
docs/
  ARCHITECTURE.md
  THREAT_MODEL.md
  RUNBOOK.md
  DEVELOPMENT.md
  API.md
  adr/
```

## 一键开发与构建

### 前置依赖

- Node.js 20+
- pnpm 9.9.0（项目已固定）
- Rust stable（见 `rust-toolchain.toml`）

Linux（Tauri/WebKit/Keyring）额外依赖参考 CI：

```bash
sudo apt-get update
sudo apt-get install -y \
  pkg-config \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libsecret-1-dev \
  patchelf
sudo apt-get install -y libwebkit2gtk-4.1-dev || sudo apt-get install -y libwebkit2gtk-4.0-dev
```

### Dev（一个命令）

```bash
pnpm install
pnpm dev
```

### Build（一个命令）

```bash
pnpm install
pnpm build
```

产物目录：`target/release/bundle/`

## 常用工程命令

```bash
pnpm typecheck
pnpm test
pnpm lint
pnpm fmt
pnpm audit
```

说明：
- `pnpm test` 会执行前端测试 + Rust workspace 测试。
- `pnpm lint` 会执行前端 lint + `cargo clippy -- -D warnings`。

## OpenD 连接与实盘解锁

### 默认连接

- Host：`127.0.0.1`
- Port：`11111`
- 配置入口：`Settings` 页面

### 实盘解锁门禁（必须全部满足）

1. 已切换到 `live` profile
2. 已阅读风险提示并输入确认短语  
   `I UNDERSTAND LIVE TRADING RISK`
3. 风险参数已改为非默认值
4. Kill Switch 全局热键已配置
5. OpenD 交易环境已设为 `real`（非 `simulate`）
6. OS Keychain 中存在 `futu.trade_password`
7. OpenD 连通性与 `get_global_state` 检查通过
8. OpenD 交易解锁调用成功

任一条件不满足会拒绝解锁并保持安全状态。

## Secrets、日志与本地数据

- Secrets：仅存 OS Keychain（不落盘明文）
  - 示例：`futu.trade_password`、`ai.openai_api_key`、`ai.deepseek_api_key`、`ai.qwen_api_key`、`ai.grok_api_key`、`ai.ollama_api_key`
- 非密钥配置：`config.json`（明文，仅存非敏感项）
- 审计与状态：`db/trade_robot.sqlite`
- 结构化日志：`logs/trade_robot.jsonl`
- 回测报告：`backtests/<id>/report.{json,html}`
- 模型评估报告：`model_evals/<id>/report.{json,html}`

数据根目录由以下代码按平台自动决定：

```rust
directories::ProjectDirs::from("com", "lihiko", "TradeRobot")
```

## 可靠性与运维特性

- Safe Mode：异常退出后下次启动默认 `halted`
- Kill Switch：
  - UI 一键触发
  - 全局热键触发
  - 立即停止策略、阻断新单、撤销未完成订单（实盘为 best-effort）
- 实盘执行：
  - `client_order_id` 去重与幂等防护
  - 订单/持仓/资金对账轮询
  - 连接退化与日亏阈值触发自动停机

## CI/CD 与发布

- 持续集成：`.github/workflows/ci.yml`
  - Typecheck / Test / Lint
- 跨平台打包：`.github/workflows/bundle.yml`
  - macOS / Ubuntu / Windows
  - 产物上传为 workflow artifacts
- Dependabot：`.github/dependabot.yml`

打包目标（按平台）：
- Windows：MSI/EXE（由 Tauri bundle 目标与环境决定）
- macOS：`.app` + `.dmg`
- Linux：`.deb` / `.AppImage`

## 文档索引

- 架构：`docs/ARCHITECTURE.md`
- 威胁建模（STRIDE）：`docs/THREAT_MODEL.md`
- 运维手册：`docs/RUNBOOK.md`
- 开发指南：`docs/DEVELOPMENT.md`
- 内部 API：`docs/API.md`
- ADR：`docs/adr/README.md`

## 故障排查

### 1) `trader_futu_connector` 构建失败（`build.rs`, No such file or directory）

如果看到类似：

```text
failed to run custom build command for `trader_futu_connector`
Error: Os { code: 2, kind: NotFound, message: "No such file or directory" }
```

按顺序检查：

1. 在仓库根目录执行构建命令（不是子目录）。
2. 验证 proto 文件存在：
   ```bash
   ls packages/connectors/futu/proto/*.proto | wc -l
   ```
   期望值：`22`
3. 清理该 crate 后重建：
   ```bash
   cargo clean -p trader_futu_connector
   pnpm build
   ```
4. 若仍失败，使用详细日志定位：
   ```bash
   cargo build -p trader_futu_connector -vv
   ```

当前 `build.rs` 已做路径稳健化（基于 `CARGO_MANIFEST_DIR`）并增加 proto 存在性检查。

### 2) Linux 桌面端无法启动

通常是 WebKit/GTK/libsecret 依赖缺失，按本文前置依赖安装系统包后重试。

### 3) 实盘始终无法解锁

优先检查：profile 是否为 `live`、交易环境是否 `real`、确认短语是否完全一致、风险参数是否非默认、`futu.trade_password` 是否已写入系统密钥库。

## KNOWN LIMITATIONS

- 审计日志回放（live session deterministic replay）尚未提供完整 UI。
- OpenD 原生 TLS 在当前协议路径不可用，远程部署建议走 SSH/VPN 隧道。
- 自动更新签名链路默认未启用（有打包与流程文档，签名需自行注入密钥）。
- SQLite 目前未启用透明加密（Secrets 已通过 OS Keychain 保护）。

## Roadmap

1. 增加完整回放引擎与回放 UI（按审计日志重建会话）。
2. 扩展执行层故障注入与长稳压测（断线、重连、重复报文、时钟漂移）。
3. 增强模型体系（数据集注册、walk-forward 矩阵、Python sidecar 监督协议）。
4. 完善发布签名与自动更新验签（含 notarization/签名流水线）。
5. 提供可选 SQLCipher 存储配置。

## License

[Apache-2.0](./LICENSE)
