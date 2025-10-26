# Slack App Setup Guide

This guide will walk you through creating a Slack app and obtaining all necessary tokens for the Slack Coder Bot.

## Prerequisites

- Admin access to a Slack workspace (or permission to install apps)
- Web browser

## Step-by-Step Setup

### Step 1: Create a Slack App

1. Go to https://api.slack.com/apps
2. Click **"Create New App"**
3. Select **"From scratch"**
4. Fill in the details:
   - **App Name**: `Slack Coder Bot` (or your preferred name)
   - **Pick a workspace**: Select your workspace
5. Click **"Create App"**

You'll be redirected to the app's Basic Information page.

### Step 2: Enable Socket Mode

Socket Mode allows the bot to receive events via WebSocket instead of requiring a public HTTP endpoint.

1. In the left sidebar, click **"Socket Mode"**
2. Toggle **"Enable Socket Mode"** to ON
3. You'll be prompted to create an app-level token:
   - **Token Name**: `socket-token` (or any name)
   - **Scope**: Select `connections:write`
   - Click **"Generate"**
4. **Copy the token** - it starts with `xapp-`
   - ⚠️ **Save this as `SLACK_APP_TOKEN`** - you won't see it again!
   - Example: `xapp-1-A01234567-1234567890123-abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890`

### Step 3: Configure Bot Token Scopes

These permissions control what your bot can do.

1. In the left sidebar, click **"OAuth & Permissions"**
2. Scroll down to **"Scopes"** → **"Bot Token Scopes"**
3. Click **"Add an OAuth Scope"** and add each of these:

   **Essential Scopes:**
   - `app_mentions:read` - Detect when bot is @mentioned
   - `channels:history` - Read messages in public channels
   - `channels:read` - View basic channel information
   - `chat:write` - Send messages
   - `groups:history` - Read messages in private channels
   - `groups:read` - View private channel information
   - `im:history` - Read direct messages
   - `im:read` - View DM information
   - `im:write` - Send direct messages

   **Why each scope is needed:**
   - `app_mentions:read` - Bot needs to know when users @mention it
   - `channels:*` - Bot needs to read messages and channel info
   - `groups:*` - Support for private channels
   - `im:*` - Support for direct messages
   - `chat:write` - Bot needs to send responses

4. Scroll back to the top of the page
5. Click **"Install to Workspace"** (or "Reinstall to Workspace" if updating)
6. Review permissions and click **"Allow"**
7. **Copy the "Bot User OAuth Token"** - it starts with `xoxb-`
   - ⚠️ **Save this as `SLACK_BOT_TOKEN`**
   - Example: `xoxb-1234567890-1234567890123-abcdefghijklmnopqrstuvwx`

### Step 4: Subscribe to Events

Tell Slack which events to send to your bot.

1. In the left sidebar, click **"Event Subscriptions"**
2. Toggle **"Enable Events"** to ON
3. Under **"Subscribe to bot events"**, click **"Add Bot User Event"**
4. Add these events:

   **Required Events:**
   - `app_mention` - When someone mentions @bot
   - `message.channels` - Messages posted in public channels
   - `message.groups` - Messages posted in private channels
   - `message.im` - Direct messages to the bot

5. Click **"Save Changes"** at the bottom

   **Note:** Slack may show a warning about "Request URL" - you can ignore this since we're using Socket Mode.

### Step 5: Get Signing Secret

Used to verify requests are actually from Slack.

1. In the left sidebar, click **"Basic Information"**
2. Scroll down to **"App Credentials"**
3. **Copy the "Signing Secret"** (click "Show" first)
   - ⚠️ **Save this as `SLACK_SIGNING_SECRET`**
   - Example: `a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6`

### Step 6: Configure Bot Display

Make your bot look nice in Slack.

