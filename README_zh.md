# Slack Coder Bot

一个集成 Claude AI 的 Slack 机器人，直接在 Slack 频道中提供智能代码生成和文档协助。机器人会分析你的代码仓库，学习你的编码规范，并帮助你编写符合项目风格的代码。

## 特性

- **仓库感知**：分析代码库以理解约定、模式和架构
- **频道隔离**：每个 Slack 频道可以使用不同的代码仓库
- **实时进度**：TodoWrite 钩子集成显示实时进度更新
- **上下文感知**：在线程中维护对话上下文
- **完整 Claude SDK 支持**：访问所有 Claude Agent SDK 功能（文件操作、git、gh CLI）

## 架构

### 系统概览

```mermaid
graph TB
    User[Slack 用户] -->|提及 @bot| Slack[Slack API]
    Slack -->|Socket Mode WebSocket| Bot[Slack Coder Bot]
    Bot -->|设置请求| MainAgent[主 Claude Agent]
    Bot -->|代码/命令请求| RepoAgent[仓库专用 Agent]

    MainAgent -->|gh repo view| GitHub[GitHub API]
    MainAgent -->|gh repo clone| FS[文件系统]
    MainAgent -->|分析代码库| FS
    MainAgent -->|保存| SystemPrompt[系统提示词 .md]

    RepoAgent -->|读取| SystemPrompt
    RepoAgent -->|代码操作| RepoFS[仓库文件]
    RepoAgent -->|git/gh 命令| GitHub

    Bot -->|进度更新| Slack
    RepoAgent -->|流式响应| Bot
    MainAgent -->|进度更新| Bot

    style Bot fill:#e1f5ff
    style MainAgent fill:#ffe1f5
    style RepoAgent fill:#f5ffe1
```

### 组件架构

```mermaid
graph TB
    subgraph "Slack 层"
        SC[SlackClient<br/>API 包装器]
        EH[EventHandler<br/>Socket Mode]
        MP[MessageProcessor<br/>消息路由]
        FH[FormHandler<br/>设置表单]
        CH[CommandHandler<br/>/help, /new-session]
        PT[ProgressTracker<br/>TodoWrite 钩子]
        MD[markdown_to_slack<br/>格式转换]
    end

    subgraph "Agent 管理层"
        AM[AgentManager<br/>生命周期]
        MA[MainAgent<br/>仓库设置]
        RA[RepoAgent<br/>代码生成]
        DM[DashMap&lt;ChannelId, RepoAgent&gt;<br/>Agent 池]
    end

    subgraph "存储层"
        WS[Workspace<br/>路径管理]
        RP["repos/channel_id/<br/>仓库克隆"]
        SP["system/channel_id/<br/>system_prompt.md"]
    end

    subgraph "会话管理"
        SM[SessionId 生成器<br/>基于 UUID]
        SS[会话状态<br/>每个 RepoAgent]
    end

    subgraph "外部服务"
        Claude[Claude API<br/>claude-agent-sdk-rs]
        GitHub[GitHub<br/>gh CLI]
        Git[Git<br/>仓库操作]
    end

    EH --> MP
    EH --> FH
    MP --> CH
    MP --> AM
    FH --> AM
    AM --> MA
    AM --> RA
    AM --> DM
    MA --> WS
    RA --> WS
    RA --> SM
    WS --> RP
    WS --> SP
    PT -.->|钩子| MA
    PT -.->|钩子| RA
    PT --> SC
    MP --> MD
    MD --> SC
    MA --> Claude
    RA --> Claude
    MA --> GitHub
    RA --> GitHub
    RA --> Git

    style SC fill:#4a9eff
    style PT fill:#ff9e4a
    style MA fill:#9eff4a
    style DM fill:#ff4a9e
    style Claude fill:#ffeb3b
    style SM fill:#e1bee7
```

### 数据流架构

