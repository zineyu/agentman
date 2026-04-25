# Agent任务管理系统 - Phase 1 MVP 实现计划

## 目标

构建最小可用产品（MVP）：支持人类+Agent混合任务管理，Agent任务自动执行并转待审核。

**时间估算**：4-6周

---

## Phase 1 范围

### 包含功能
- [ ] 飞书Base表结构创建（任务主表、执行记录表、运行时表）
- [ ] Agent Daemon核心框架（Rust）
- [ ] 任务轮询与执行（支持Claude Code + OpenCode）
- [ ] 基础状态流转（待办→进行中→待审核→已完成）
- [ ] 任务预分配机制
- [ ] 日志回写（批量+实时混合策略）
- [ ] 自动clone仓库+创建分支
- [ ] 审核驳回后自动重试（最多3次）
- [ ] Daemon心跳上报

### 不包含（Phase 2+）
- 技能系统
- 多Daemon负载均衡
- Webhook实时触发
- 工时统计看板
- 高级任务依赖
- 评论/协作功能
- 标签系统

---

## 任务分解

### Week 1: 飞书Base数据层搭建

#### Day 1-2: 环境准备与认证
- [ ] **TASK-1.1**: 确认lark-cli认证可用，获取tenant_access_token
  - 验证 `lark-cli auth status` 
  - 确认有 `task:task:read`, `task:task:write` 等权限
  - 测试Base API访问权限
- [ ] **TASK-1.2**: 创建测试Base（如未提供）
  - 使用 `lark-cli base +base-create` 或确认已有Base token

#### Day 3-4: 创建核心数据表
- [ ] **TASK-1.3**: 创建任务主表（Tasks）
  - 字段：任务编号、任务标题、任务描述、执行者类型、执行者、任务状态、优先级
  - 字段：开始时间、截止时间、完成时间、创建人、创建时间、最后更新
  - 字段：Agent类型、工作目录、仓库地址、分支名称、审核人、审核意见、审核驳回理由
  - 字段：重试次数、催办次数、最后催办时间、分配的运行时
  - **依赖**: TASK-1.2完成
- [ ] **TASK-1.4**: 创建执行记录表（ExecutionLogs）
  - 字段：关联任务、执行序号、Agent类型、执行状态、开始/结束时间、执行输出、错误信息、提交记录
  - **依赖**: TASK-1.3完成（需要任务主表先存在）
- [ ] **TASK-1.5**: 创建运行时表（Runtimes）
  - 字段：运行时ID、主机名、IP地址、可用Agent、状态、最后心跳、当前任务、操作系统、版本号

#### Day 5: 表结构验证
- [ ] **TASK-1.6**: 验证所有表结构正确
  - 使用 `+field-list` 确认每个表的字段
  - 手动创建1-2条测试记录验证字段类型
  - **验收标准**: 能通过API正确读写所有字段

---

### Week 2: Agent Daemon框架开发

#### Day 1-2: 项目脚手架与配置
- [ ] **TASK-2.1**: 初始化Rust项目
  ```bash
  cargo new --bin agent-daemon
  cd agent-daemon
  ```
- [ ] **TASK-2.2**: 添加核心依赖
  - `tokio` (full features)
  - `reqwest` (json, rustls-tls)
  - `serde` + `serde_json`
  - `tokio-util`
  - `which`
  - `clap`
  - `config` (toml配置)
  - `chrono`
  - `anyhow` + `thiserror`
  - `tracing` + `tracing-subscriber`
- [ ] **TASK-2.3**: 配置文件解析
  - 支持 `config.toml` 和环境变量覆盖
  - 配置项：daemon、feishu、polling、agents、execution、git

#### Day 3: 飞书Base API客户端
- [ ] **TASK-2.4**: 实现BaseClient
  - 读取lark-cli配置获取access_token
  - 封装HTTP请求（GET/POST/PATCH）
  - 错误处理和重试逻辑（指数退避）
  - API限流保护（令牌桶或简单QPS限制）
  - **关键方法**:
    - `query_records(table_id, filter)` - 查询记录
    - `update_record(table_id, record_id, fields)` - 更新记录
    - `create_record(table_id, fields)` - 创建记录

#### Day 4-5: 数据模型与心跳
- [ ] **TASK-2.5**: 定义数据模型（models.rs）
  - `Task` 结构体（对应任务主表）
  - `ExecutionLog` 结构体（对应执行记录表）
  - `Runtime` 结构体（对应运行时表）
  - 枚举：TaskStatus、ExecutorType、AgentType