1. Still in **"Basic Information"**
2. Scroll to **"Display Information"**
3. Set:
   - **App name**: `Slack Coder Bot`
   - **Short description**: `AI-powered code generation bot using Claude`
   - **App icon**: Upload an icon (optional)
   - **Background color**: Choose a color (optional)
4. Click **"Save Changes"**

### Step 7: Test in Slack

1. Open your Slack workspace
2. Find your bot in the Apps section (left sidebar)
3. The bot should appear as "Slack Coder Bot" (or your chosen name)
4. You can now invite it to channels!

## Summary of Tokens

You should now have three tokens:

| Token | Environment Variable | Starts With | Where to Find |
|-------|---------------------|-------------|---------------|
| Bot User OAuth Token | `SLACK_BOT_TOKEN` | `xoxb-` | OAuth & Permissions → "Bot User OAuth Token" |
| App-Level Token | `SLACK_APP_TOKEN` | `xapp-` | Socket Mode → App-Level Tokens |
| Signing Secret | `SLACK_SIGNING_SECRET` | (alphanumeric) | Basic Information → App Credentials |

## Configure Your Bot

Create a `.env` file in your project root:

```bash
# Copy the example
cp .env.example .env

# Edit with your tokens
nano .env  # or vim, or your favorite editor
```

Your `.env` should look like:

```env
# Slack Configuration
SLACK_BOT_TOKEN=xoxb-1234567890-1234567890123-abcdefghijklmnopqrstuvwx
SLACK_APP_TOKEN=xapp-1-A01234567-1234567890123-abcdef123456...
SLACK_SIGNING_SECRET=a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6

# Claude Configuration
CLAUDE_API_KEY=sk-ant-api03-...
CLAUDE_MODEL=claude-sonnet-4
CLAUDE_MAX_TOKENS=8192

# Workspace Configuration
WORKSPACE_BASE_PATH=~/.slack_coder
MAX_REPO_SIZE_MB=1024
CLEANUP_INTERVAL_SECS=3600

# Agent Configuration
MAIN_AGENT_PROMPT_PATH=specs/0003-system-prompt.md
AGENT_TIMEOUT_SECS=1800
MAX_CONCURRENT_REQUESTS=10

# Logging
RUST_LOG=info
```

## Testing Your Setup

### 1. Verify Tokens

Before running the bot, verify your tokens work:

```bash
# Test Bot Token
curl -X POST https://slack.com/api/auth.test \
  -H "Authorization: Bearer $SLACK_BOT_TOKEN"

# Should return: "ok": true
```

### 2. Start the Bot

```bash
cargo run --release
```

You should see:
```
Starting Slack Coder Bot
Configuration loaded
Workspace initialized at "~/.slack_coder"
Slack client created
Agent manager created
Scanning Slack channels...
Found 0 channels
Channels scanned and agents restored
Event handler starting...
```

### 3. Test in Slack

1. In Slack, create a test channel: `#bot-test`
2. Invite the bot: `/invite @Slack Coder Bot`
3. Mention the bot with a repo: `@Slack Coder Bot tyrchen/rust-lib-template`
4. You should see:
   ```
   Setting up repository `tyrchen/rust-lib-template`...
   This may take a minute.

   Progress:
   ⏳ Validate repository access
   ⬜ Clone repository to workspace
   ⬜ Analyze codebase
   ⬜ Generate system prompt
   ⬜ Save system prompt to disk
   ```

## Troubleshooting

### "Invalid Token" Error

**Problem:** Bot fails to start with authentication error

**Solution:**
1. Verify tokens are copied correctly (no extra spaces)
2. Check token hasn't been revoked
3. Reinstall app to workspace (OAuth & Permissions → Reinstall)
4. Generate new app-level token if needed

### Bot Doesn't Respond to Mentions

**Problem:** Bot is online but doesn't reply when mentioned

**Checklist:**
- ✅ Is `app_mention` event subscribed? (Event Subscriptions)
- ✅ Is Socket Mode enabled?
- ✅ Are bot token scopes correct?
- ✅ Is the bot invited to the channel? (`/invite @bot`)
- ✅ Check bot logs for errors