```mermaid
graph LR
    subgraph "输入流"
        U[用户消息] --> SE[Slack 事件]
        SE --> DD[去重缓存]
        DD --> EM[事件匹配]
    end

    subgraph "处理流"
        EM -->|命令| CMD[命令路由]
        EM -->|仓库模式| SETUP[设置流程]
        EM -->|文本| QUERY[查询流程]

        CMD --> HC[帮助命令]
        CMD --> NSC[新会话命令]

        SETUP --> VA[验证仓库]
        VA --> CL[克隆仓库]
        CL --> AN[分析代码]
        AN --> GP[生成提示词]
        GP --> CR[创建 Agent]

        QUERY --> GA[获取 Agent]
        GA --> SQ[发送查询]
        SQ --> SR[流式响应]
    end

    subgraph "输出流"
        HC --> FMT[格式化 Markdown]
        NSC --> FMT
        CR --> FMT
        SR --> FMT
        FMT --> SPLIT[分割块]
        SPLIT --> SLACK[发送到 Slack]
    end

    style EM fill:#b3e5fc
    style SETUP fill:#ffccbc
    style QUERY fill:#c8e6c9
    style FMT fill:#f8bbd0
```

### 仓库设置流程（详细）

```mermaid
sequenceDiagram
    participant U as 用户
    participant S as Slack
    participant EH as EventHandler
    participant FH as FormHandler
    participant AM as AgentManager
    participant MA as MainAgent
    participant PT as ProgressTracker
    participant FS as 文件系统
    participant GH as GitHub (gh CLI)
    participant C as Claude API

    U->>S: 邀请 @SlackCoderBot 到 #project
    S->>EH: app_mention 事件 (channel_join)
    EH->>FH: show_repo_setup_form()
    FH->>S: 显示欢迎消息 + 说明

    U->>S: 提及机器人并输入 "owner/repo"
    S->>EH: app_mention 事件
    EH->>EH: 解析仓库模式 (owner/repo)
    EH->>FH: handle_repo_setup(channel, "owner/repo")

    FH->>AM: setup_channel(channel, repo_name)
    AM->>AM: 创建带钩子的 MainAgent
    AM->>MA: new(settings, workspace, tracker, channel)
    MA->>MA: 加载 main-agent-system-prompt.md
    MA->>MA: 创建 TodoWrite 钩子
    MA-->>AM: MainAgent 实例

    AM->>MA: connect()
    MA->>C: 连接到 Claude API
    C-->>MA: 连接已建立

    AM->>MA: setup_repository(repo_name, channel)
    MA->>C: 发送带任务的设置提示词

    Note over MA,C: Claude 执行带 TodoWrite 的任务

    MA->>MA: TodoWrite: 验证仓库
    MA->>PT: 触发 PostToolUse 钩子
    PT->>S: 更新: ⏳ 验证仓库中...

    MA->>GH: gh repo view owner/repo
    GH-->>MA: 仓库元数据

    MA->>MA: TodoWrite: 克隆仓库
    MA->>PT: PostToolUse 钩子
    PT->>S: 更新: ✅ 已验证, ⏳ 克隆中...

    MA->>GH: gh repo clone owner/repo
    GH->>FS: 克隆到 ~/.slack_coder/repos/C123/
    FS-->>MA: 仓库已克隆

    MA->>MA: TodoWrite: 分析代码库
    MA->>PT: PostToolUse 钩子
    PT->>S: 更新: ✅ 已克隆, ⏳ 分析中...

    MA->>FS: 读取 package.json, Cargo.toml 等
    MA->>FS: 读取源文件
    FS-->>MA: 文件内容
    MA->>MA: 检测模式、约定

    MA->>MA: TodoWrite: 生成系统提示词
    MA->>PT: PostToolUse 钩子
    PT->>S: 更新: ✅ 已分析, ⏳ 生成中...

    MA->>MA: 创建仓库特定指令
    MA->>FS: 写入 ~/.slack_coder/system/C123/system_prompt.md
    FS-->>MA: 文件已写入

    MA->>MA: TodoWrite: 完成
    MA->>PT: PostToolUse 钩子
    PT->>S: 更新: ✅ 全部完成!

    MA->>C: 断开连接
    C-->>MA: 已断开
    MA-->>AM: 设置完成

    AM->>AM: create_repo_agent(channel)
    AM->>AM: 使用系统提示词创建 RepoAgent
    AM->>AM: 存储到 DashMap<ChannelId, RepoAgent>
    AM->>S: "🤖 Agent 就绪 - 会话 ID: session-C123-..."

    Note over S: 频道现已准备就绪，可以查询
```

### 消息处理流程（代码生成）

