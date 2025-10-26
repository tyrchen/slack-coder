# Quick Start Guide

Get your Slack Coder Bot running in 15 minutes!

## Overview

```mermaid
graph LR
    A[Create Slack App] --> B[Get 3 Tokens]
    B --> C[Configure .env]
    C --> D[Run Bot]
    D --> E[Invite to Channel]
    E --> F[Setup Repository]
    F --> G[Start Coding!]

    style A fill:#e1f5ff
    style B fill:#ffe1f5
    style C fill:#f5ffe1
    style D fill:#fff5e1
    style E fill:#f5e1ff
    style F fill:#e1fff5
    style G fill:#90EE90
```

## Prerequisites Checklist

Before starting, ensure you have:

- [ ] Slack workspace admin access
- [ ] Claude API key ([Get one here](https://console.anthropic.com/))
- [ ] GitHub account with `gh` CLI authenticated
- [ ] Rust installed (2024 edition)

## 5-Minute Slack Setup

### 1. Create App (2 minutes)

1. Go to https://api.slack.com/apps → **Create New App** → **From scratch**
2. Name: `Slack Coder Bot`, select your workspace

### 2. Enable Socket Mode (1 minute)

1. Left sidebar → **Socket Mode** → Enable
2. Create token with `connections:write` scope
3. **Copy token** (starts with `xapp-`) → Save as `SLACK_APP_TOKEN`

### 3. Add Permissions (1 minute)

1. Left sidebar → **OAuth & Permissions**
2. Add these Bot Token Scopes (copy-paste friendly list):
   ```
   app_mentions:read
   channels:history
   channels:read
   chat:write
   groups:history
   groups:read
   im:history
   im:read
   im:write
   ```
3. Click **Install to Workspace** → **Allow**
4. **Copy Bot User OAuth Token** (starts with `xoxb-`) → Save as `SLACK_BOT_TOKEN`

### 4. Subscribe to Events (1 minute)

1. Left sidebar → **Event Subscriptions** → Enable
2. Add Bot User Events:
   ```
   app_mention
   message.channels
   message.groups
   message.im
   ```
3. **Save Changes**

### 5. Get Signing Secret (30 seconds)

1. Left sidebar → **Basic Information**
2. App Credentials → **Signing Secret** → Show
3. **Copy** → Save as `SLACK_SIGNING_SECRET`

## 5-Minute Bot Setup

### 1. Clone & Configure (2 minutes)

```bash
# Clone repository
git clone https://github.com/tyrchen/slack-coder
cd slack-coder

# Create .env file
cp .env.example .env
```

### 2. Add Your Tokens (2 minutes)

Edit `.env` and add your tokens:

```env
SLACK_BOT_TOKEN=xoxb-your-token-here
SLACK_APP_TOKEN=xapp-your-token-here
SLACK_SIGNING_SECRET=your-secret-here
CLAUDE_API_KEY=sk-ant-your-key-here
```

### 3. Build & Run (1 minute)

```bash
# Build
cargo build --release

# Run
cargo run --release
```

You should see:
```
✓ Starting Slack Coder Bot
✓ Configuration loaded
✓ Workspace initialized
✓ Slack client created
✓ Agent manager created
✓ Scanning Slack channels...
✓ Event handler starting...
```

## 5-Minute First Use

### 1. Create Test Channel (30 seconds)

In Slack:
```
Create channel: #bot-test
/invite @Slack Coder Bot
```

### 2. Setup Repository (30 seconds)

Mention the bot with a repository:
```
@Slack Coder Bot tyrchen/rust-lib-template
```

### 3. Watch Setup Progress (2-3 minutes)

You'll see real-time updates:
```
Setting up repository `tyrchen/rust-lib-template`...
This may take a minute.

Progress:
✅ Validate repository access
✅ Clone repository to workspace
⏳ Analyze codebase
⬜ Generate system prompt
⬜ Save system prompt to disk
```

Wait for:
```
✅ Repository `tyrchen/rust-lib-template` is now ready!

You can now ask me to generate code, write documentation,
or use commands like `/help`.
```

### 4. Generate Your First Code (1 minute)

Try this:
```
@Slack Coder Bot add a hello world function
```

You'll see:
```
Progress: 1/2
⏳ Review existing code patterns
⬜ Generate function

[Code generation happens]

Created `src/hello.rs`:
```rust
pub fn hello_world() -> String {
    "Hello, World!".to_string()
}
```

## Common First-Time Issues

### Issue: "SLACK_BOT_TOKEN not set"

**Fix:** Check `.env` file exists and has the token

```bash
# Verify .env exists
ls -la .env

# Check token is set
grep SLACK_BOT_TOKEN .env
```

### Issue: Bot shows as "away" in Slack

**Fix:** Bot process must be running

```bash
# Check if bot is running
ps aux | grep slack-coder

# If not running, start it
cargo run --release
```

### Issue: "Repository not found"

**Fix:** Ensure `gh` is authenticated

```bash
# Check auth status
gh auth status

# Should show: ✓ Logged in to github.com as username

# If not authenticated
gh auth login
```

### Issue: Bot joins channel but doesn't respond

**Checklist:**
- [ ] Did you @mention the bot? (Don't just type text)
- [ ] Are logs showing the event? (`RUST_LOG=debug cargo run`)
- [ ] Are all event subscriptions active?
- [ ] Is Socket Mode enabled?

## What's Next?

### Explore Features

Try these commands:
```
@Slack Coder Bot /help
@Slack Coder Bot /context
@Slack Coder Bot document the main module
@Slack Coder Bot add tests for user.rs
@Slack Coder Bot refactor auth to use async
```

### Setup Multiple Channels

Each channel can have its own repository:

```
#frontend → company/react-app
#backend  → company/api-server
#mobile   → company/mobile-app
```

Just invite the bot and provide the repo for each channel!

### Monitor Activity

Watch the logs to see what's happening:

```bash
# Detailed logging
RUST_LOG=debug cargo run --release

# Filter for specific events
RUST_LOG=slack_coder::agent=debug cargo run --release
```

### Check Workspace

See what's been created:

```bash
# List all configured channels
ls -la ~/.slack_coder/repos/

# View a system prompt
cat ~/.slack_coder/system/C12345ABC/system_prompt.md

# Check repository
cd ~/.slack_coder/repos/C12345ABC
git status
```

## Success! 🎉

You now have a fully functional AI coding assistant in Slack!

**Pro Tips:**
- Use threads to keep conversations organized
- Be specific in your requests for better results
- The bot learns from your codebase - it gets smarter over time
- You can edit system prompts to customize behavior

**Need Help?**
- Full documentation: [README.md](../README.md)
- Detailed Slack setup: [SLACK_SETUP.md](./SLACK_SETUP.md)
- Architecture details: [specs/](../specs/)
- GitHub Issues: https://github.com/tyrchen/slack-coder/issues