**Debug:**
```bash
# Run with debug logging
RUST_LOG=debug cargo run

# Look for:
# "Received push event: AppMention"
# "App mentioned in channel: C12345"
```

### "Missing Scopes" Error

**Problem:** Bot says it lacks permissions

**Solution:**
1. Go to OAuth & Permissions
2. Add the missing scope from the list above
3. Click **"Reinstall to Workspace"**
4. Click **"Allow"**
5. Copy the new `SLACK_BOT_TOKEN` (it will change!)
6. Update `.env` file
7. Restart the bot

### Socket Mode Connection Fails

**Problem:** Bot can't establish WebSocket connection

**Checklist:**
- ✅ Is Socket Mode enabled?
- ✅ Is `SLACK_APP_TOKEN` correct?
- ✅ Does app-level token have `connections:write` scope?
- ✅ Check firewall/network allows WebSocket connections

**Fix:**
1. Socket Mode → View app-level tokens
2. Regenerate token with `connections:write`
3. Update `.env` with new token
4. Restart bot

## Security Best Practices

### 1. Token Storage

**Never commit tokens to git:**
```bash
# Ensure .env is in .gitignore
echo ".env" >> .gitignore
```

**Use environment variables in production:**
```bash
# Set in your deployment environment
export SLACK_BOT_TOKEN=xoxb-...
export SLACK_APP_TOKEN=xapp-...
# etc.
```

### 2. Token Rotation

Rotate tokens periodically:
1. Generate new token in Slack app settings
2. Update `.env`
3. Restart bot
4. Revoke old token

### 3. Minimal Permissions

Only grant scopes the bot actually needs. Review the scope list above - don't add extra permissions.

## Advanced Configuration

### Using Secrets Manager

For production, use a secrets manager instead of `.env`:

```rust
// Example: AWS Secrets Manager
let secret = get_secret("slack-coder/prod").await?;
let slack_token = secret["SLACK_BOT_TOKEN"].to_string();
```

### Multiple Workspaces

To run the bot in multiple workspaces:

1. Install the app to each workspace
2. Each installation gets its own tokens
3. Run separate bot instances with different `.env` files
4. Or use workspace-specific configuration

### Rate Limits

Slack has rate limits:
- Tier 1: 1 request/minute
- Tier 2: 20 requests/minute
- Tier 3: 50 requests/minute
- Tier 4: 100 requests/minute

The bot handles these automatically with retries.

## Quick Reference

### Required Slack App Settings

```
✅ Socket Mode: Enabled
✅ App-Level Token: connections:write
✅ Bot Token Scopes:
   - app_mentions:read
   - channels:history
   - channels:read
   - chat:write
   - groups:history
   - groups:read
   - im:history
   - im:read
   - im:write
✅ Event Subscriptions: Enabled
✅ Bot Events:
   - app_mention
   - message.channels
   - message.groups
   - message.im
```

### Environment Variables Checklist

```bash
✅ SLACK_BOT_TOKEN=xoxb-...       # From OAuth & Permissions
✅ SLACK_APP_TOKEN=xapp-...       # From Socket Mode
✅ SLACK_SIGNING_SECRET=...       # From Basic Information
✅ CLAUDE_API_KEY=sk-ant-...      # From Claude Console
✅ WORKSPACE_BASE_PATH=~/.slack_coder
✅ MAIN_AGENT_PROMPT_PATH=specs/0003-system-prompt.md
✅ RUST_LOG=info
```

## Next Steps

After completing setup:

1. **Test the bot** in a test channel
2. **Invite to production channels** where you need code help
3. **Configure repositories** for each channel
4. **Start coding!** 🚀

For usage examples, see the main [README.md](../README.md).

For issues, check the [Troubleshooting](#troubleshooting) section above or open a GitHub issue.