```mermaid
sequenceDiagram
    participant U as 用户
    participant S as Slack
    participant EH as EventHandler
    participant MP as MessageProcessor
    participant AM as AgentManager
    participant RA as RepoAgent
    participant PT as ProgressTracker
    participant C as Claude API
    participant FS as 文件系统
    participant MD as Markdown 格式化

    U->>S: @SlackCoderBot 添加用户认证 API
    S->>EH: app_mention 事件
    EH->>EH: 检查去重缓存
    EH->>EH: 去除机器人提及

    EH->>MP: process_message(SlackMessage)
    MP->>MP: 检查是否为命令 (以 / 开头)
    MP->>AM: has_agent(channel)?
    AM-->>MP: true

    MP->>AM: get_repo_agent(channel)
    AM-->>MP: Arc<Mutex<RepoAgent>>

    MP->>RA: lock().await
    Note over MP,RA: 获取独占锁

    MP->>RA: query("添加用户认证 API")
    RA->>RA: 获取当前 session_id
    RA->>C: query_with_session(message, session_id)
    RA->>RA: 更新 last_activity 时间戳

    Note over RA,C: Claude 使用 TodoWrite 钩子处理

    RA->>RA: TodoWrite: 规划认证
    RA->>PT: PostToolUse 钩子
    PT->>S: 更新: ⏳ 规划认证中...

    RA->>FS: 读取现有认证代码
    FS-->>RA: 当前实现

    RA->>RA: TodoWrite: 生成认证模块
    RA->>PT: PostToolUse 钩子
    PT->>S: 更新: ⏳ 生成认证模块中...

    RA->>FS: 写入 src/auth/mod.rs
    RA->>FS: 写入 src/auth/jwt.rs
    FS-->>RA: 文件已创建

    RA->>RA: TodoWrite: 添加测试
    RA->>PT: PostToolUse 钩子
    PT->>S: 更新: ⏳ 添加测试中...

    RA->>FS: 写入 src/auth/tests.rs
    FS-->>RA: 测试已创建

    RA->>RA: TodoWrite: 完成
    RA->>PT: PostToolUse 钩子
    PT->>S: 更新: ✅ 完成!

    C-->>RA: 流式返回最终结果
    RA-->>MP: 结果消息

    MP->>MD: markdown_to_slack(result)
    MD-->>MP: Slack 格式文本

    MP->>MP: 检查消息大小 (40KB 限制)
    alt 消息 > 40KB
        MP->>S: 分块发送，带 "(continued...)"
    else 正常大小
        MP->>S: 发送格式化消息
    end

    MP->>RA: unlock()
    Note over MP,RA: 释放锁

    S-->>U: 显示带代码的响应
```

### 会话管理流程

```mermaid
sequenceDiagram
    participant U as 用户
    participant S as Slack
    participant MP as MessageProcessor
    participant CH as CommandHandler
    participant RA as RepoAgent
    participant C as Claude API

    Note over RA: Agent 启动时创建初始会话
    RA->>RA: session_id = generate_session_id(channel)
    Note over RA: 格式: session-C123-1234567890-a3f9b2

    U->>S: @bot /new-session
    S->>MP: 处理消息
    MP->>CH: handle_command("/new-session")
    CH->>RA: start_new_session()

    RA->>RA: 生成新的 session_id
    RA->>RA: 更新 current_session_id (RwLock)
    RA->>C: 后续查询使用新的 session_id
    Note over RA,C: 之前的对话上下文已清除

    RA-->>CH: new_session_id
    CH->>S: "新会话已启动\n会话 ID: session-C123-..."

    Note over U,C: 此频道中的所有未来消息<br/>使用新的会话 ID
```

### TodoWrite 钩子处理流程

```mermaid
sequenceDiagram
    participant C as Claude API
    participant RA as RepoAgent/MainAgent
    participant H as TodoWrite 钩子
    participant P as Plan (Arc<Mutex>)
    participant PT as ProgressTracker
    participant S as Slack

    C->>RA: 执行工具: TodoWrite
    Note over C,RA: 工具输入包含 todos 数组

    RA->>H: 触发 PostToolUse 钩子
    H->>H: 将 tool_input 解析为 Plan

    H->>P: lock().await
    H->>P: update(new_plan)
    Note over P: 合并新任务与时间数据

    P->>P: 跟踪任务开始时间
    P->>P: 计算任务持续时间
    P-->>H: 带时间的更新计划

    H->>PT: update_progress(channel, plan)

    PT->>PT: 格式化进度消息
    Note over PT: 进度: 2/5<br/>当前: 生成代码<br/>✅ 规划 (2.3s)<br/>✅ 读取文件 (1.1s)<br/>⏳ 生成代码<br/>⬜ 添加测试<br/>⬜ 文档

    PT->>S: 更新或发送新消息
    alt 进度消息存在
        PT->>S: 更新现有消息
    else 无进度消息
        PT->>S: 发送新进度消息
    end

    S-->>PT: 消息已更新
    H-->>RA: 钩子处理完成
```

