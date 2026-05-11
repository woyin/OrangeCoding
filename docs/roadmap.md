# OrangeCoding 系统升级路线图

> 基于 `docs/analysis.md` 分析报告和已完成的 TODO.md 任务，
> 本文档定义从当前状态到完整 Agent OS 的演进路径。

---

## 当前状态

### 已完成
- **Reference 学习** (Phase 1): `docs/analysis.md` — 7 大模块分析完成
- **TODO 规划与实现** (Phase 2): 28 项任务全部完成
  - Tools: T-001~T-005 (元数据、验证、权限、并发、钩子)
  - Context: C-001~C-005 (压缩、预算、分组、触发器、重注入)
  - Agent: A-001~A-004 (任务系统、取消令牌、Fork、邮箱)
  - Memory: M-001~M-007 (分类、存储、索引、会话、召回、AutoDream)
  - Verification: V-001~V-002 (验证框架、工具摘要)
  - Buddy: B-001~B-002 (确定性身份、异步观察者)
  - KAIROS: K-001~K-003 (后采样钩子、建议引擎、上下文提示)
- **Web 控制面 Phase A**: 本地 Web 控制 (协议/服务器/Worker/CLI serve)
- **不变量提取** (Phase 3): `docs/invariants.md` — 18 条规则 8 个类别
- **不变量测试** (Phase 4): 6 个集成测试套件
- **不变量检查器** (Phase 5): `orangecoding-invariant` crate (checker/report/rules)
- **Pre-Check Gate** (Phase 6): 集成到 agent executor，自主模式下拦截 git commit
- **Runtime Guard** (Phase 7): 集成到 agent executor，拦截高危工具调用
- **Auto Rollback** (Phase 8): rollback 模块 + Goal 配置集成
- **Verification Agent** (Phase 9): 5 项检查流水线 + Goal 配置集成
- **Self-Healing** (Phase 10): 检测→建议→修复→验证生命周期
- **Self-Evolving** (Phase 11): 模式学习→策略生成→快照比较
- **Agent OS 文档** (Phase 12): `docs/agent_os.md` — 完整架构文档

### 缺失
- Web 控制面 Phase B/C

---

## 依赖关系图

```text
Phase 2b: roadmap.md ─────────────────────────────────────┐
                                                          │
Phase 3: invariants.md ◄──────────────────────────────────┘
    │
Phase 4: invariant tests ◄───────────────────────────────┘
    │
Phase 5: invariant checker (orangecoding-invariant crate) ◄─────┘
    │
    ├──► Phase 6: pre-check gate
    │
    ├──► Phase 7: runtime guard ──► Phase 8: auto rollback
    │                                    │
    ├──► Phase 9: verification agent     │
    │         │                          │
    │         └──► Phase 10: self-healing ◄─┘
    │                   │
    │                   └──► Phase 11: self-evolving
    │                              │
    │                              └──► Phase 12: Agent OS doc
    │
    ├──► Web Phase B: remote worker ──► Web Phase C: public control
    │
    └──► Enhanced Review System
              │
              └──► Review Round 1 → 2 → 3
```

---

## Phase 3: 系统不变量提取

**输出**: `docs/invariants.md`

**覆盖范围**:

| 不变量类别 | 行为规则 | 严重性 |
|-----------|---------|--------|
| Auth-WS | WebSocket 连接必须携带有效 token | Critical |
| Auth-API | HTTP API 必须通过认证中间件 | Critical |
| Cancellation | 取消信号必须传播到所有子任务 | High |
| Session-Continuity | 会话上下文必须跨 turn 持久化 | High |
| Tool-Permission | 高危工具执行前必须通过权限检查 | Critical |
| Context-Consistency | 压缩后上下文不得丢失关键信息 | Medium |
| Audit-Completeness | 所有高危操作必须有审计记录 | High |
| Event-Ordering | 事件序列必须保持因果顺序 | Medium |

---

## Phase 4: 不变量测试

**目录**: `tests/invariants/`

**每条不变量**:
1. 正常路径测试 (PASS)
2. 违规路径测试 (必须检测到违规)
3. 边界条件测试

---

## Phase 5: 不变量检查器

**输出**: `crates/orangecoding-invariant/`

**模块**:
- `checker.rs` — 不变量验证执行器
- `report.rs` — 违规报告生成
- `rules.rs` — 规则定义与加载

---

## Phase 6: Pre-Check Gate

**集成到**: `crates/orangecoding-invariant/src/gate.rs`

**能力**:
- 分析 `git diff` 输出
- 检测受影响的不变量
- 高风险变更 → 阻止提交

---

## Phase 7: Runtime Guard

**集成到**: `crates/orangecoding-invariant/src/runtime_guard.rs`

**拦截项**:
- 未鉴权 WebSocket 连接
- 未授权工具调用
- 未传播的取消信号
- 丢失的会话状态

---

## Phase 8: Auto Rollback

**集成到**: `crates/orangecoding-invariant/src/rollback.rs`

**触发条件**:
- 测试失败
- 不变量违规
- 运行时违规

**行为**:
- 自动 `git revert`
- 输出 `docs/rollback_log.md`

---

## Phase 9: Verification Agent

**集成到**: `crates/orangecoding-agent/src/verification/`

**检查项**:
- 是否符合 `analysis.md` 设计
- 是否违反不变量
- 是否存在绕过路径
- 是否引入新 bug

**输出**: `docs/verification/<ID>.md`

---

## Phase 10: Self-Healing

**集成到**: `crates/orangecoding-invariant/src/healing.rs`

**流程**:
1. 检测不变量违规
2. 生成修复建议 (`docs/fix_suggestions.md`)
3. TDD 修复
4. 验证
5. 提交

---

## Phase 11: Self-Evolving

**输出**:
- `docs/evolution_data.md`
- `docs/evolution_patterns.md`
- `docs/evolution_strategies.md`

**流程**:
1. 识别失败模式 (invariant_report + verification + rollback logs)
2. 生成优化策略
3. 执行一个策略
4. 对比效果
5. 无效则回滚

---

## Phase 12: Agent OS Architecture

**输出**: `docs/agent_os.md`

**结构**:
1. Runtime Kernel (tools / memory / session)
2. Invariant Engine (规则 + 校验)
3. Guard System (pre-check + runtime)
4. Orchestration (task graph / agent team)
5. Agent Layer (planner / executor / verifier)
6. Evolution Engine (自进化)

---

## Web Control Plane Phases

### Phase B: Remote Worker Link
- Worker 主动连接 Gateway
- Gateway 会话路由到远程 Worker
- 审批穿透到远程 Worker
- 断线重连与会话恢复

### Phase C: Public Control Plane
- OIDC/OAuth 2.1 登录
- RBAC (viewer/operator/admin)
- Worker token / mTLS 证书
- 审计增强
- 速率限制
- 管理后台

---

## Enhanced Review System

**能力**:
- 多维度代码审查 (正确性/安全/性能/可维护性/测试)
- 迭代改进循环 (审查 → 修复 → 再审查)
- 结构化 JSON 输出
- 改进后的 diff 生成

---

## 质量保证

每个 Phase 完成后:
1. `cargo test --workspace` 全量通过
2. `cargo check --workspace` 无新增错误
3. 不变量检查通过 (Phase 5 之后)
4. Verification Agent 验证 (Phase 9 之后)
5. 代码审查 (3 轮)