- [ ] **TASK-2.6**: 实现心跳上报
  - 每60秒更新运行时表的"最后心跳"和"状态"
  - Daemon启动时注册运行时（创建或更新记录）
  - Daemon退出时更新状态为"离线"
  - **验收标准**: 运行时表能正确显示Daemon在线状态

---

### Week 3: 核心执行逻辑

#### Day 1-2: 任务轮询器
- [ ] **TASK-3.1**: 实现TaskPoller
  - 每30秒轮询任务主表
  - 查询条件：
    ```
    执行者类型 = "agent"
    AND 任务状态 = "待办"
    AND 分配的运行时 = "本Daemon的runtime_id"
    AND (重试次数 IS NULL OR 重试次数 < 3)
    ```
  - 检测到可执行任务时，原子更新状态为"进行中"并记录开始时间
  - 使用记录version或乐观锁防止并发冲突
- [ ] **TASK-3.2**: Agent CLI检测
  - 实现 `AgentManager`
  - 检测配置中启用的CLI命令是否在PATH中
  - 维护可用Agent列表
  - **验收标准**: 启动时能正确打印检测到的Agent

#### Day 3-4: 任务执行器
- [ ] **TASK-3.3**: 实现TaskExecutor
  - `AgentAdapter` trait定义
  - `ClaudeCodeAdapter` 实现
  - `OpenCodeAdapter` 实现（如时间允许）
  - 执行流程：
    1. 准备环境（clone仓库、创建分支）
    2. 构建提示词（任务描述 + 上下文）
    3. 调用Agent CLI（使用tokio::process::Command）
    4. 实时捕获stdout/stderr
    5. 监控执行状态
- [ ] **TASK-3.4**: Git环境准备
  - `GitPreparer` 模块
  - 方法：`prepare_workspace(task)`
    - 检查仓库是否已clone
    - 如未clone，执行 `git clone`
    - 创建分支：`git checkout -b agent-task-{task_id}`
    - 切换工作目录到任务目录
  - **验收标准**: 能正确clone仓库并创建分支

#### Day 5: 日志回写系统
- [ ] **TASK-3.5**: 实现LogWriter
  - 缓冲区：Vec<String>（最大10条）
  - 实时触发关键词："error", "failed", "success", "commit", "push"
  - 回写目标：执行记录表的"执行输出"字段
  - 策略：
    - 普通日志：累积到10条后批量追加
    - 关键词日志：立即追加
    - 执行结束时：flush剩余日志
  - **验收标准**: 日志能正确回写到Base

---

### Week 4: 状态流转与审核

#### Day 1-2: 状态更新
- [ ] **TASK-4.1**: 执行完成处理
  - Agent执行完成后：
    1. 更新任务状态为"待审核"
    2. 设置审核人 = 创建人
    3. 记录完成时间
    4. 创建执行记录（状态=成功/失败）
  - 执行失败处理：
    1. 记录错误信息
    2. 如果重试次数 < 3：更新状态为"待办"（等待重新执行）
    3. 如果重试次数 >= 3：更新状态为"已取消"或保持"待审核"（人工介入）

#### Day 3-4: 审核驳回处理
- [ ] **TASK-4.2**: 驳回检测与重试
  - 轮询逻辑扩展：
    ```
    查询条件增加：
    OR (任务状态 = "进行中" AND 重试次数 > 0 AND 审核驳回理由 IS NOT NULL)
    ```
  - 重新执行时：
    1. 将驳回理由追加到提示词上下文
    2. 更新执行序号（执行记录表）
    3. 执行完成后清空审核驳回理由
- [ ] **TASK-4.3**: 添加系统评论
  - 驳回时：添加评论"审核驳回：{理由}，开始第{重试次数}次重试"
  - 完成时：添加评论"任务执行完成，等待审核"
  - 失败时：添加评论"执行失败：{错误摘要}"

#### Day 5: 催办工作流（仅人类任务）
- [ ] **TASK-4.4**: 实现催办逻辑
  - 飞书工作流配置（用户手动配置或提供配置脚本）
  - 条件：执行者类型=human、状态=待办/进行中、即将到期、催办次数<3
  - 动作：催办次数+1、发送飞书消息
  - **注**：Phase 1可简化，先提供工作流JSON配置，用户手动导入

---

### Week 5: 集成测试与Bug修复