## 快速开始

**新手？** → [快速开始指南（15 分钟）](docs/QUICK_START.md)

**需要详细的 Slack 设置？** → [完整 Slack 设置指南](docs/SLACK_SETUP.md)

**机器人没有响应？** → [调试指南](docs/DEBUGGING.md)

## 设置

### 前置条件

1. **Rust**（2024 版本）
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **GitHub CLI** (`gh`)
   ```bash
   # macOS
   brew install gh

   # Linux
   sudo apt install gh

   # 认证
   gh auth login
   ```

3. **Git**
   ```bash
   git --version  # 应该已安装
   ```

### Slack 应用配置

1. **创建 Slack 应用**，访问 https://api.slack.com/apps
   - 点击 "Create New App" → "From scratch"
   - 名称: "Slack Coder Bot"
   - 选择你的工作区

2. **配置 OAuth & Permissions**
   - 导航到 "OAuth & Permissions"
   - 添加 Bot Token 作用域：
     - `app_mentions:read` - 读取提及
     - `channels:history` - 读取频道消息
     - `channels:read` - 列出频道
     - `chat:write` - 发送消息
     - `groups:history` - 读取私有频道消息
     - `groups:read` - 列出私有频道
     - `im:history` - 读取 DM
     - `im:read` - 列出 DM
     - `im:write` - 发送 DM
   - 将应用安装到工作区
   - 复制 **Bot User OAuth Token**（以 `xoxb-` 开头）

3. **启用 Socket Mode**
   - 导航到 "Socket Mode"
   - 启用 Socket Mode
   - 使用 `connections:write` 作用域创建应用级令牌
   - 复制 **App-Level Token**（以 `xapp-` 开头）

4. **订阅事件**
   - 导航到 "Event Subscriptions"
   - 启用事件
   - 订阅机器人事件：
     - `app_mention` - 机器人被提及时
     - `message.channels` - 频道消息
     - `message.groups` - 私有频道消息
     - `message.im` - 直接消息

5. **获取签名密钥**
   - 导航到 "Basic Information"
   - 复制 **Signing Secret**

### 安装

1. **克隆仓库**
   ```bash
   git clone https://github.com/tyrchen/slack-coder
   cd slack-coder
   ```

2. **配置环境**
   ```bash
   cp .env.example .env
   # 使用你的令牌编辑 .env
   ```

3. **在 `.env` 中设置环境变量**：
   ```env
   # Slack 配置
   SLACK_BOT_TOKEN=xoxb-your-bot-token-here
   SLACK_APP_TOKEN=xapp-your-app-token-here
   SLACK_SIGNING_SECRET=your-signing-secret-here

   # Claude 配置
   CLAUDE_API_KEY=your-claude-api-key-here
   CLAUDE_MODEL=claude-sonnet-4
   CLAUDE_MAX_TOKENS=8192

   # 工作区配置
   WORKSPACE_BASE_PATH=~/.slack_coder
   MAX_REPO_SIZE_MB=1024
   CLEANUP_INTERVAL_SECS=3600

   # Agent 配置
   MAIN_AGENT_PROMPT_PATH=specs/0003-system-prompt.md
   AGENT_TIMEOUT_SECS=1800
   MAX_CONCURRENT_REQUESTS=10

   # 日志
   RUST_LOG=info
   ```

4. **构建并运行**
   ```bash
   cargo build --release
   cargo run --release
   ```

## 使用

### 初始设置（每个频道）

1. **邀请机器人**到 Slack 频道：
   ```
   /invite @SlackCoderBot
   ```

2. **提供仓库**，在提示时输入：
   ```
   tyrchen/rust-lib-template
   ```

3. **等待设置**（通常需要 1-2 分钟）：
   ```
   进度：
   ✅ 验证仓库访问
   ✅ 克隆仓库到工作区
   ⏳ 分析代码库
   ⬜ 生成系统提示词
   ⬜ 保存系统提示词到磁盘
   ```

4. **开始编码**，当你看到：
   ```
   ✅ 仓库 `tyrchen/rust-lib-template` 现已准备就绪！

   你现在可以要求我生成代码、编写文档，
   或使用 `/help` 等命令。
   ```

