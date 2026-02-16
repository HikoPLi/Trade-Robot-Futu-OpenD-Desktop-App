# User Guide (High-Standard SOP)

> 应用内入口：左侧导航 `教程 / Guide`（支持 `中文 / 繁體中文 / English` 三语）。
> 建议先在应用内按清单执行，再结合本手册做团队培训与审计归档。

本手册是面向交易员、策略开发、风控与运维的统一操作说明。目标不是“功能介绍”，而是提供可执行、可审计、可复盘的高标准操作流程。

适用版本：当前仓库主干版本（`main`）。  
适用模式：`paper`、`research`、`live`（默认仅建议先用 `paper`）。

## 1. 使用原则

1. 默认纸上交易：所有新策略、新模型、新参数，先在 `paper` 验证。
2. 风险优先：当信息不充分、模型异常、连接异常时，优先 `hold` 或停机，不强行交易。
3. 先风控后收益：先定义最大损失与暴露，再定义收益目标。
4. 所有关键动作可审计：关键配置、下单、策略信号、风控拒单均要落在 `Audit` 与结构化日志中。
5. 实盘必须有退出机制：全局 Kill Switch 热键必须先配置并验证。

## 2. 15 分钟快速上手

### 2.1 启动

```bash
pnpm install
pnpm dev
```

### 2.2 首次配置（推荐顺序）

1. `Settings -> Language` 选择语言。
2. `Settings -> Profiles` 选择 `paper`。
3. `Settings -> Risk Limits` 设置非默认风控（即便纸上也建议）。
4. `Settings -> Kill Switch Hotkey` 设置并应用快捷键。
5. `Market` 添加 3-5 个标的到 watchlist。
6. `Trading` 手动下 1 笔小单，确认订单与持仓更新正常。
7. `Strategies` 启动一个内置策略（先小参数）。
8. `Audit` 查看是否有 `signal_fired`、`order_placed`、`order_filled`。

## 3. 页面操作说明

### 3.1 Dashboard（总览）

- 关注：
  - `Engine` 状态（`running` / `halted`）
  - `safe_mode` 是否触发
  - `kill_switch` 是否触发
  - 当日 PnL、暴露、策略实例数量
- 任何状态不确定时：立刻触发 Kill Switch。

### 3.2 Market（行情与 K 线）

- Watchlist：输入 `US.AAPL`、`HK.00700` 等标准符号。
- K 线周期：
  - `paper/research`：`5s`、`1m`、`5m`
  - `live`：`1m`、`5m`（OpenD 限制）
- 若 K 线没有更新：
  1. 点击 `Refresh`
  2. 检查 `Diagnostics` 是否有实时事件
  3. 检查当前 profile 与 OpenD 连接状态

### 3.3 Trading（手动交易）

- 下单前核对：
  - 符号、方向、数量、类型（市价/限价）
  - 限价是否落在价格带内
  - 当前持仓与最大持仓限制
- 异常处理：
  - 下单报错先看错误文本，再去 `Audit` 查 `order_rejected` 明细。

### 3.4 Strategies（策略工作台）

- 生命周期建议：`draft -> paper -> live`
- 新策略上线流程：
  1. 在 `draft` 调参
  2. 在 `paper` 连续运行并记录结果
  3. 达到门槛后再切换 `live`
- 推荐先用：
  - `MA Crossover`
  - `Mean Reversion`
  - `Short-Term Momentum Bot`

### Short-Term Momentum Bot（短线机器人）建议参数模板

- 保守模板（推荐）：
  - `lookback`: 12
  - `entry_momentum_bps`: 25-35
  - `min_volatility_bps`: 8-15
  - `take_profit_bps`: 30-50
  - `stop_loss_bps`: 15-25
  - `trade_qty`: 小仓位起步
  - `ai_confirm`: true（先在 paper 验证）
  - `min_ai_confidence`: 0.65-0.75

### 3.5 Models（模型管理）

- 支持：
  - 内置模型
  - ONNX 注册与离线评估
  - Model API 路由（OpenAI / DeepSeek / Qwen / Grok / Ollama）
- 建议流程：
  1. 先离线评估（固定 seed）
  2. 看指标：`IC`、`accuracy`、样本量、误差分布
  3. 再做 paper 联机验证

### 模型 API 路由配置（Settings）

1. 在 `Settings -> Model API Routing` 配置 provider：
   - `base_url`、`model`、`timeout`、`max_tokens`
2. 在 `Settings -> Credentials` 写入对应 API Key（OS Keychain）。
3. 先 `Test Provider`，确认可返回结构化信号。
4. 再在策略参数里开启 `ai_confirm`。

说明：AI 只做辅助决策门禁，不保证收益。

### 3.6 Backtest（回测）