#### Day 1-2: 端到端测试
- [ ] **TASK-5.1**: 编写测试用例
  - 场景1：创建Agent任务 → Daemon自动执行 → 状态变为待审核
  - 场景2：审核通过 → 状态变为已完成
  - 场景3：审核驳回 → Daemon重新执行 → 再次待审核
  - 场景4：执行失败3次 → 停止重试
  - 场景5：多任务并发执行（max_concurrent=2）
- [ ] **TASK-5.2**: 手动端到端测试
  - 在测试Base上创建真实任务
  - 验证完整生命周期

#### Day 3-4: 错误处理与边界情况
- [ ] **TASK-5.3**: 强化错误处理
  - API请求失败的重试（指数退避）
  - Agent CLI未找到的处理
  - Git操作失败的处理
  - 工作目录权限问题
  - 长时间运行任务的超时处理
- [ ] **TASK-5.4**: 优雅退出
  - 信号处理（SIGINT/SIGTERM）
  - 当前执行任务的清理
  - 运行时状态更新为"离线"

#### Day 5: 性能优化
- [ ] **TASK-5.5**: 优化轮询效率
  - 使用视图筛选减少API返回数据量
  - 本地缓存运行时信息
  - 减少不必要的API调用

---

### Week 6: 文档与交付

#### Day 1-2: 使用文档
- [ ] **TASK-6.1**: 编写用户指南
  - 如何创建Agent任务
  - 如何配置Daemon
  - 状态流转说明
  - 审核操作指南
- [ ] **TASK-6.2**: 编写部署文档
  - 环境要求
  - 编译步骤
  - 配置文件说明
  - 启动/停止命令

#### Day 3-4: 代码整理
- [ ] **TASK-6.3**: 代码审查
  - 检查错误处理
  - 检查资源泄漏
  - 检查日志质量
- [ ] **TASK-6.4**: 添加单元测试
  - BaseClient单元测试（使用mock）
  - 配置解析测试
  - 日志缓冲测试

#### Day 5: 最终验收
- [ ] **TASK-6.5**: 最终端到端验证
  - 完整流程跑通
  - 所有验收标准通过
  - 交付物清单确认

---

## 验收标准

### 功能验收
- [ ] 能在飞书Base中创建Agent任务并正确存储
- [ ] Daemon启动后能自动发现并执行分配给自己的任务
- [ ] Agent执行完成后任务状态自动变为"待审核"
- [ ] 审核驳回后Daemon能自动重新执行（最多3次）
- [ ] 人类任务的催办工作流正常运行
- [ ] 执行日志能正确回写到Base

### 性能验收
- [ ] 轮询间隔30秒内响应新任务
- [ ] 日志回写延迟不超过1分钟（批量模式）
- [ ] 同时执行2个任务不互相干扰
- [ ] API调用频率不超过飞书限制（20 QPS）

### 稳定性验收
- [ ] Daemon连续运行24小时无崩溃
- [ ] 网络断开后能自动恢复
- [ ] 异常任务不影响其他任务执行
- [ ] 优雅退出不遗留僵尸进程

---

## 依赖与风险

### 外部依赖
| 依赖 | 状态 | 备注 |
|------|------|------|
| 飞书Base | 待创建 | Week 1完成 |
| lark-cli认证 | 待确认 | 需要用户确认已登录 |
| Agent CLI安装 | 用户负责 | Claude Code/OpenCode需用户安装 |
| Git仓库访问 | 用户负责 | Daemon需要仓库读写权限 |

### 风险应对
| 风险 | 概率 | 影响 | 应对 |
|------|------|------|------|
| 飞书API变更 | 低 | 高 | 封装BaseClient，隔离变化 |
| Agent CLI行为不一致 | 中 | 中 | 适配器模式隔离差异 |
| 用户环境差异 | 高 | 中 | 提供详细部署文档 |
| 时间超支 | 中 | 中 | Phase 1严格控制范围 |

---

## 交付物

1. **源代码**: `agent-daemon/` 目录下的完整Rust项目
2. **配置文件模板**: `config.example.toml`
3. **设计文档**: `docs/superpowers/specs/architecture-design.md`（已存在）
4. **用户指南**: `docs/user-guide.md`
5. **部署文档**: `docs/deployment.md`
6. **飞书Base表结构**: 导出配置或创建脚本

---

*计划版本：v1.0*
*基于设计文档：architecture-design.md v1.0*
*创建时间：2025-04-25*