### 日常使用

**生成代码：**
```
@SlackCoderBot 为用户认证添加新的 API 端点
```

**编写文档：**
```
@SlackCoderBot 为认证模块编写文档
```

**重构代码：**
```
@SlackCoderBot 重构用户服务以使用 async/await
```

**修复错误：**
```
@SlackCoderBot 修复 api/user.rs 第 42 行的空指针错误
```

**使用斜杠命令：**
```
@SlackCoderBot /help
@SlackCoderBot /new-session
```

### 功能演示

**进度跟踪：**
所有操作都显示实时进度：
```
进度: 2/4
当前: 生成代码

✅ 审查现有 API 结构
✅ 设计用户资料端点
⏳ 实现端点处理器
⬜ 添加测试
```

**上下文感知响应：**
机器人从你的代码库学习并生成符合以下内容的代码：
- 编码风格和约定
- 架构模式
- 测试框架
- 文档标准
- 命名约定

**线程支持：**
在线程中继续对话以获得更好的组织。

## 目录结构

设置后，你的工作区将如下所示：

```
~/.slack_coder/
├── repos/
│   ├── C12345ABC/              # 频道 ID
│   │   ├── .git/
│   │   ├── src/
│   │   └── ...                 # 完整仓库克隆
│   └── C67890DEF/
│       └── ...
└── system/
    ├── C12345ABC/
    │   └── system_prompt.md    # 仓库特定指令
    └── C67890DEF/
        └── system_prompt.md
```

## 开发

### 运行测试

```bash
cargo test
```

### 代码检查

```bash
cargo clippy --all-targets --all-features
```

### 生产构建

```bash
cargo build --release
```

### Docker 部署

```bash
docker build -t slack-coder .
docker run -d \
  --name slack-coder \
  --env-file .env \
  -v ~/.slack_coder:/root/.slack_coder \
  slack-coder
```

## 故障排除

### 机器人不响应

**检查 Socket Mode 连接：**
```bash
# 在日志中查找：
# "Event handler starting..."
# "Listening for Slack events..."
```

**验证令牌：**
```bash
# 检查 SLACK_APP_TOKEN 是否有效
# 检查 SLACK_BOT_TOKEN 是否有效
```

### 仓库设置失败

**检查 GitHub 认证：**
```bash
gh auth status
# 应显示: Logged in to github.com as <username>
```

**检查仓库访问：**
```bash
gh repo view owner/repo-name
# 应显示仓库详情
```

**检查磁盘空间：**
```bash
df -h ~/.slack_coder
# 确保有足够的空间存储仓库
```

### Agent 不响应

**检查 agent 状态：**
```bash
# 查找日志：
# "Agent restored for channel C12345"
# "Processing message from U123 in channel C12345"
```

**检查系统提示词是否存在：**
```bash
ls -la ~/.slack_coder/system/C12345/system_prompt.md
cat ~/.slack_coder/system/C12345/system_prompt.md
```

**重启机器人：**
```bash
# 终止并重启 - agent 将在启动时恢复
```

## 配置参考

### 环境变量

| 变量 | 必需 | 默认值 | 描述 |
|----------|----------|---------|-------------|
| `SLACK_BOT_TOKEN` | ✅ | - | Bot OAuth 令牌 (xoxb-...) |
| `SLACK_APP_TOKEN` | ✅ | - | 应用级令牌 (xapp-...) |
| `SLACK_SIGNING_SECRET` | ✅ | - | 用于验证的签名密钥 |
| `CLAUDE_API_KEY` | ✅ | - | Claude API 密钥 |
| `CLAUDE_MODEL` | ❌ | claude-sonnet-4 | 使用的 Claude 模型 |
| `CLAUDE_MAX_TOKENS` | ❌ | 8192 | 每次请求的最大令牌数 |
| `WORKSPACE_BASE_PATH` | ❌ | ~/.slack_coder | 仓库的基础目录 |
| `MAX_REPO_SIZE_MB` | ❌ | 1024 | 最大仓库大小 (MB) |
| `CLEANUP_INTERVAL_SECS` | ❌ | 3600 | Agent 清理间隔 |
| `MAIN_AGENT_PROMPT_PATH` | ❌ | specs/0003-system-prompt.md | 主 agent 提示词 |
| `AGENT_TIMEOUT_SECS` | ❌ | 1800 | 不活动 agent 超时 |
| `MAX_CONCURRENT_REQUESTS` | ❌ | 10 | 最大并发请求数 |
| `RUST_LOG` | ❌ | info | 日志级别 (trace, debug, info, warn, error) |