- 回测前准备：
  - 明确 symbol 与策略参数
  - 固定 fee/slippage
  - 固定随机种子与数据集
- 输出解读：
  - `Sharpe`、`Max Drawdown`、`Hit Rate`、`Turnover`
  - 关注回撤与稳定性，不只看总收益

### 3.7 Audit / Diagnostics（审计与诊断）

- `Audit`：按 `event_type`、`trace_id` 过滤，支持 JSONL 导出。
- `Diagnostics`：看事件流、快照、profile 状态。
- 事故排查优先级：
  1. Audit 事件时间线
  2. 结构化日志 `trade_robot.jsonl`
  3. 配置快照与策略参数

## 4. 实盘前 Go / No-Go 清单

满足全部条件才允许进入实盘：

- [ ] 当前 profile 为 `live`
- [ ] OpenD 交易环境为 `real`
- [ ] 已设置非默认风控限制
- [ ] 已配置并验证 Kill Switch 热键
- [ ] `futu.trade_password` 已写入 OS Keychain
- [ ] OpenD 连接测试通过
- [ ] 风险声明已阅读并确认短语输入正确：
  - `I UNDERSTAND LIVE TRADING RISK`
- [ ] 至少一次完整 paper 演练已通过（含下单、策略、停止、审计导出）

任一不满足：`No-Go`。

## 5. 日常运行 SOP（高标准）

### 5.1 开盘前

1. 检查引擎状态（非 safe mode，kill switch 未触发）。
2. 检查连接质量（行情更新正常）。
3. 检查风险参数（特别是日亏阈值、仓位上限、频率限制）。
4. 检查策略与模型版本（避免临时变更）。
5. 运行 1 次小额纸上/试单流程验证链路。

### 5.2 盘中

1. 每 15-30 分钟看一次风险与订单拒单率。
2. 异常波动时降低策略并发或停策略观察。
3. 发现异常信号密度、重复下单倾向、连接不稳：
   - 先 Kill Switch
   - 再定位原因

### 5.3 收盘后

1. 导出审计 JSONL。
2. 汇总当日策略表现与异常事件。
3. 记录参数变更与版本差异。
4. 将次日变更先放到 `paper` 计划中，不直接改实盘。

## 6. 故障与应急

### 6.1 一键停机

- UI 顶栏 `Kill Switch`
- 或全局热键

效果：
- 停策略
- 拦截新单
- 撤销未完成订单（实盘为 best-effort）
- 引擎转 `halted`

### 6.2 崩溃恢复

- 若上次异常退出，系统可能进入 `safe_mode`
- 恢复步骤：
  1. 保留日志与审计证据
  2. 找到根因并修复
  3. 先在 paper 重现与验证
  4. 再恢复正常运行

## 7. 数据与安全

- Secrets 永不明文落盘，只放 OS Keychain。
- 常见密钥：
  - `futu.trade_password`
  - `ai.openai_api_key`
  - `ai.deepseek_api_key`
  - `ai.qwen_api_key`
  - `ai.grok_api_key`
  - `ai.ollama_api_key`
- 建议：
  - 每 90 天轮换 API Key
  - 最小权限原则
  - 生产环境禁用无关 provider

## 8. 团队培训方法（教程模板）

### 8.1 90 分钟训练营

1. 0-20 分钟：系统安全模型与风控边界
2. 20-45 分钟：paper 实操（行情、下单、策略、审计）
3. 45-65 分钟：回测与模型评估
4. 65-80 分钟：实盘解锁流程演练（不实际下实盘单）
5. 80-90 分钟：故障应急演练（Kill Switch + 审计导出）

### 8.2 考核标准

- 能独立完成 `paper` 全链路
- 能解释每条核心风控限制
- 能在 30 秒内触发停机并导出审计
- 能给出“Go / No-Go”判断并说明依据

## 9. 常见问题（FAQ）

### Q1: 为什么有信号但没下单？

可能原因：
- 风控拒绝（数量、价格带、时段、频率）
- AI 门禁未通过（`ai_confirm=true` 且 confidence/action 不满足）
- 引擎已 `halted` 或 Kill Switch 已触发

### Q2: 为什么 live 看不到 5s K 线？

OpenD 实时 K 线在当前实现仅支持 `1m/5m`。`5s` 仅在 `paper/research` 可用。

### Q3: 模型 API provider 测试失败怎么办？

按顺序检查：
1. `base_url` 与 `model`
2. 对应 API Key 是否写入 Keychain
3. 网络连通性与超时
4. provider 返回是否为可解析 JSON 结构

## 10. 参考文档

- `docs/ARCHITECTURE.md`
- `docs/API.md`
- `docs/RUNBOOK.md`
- `docs/THREAT_MODEL.md`
- `README.md`