### Slack 所需权限

**Bot Token 作用域：**
- `app_mentions:read`
- `channels:history`
- `channels:read`
- `chat:write`
- `groups:history`
- `groups:read`
- `im:history`
- `im:read`
- `im:write`

**应用级令牌作用域：**
- `connections:write`（用于 Socket Mode）

## 工作原理

### 1. 机器人初始化

```mermaid
sequenceDiagram
    participant B as Bot
    participant S as Slack API
    participant W as Workspace
    participant A as Agent Manager

    B->>B: 从 .env 加载配置
    B->>W: 创建工作区目录
    B->>S: 通过 Socket Mode 连接
    B->>A: 创建 AgentManager
    B->>S: 列出所有频道

    loop 对于每个频道
        S-->>B: 频道 C12345
        B->>W: 检查是否已设置 (repos/C12345 存在)
        alt 已设置
            W-->>B: 找到
            B->>A: 从磁盘创建 RepoAgent
            B->>B: 添加到 agent 池
        else 未设置
            B->>B: 等待用户设置
        end
    end

    B->>S: 开始监听事件
```

### 2. 仓库设置（主 Agent）

主 agent 执行以下步骤：

1. **验证** - 使用 `gh repo view` 检查可访问性
2. **克隆** - 使用 `gh repo clone` 到 `~/.slack_coder/repos/{channel_id}/`
3. **分析** - 读取文件以了解：
   - 语言和框架
   - 代码约定和模式
   - 架构和设计
   - 测试方法
   - 文档风格
4. **生成提示词** - 创建仓库特定指令
5. **保存** - 写入 `~/.slack_coder/system/{channel_id}/system_prompt.md`

### 3. 代码生成（仓库 Agent）

每个频道都有一个专用 agent：

1. **加载**带有仓库知识的系统提示词
2. **设置工作目录**到仓库位置
3. **处理请求**，包含完整上下文
4. **执行操作**（读取、写入、git、gh）
5. **维护状态**，跨对话线程

### 4. 进度跟踪

使用 PostToolUse 钩子拦截 TodoWrite 调用：

```rust
// 当 agent 使用 TodoWrite 时：
{
  "todos": [
    {"content": "审查代码", "activeForm": "审查代码中", "status": "completed"},
    {"content": "生成端点", "activeForm": "生成端点中", "status": "in_progress"},
    {"content": "添加测试", "activeForm": "添加测试中", "status": "pending"}
  ]
}

// 钩子自动更新 Slack：
进度: 1/3
当前: 生成端点中

✅ 审查代码
⏳ 生成端点中
⬜ 添加测试
```

## 模块架构

```mermaid
graph TB
    subgraph "应用入口"
        MAIN["main.rs<br/>Bot 初始化"]
        LIB["lib.rs<br/>模块导出"]
    end

    subgraph "配置模块"
        CONF["config/settings.rs<br/>环境变量<br/>Settings 结构"]
    end

    subgraph "错误处理"
        ERR["error.rs<br/>SlackCoderError<br/>Result 类型"]
    end

    subgraph "Slack 模块"
        CLIENT["client.rs<br/>SlackClient<br/>API 包装器"]
        EVENTS["events.rs<br/>EventHandler<br/>Socket Mode 监听器"]
        MSGS["messages.rs<br/>MessageProcessor<br/>查询路由器"]
        FORMS["forms.rs<br/>FormHandler<br/>设置流程"]
        CMDS["commands.rs<br/>CommandHandler<br/>help 和 new-session"]
        PROG["progress.rs<br/>ProgressTracker<br/>TodoWrite 显示"]
        MDCONV["markdown.rs<br/>Markdown 转 Slack<br/>格式转换器"]
        TYPES["types.rs<br/>ChannelId UserId<br/>MessageTs ThreadTs"]
    end

    subgraph "Agent 模块"
        MGR["manager.rs<br/>AgentManager<br/>生命周期和池"]
        MAIN_AG["main_agent.rs<br/>MainAgent<br/>仓库设置"]
        REPO_AG["repo_agent.rs<br/>RepoAgent<br/>代码生成"]
        HOOKS["hooks.rs<br/>create_todo_hooks<br/>PostToolUse 处理器"]
        AG_TYPES["types.rs<br/>Plan Task<br/>TaskStatus"]
    end

    subgraph "存储模块"
        WS["workspace.rs<br/>Workspace<br/>路径管理器"]
    end

    subgraph "会话模块"
        SESS["session.rs<br/>SessionId<br/>generate_session_id"]
    end

    subgraph "外部依赖"
        CLAUDE["claude-agent-sdk-rs<br/>ClaudeClient<br/>ClaudeAgentOptions"]
        SLACK_M["slack-morphism<br/>Socket Mode<br/>Events API"]
        DASHMAP["dashmap<br/>DashMap<br/>并发 HashMap"]
    end

    MAIN --> CONF
    MAIN --> CLIENT
    MAIN --> EVENTS
    MAIN --> MGR
    MAIN --> WS
    MAIN --> PROG

    EVENTS --> MSGS
    EVENTS --> FORMS
    EVENTS --> TYPES

    MSGS --> CMDS
    MSGS --> MGR
    MSGS --> MDCONV

    FORMS --> MGR

    MGR --> MAIN_AG
    MGR --> REPO_AG
    MGR --> DASHMAP

    MAIN_AG --> HOOKS
    MAIN_AG --> CLAUDE
    MAIN_AG --> WS

    REPO_AG --> HOOKS
    REPO_AG --> CLAUDE
    REPO_AG --> WS
    REPO_AG --> SESS

    HOOKS --> AG_TYPES
    HOOKS --> PROG

    PROG --> CLIENT

    CLIENT --> SLACK_M
    EVENTS --> SLACK_M

    style MAIN fill:#e1f5ff
    style MGR fill:#ffe1f5
    style CLAUDE fill:#ffeb3b
    style SLACK_M fill:#4a9eff
```

## 项目结构

```
slack-coder/
├── Cargo.toml                      # 项目依赖和元数据
├── README.md                       # 英文版本
├── README_zh.md                    # 此文件
├── .env.example                    # 环境变量模板
│
├── src/
│   ├── main.rs                     # 应用入口点
│   │                               # - 初始化 tracing/logging
│   │                               # - 加载配置
│   │                               # - 创建 workspace、SlackClient
│   │                               # - 启动 EventHandler
│   │
│   ├── lib.rs                      # 公共模块导出
│   ├── error.rs                    # 错误类型 (SlackCoderError, Result)
│   │
│   ├── config/
│   │   ├── mod.rs
│   │   └── settings.rs             # 从 .env 加载配置
│   │                               # - SlackConfig, ClaudeConfig
│   │                               # - WorkspaceConfig, AgentConfig
│   │
│   ├── session.rs                  # 会话 ID 生成
│   │                               # - SessionId 类型
│   │                               # - generate_session_id()
│   │
│   ├── slack/                      # Slack 集成层
│   │   ├── mod.rs
│   │   ├── client.rs               # SlackClient - HTTP API 包装器
│   │   │                           # - send_message(), list_channels()
│   │   │                           # - update_message()
│   │   │
│   │   ├── events.rs               # EventHandler - Socket Mode 监听器
│   │   │                           # - handle_push_event()
│   │   │                           # - 事件去重
│   │   │                           # - 路由到 FormHandler/MessageProcessor
│   │   │
│   │   ├── forms.rs                # FormHandler - 仓库设置
│   │   │                           # - show_repo_setup_form()
│   │   │                           # - handle_repo_setup()
│   │   │
│   │   ├── messages.rs             # MessageProcessor - 消息路由
│   │   │                           # - process_message()
│   │   │                           # - forward_to_agent()
│   │   │                           # - 流式和格式化响应
│   │   │
│   │   ├── commands.rs             # CommandHandler - 斜杠命令
│   │   │                           # - /help, /new-session
│   │   │
│   │   ├── progress.rs             # ProgressTracker - TodoWrite 钩子显示
│   │   │                           # - update_progress()
│   │   │                           # - 格式化任务进度消息
│   │   │
│   │   ├── markdown.rs             # Markdown 到 Slack mrkdwn 转换器
│   │   │                           # - markdown_to_slack()
│   │   │
│   │   └── types.rs                # Slack 领域类型
│   │                               # - ChannelId, UserId, MessageTs, ThreadTs
│   │
│   ├── agent/                      # Claude agent 管理
│   │   ├── mod.rs
│   │   ├── manager.rs              # AgentManager - 生命周期管理
│   │   │                           # - setup_channel()
│   │   │                           # - get_repo_agent()
│   │   │                           # - DashMap<ChannelId, RepoAgent>
│   │   │
│   │   ├── main_agent.rs           # MainAgent - 仓库设置
│   │   │                           # - setup_repository()
│   │   │                           # - 验证、克隆、分析、生成提示词
│   │   │
│   │   ├── repo_agent.rs           # RepoAgent - 代码生成
│   │   │                           # - query(), receive_response()
│   │   │                           # - 会话管理
│   │   │                           # - 加载仓库特定系统提示词
│   │   │
│   │   ├── hooks.rs                # TodoWrite 钩子实现
│   │   │                           # - create_todo_hooks()
│   │   │                           # - PostToolUse 处理器
│   │   │                           # - 更新 Plan 和 ProgressTracker
│   │   │
│   │   └── types.rs                # Agent 领域类型
│   │                               # - Plan, Task, TaskStatus
│   │                               # - 时间跟踪
│   │
│   └── storage/
│       ├── mod.rs
│       └── workspace.rs            # Workspace - 文件系统路径
│                                   # - repo_path(), system_prompt_path()
│                                   # - load_system_prompt()
│
├── prompts/
│   ├── main-agent-system-prompt.md    # MainAgent 指令
│   └── repo-agent-workflow.md         # RepoAgent 工作流指令
│
├── specs/                          # 技术规范
│   ├── README.md
│   ├── 0001-slack-bot-spec.md
│   ├── 0002-slack-bot-design.md
│   ├── 0003-system-prompt.md
│   ├── 0004-initial-plan.md
│   ├── 0005-slack-new-session-command.md
│   └── instructions.md
│
├── docs/                           # 用户文档
│   ├── QUICK_START.md
│   ├── SLACK_SETUP.md
│   └── DEBUGGING.md
│
├── examples/
│   └── agent.rs                    # 简单 Claude agent 示例
│
└── vendors/                        # 供应商依赖
    ├── claude-agent-sdk-rs/
    └── slack-morphism-rust/
```

### 关键文件参考

| 文件 | 用途 | 关键导出 |
|------|---------|-------------|
| `src/main.rs` | 应用入口点 | `main()` |
| `src/slack/events.rs` | Socket Mode 事件处理 | `EventHandler`, `handle_push_event()` |
| `src/slack/messages.rs` | 消息处理 | `MessageProcessor`, `process_message()` |
| `src/agent/manager.rs` | Agent 生命周期 | `AgentManager`, `setup_channel()` |
| `src/agent/repo_agent.rs` | 代码生成 agent | `RepoAgent`, `query()`, `start_new_session()` |
| `src/agent/hooks.rs` | TodoWrite 钩子 | `create_todo_hooks()` |
| `src/slack/progress.rs` | 进度显示 | `ProgressTracker`, `update_progress()` |
| `src/storage/workspace.rs` | 文件路径 | `Workspace`, 路径助手 |
| `src/session.rs` | 会话 ID | `SessionId`, `generate_session_id()` |

## 高级使用

### 多频道

每个频道维护自己的仓库：

```
#project-alpha → tyrchen/project-alpha
#project-beta  → tyrchen/project-beta
#team-shared   → company/shared-lib
```

Agent 完全隔离 - 没有跨频道数据泄漏。

### Agent 清理

不活动的 agent 会在超时后自动清理（默认：30 分钟）。

### 自定义系统提示词

你可以手动编辑系统提示词：

```bash
# 编辑生成的提示词
vim ~/.slack_coder/system/C12345/system_prompt.md

# 重启机器人以重新加载（或等待下次 agent 创建）
```

## 贡献

欢迎贡献！请：

1. Fork 仓库
2. 创建功能分支
3. 进行更改
4. 运行测试：`cargo test`
5. 运行 clippy：`cargo clippy --all-targets --all-features`
6. 提交 pull request

## 许可证

本项目根据 MIT 条款分发。

详见 [LICENSE](LICENSE.md)。

版权所有 2025 Tyr Chen

## 相关项目

- [claude-agent-sdk-rs](https://github.com/anthropics/claude-agent-sdk-rs) - Rust 版 Claude Agent SDK
- [slack-morphism](https://github.com/abdolence/slack-morphism-rust) - Rust 版 Slack API 客户端

## 支持

有关问题和疑问：
- GitHub Issues: https://github.com/tyrchen/slack-coder/issues
- 文档：详见 `specs/` 目录获取详细规范
