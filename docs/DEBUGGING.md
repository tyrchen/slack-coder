# Debugging Guide

## Understanding the Logs

The bot now has comprehensive debug logging with emojis for easy scanning:

### Startup Sequence

When the bot starts successfully, you should see:

```
🚀 Starting Slack Coder Bot
✅ Configuration loaded
✅ Workspace initialized at "~/.slack_coder"
✅ Slack client created
✅ Agent manager created
🔍 Scanning Slack channels for existing setups...
📋 Fetching channel list from Slack API...
Found 3 channels where bot is a member
  - C12345ABC
  - C67890DEF
  - C11111GHI
📊 Total channels to check: 3
  Channel C12345ABC not setup yet
  Channel C67890DEF not setup yet
  Channel C11111GHI not setup yet
📈 Restored 0 agents from disk
✅ Channels scanned and agents restored
🎧 Starting event handler (Socket Mode)...
🔧 Initializing event handler components...
🔌 Connecting to Slack via Socket Mode...
✅ Connected! Listening for Slack events...
📱 Bot is ready to receive messages. Invite it to a channel and @mention it!
```

### When Bot is Mentioned

When someone @mentions the bot, you should see:

```
📨 Received push event: AppMention(...)
🔔 App mentioned in channel: C12345ABC by user: U98765XYZ
📝 Original text: '<@U00BOTID> tyrchen/rust-lib-template'
🧹 Cleaned text: 'tyrchen/rust-lib-template'
🔧 Detected setup request: tyrchen/rust-lib-template
🔧 Starting repository setup
  Channel: C12345ABC
  Repository: tyrchen/rust-lib-template
✅ Validated format: owner=tyrchen, repo=rust-lib-template
✅ Acknowledgment sent
🚀 Invoking agent manager to setup channel...
🎬 Setting up channel C12345ABC with repository tyrchen/rust-lib-template
✅ Main agent created
🔗 Connecting main agent to Claude...
✅ Connected to Claude
🚀 Running repository setup (this may take 1-2 minutes)...
[TodoWrite hook updates appear here]
✅ Repository setup completed
🤖 Creating repository-specific agent...
✅ Repository agent created and cached
✅ Agent setup completed
🎉 Setup workflow completed successfully
```

## Common Issues & Solutions

### Issue 1: "Nothing Happened" After Inviting Bot

**Symptoms:**
- Bot invited to channel
- No response when mentioned
- Logs show bot is running

**Debug Steps:**

1. **Check if bot received the event:**
   ```bash
   # Look in logs for:
   📨 Received push event: AppMention
   ```
   - **If you see this**: Event was received, continue to step 2
   - **If you don't see this**: Problem is with Slack configuration

2. **If no event received, check Event Subscriptions:**
   - Go to https://api.slack.com/apps → Your App → Event Subscriptions
   - Ensure `app_mention` is subscribed under "Subscribe to bot events"
   - Click "Save Changes" if you made changes
   - **Reinstall the app** to workspace

3. **Check Socket Mode connection:**
   ```bash
   # Look for:
   ✅ Connected! Listening for Slack events...
   ```
   - **If you see this**: Socket Mode is working
   - **If error**: Check `SLACK_APP_TOKEN` is correct

4. **Check bot mention format:**
   ```
   # Correct:
   @slack-coder tyrchen/rust-lib-template

   # Wrong:
   slack-coder tyrchen/rust-lib-template   # Missing @
   Hey @slack-coder setup repo             # Not recognized pattern
   ```

5. **Check you're mentioning the right bot:**
   - Type `@` in Slack and look for your bot name
   - It should autocomplete
   - The name must match what you configured

### Issue 2: Bot Connects but Doesn't Process Events

**Debug with these log markers:**

```bash
# Expected flow:
📨 Received push event       # ✅ Event received
🔔 App mentioned             # ✅ Parsed as app mention
📝 Original text             # ✅ Extracted text
🧹 Cleaned text              # ✅ Stripped bot mention
🔧 Detected setup request    # ✅ Recognized as setup
```

**If flow stops at any point:**

- **Stops at "Received push event"**: Event type not matched
  - Check what event type was received
  - May need to handle different event types

- **Stops at "App mentioned"**: Event parsing failed
  - Check Slack API version compatibility
  - Verify event structure matches expectations

- **Stops at "Cleaned text"**: Text extraction failed
  - Check if `mention.content.text` exists
  - Try logging the full mention object

### Issue 3: Setup Fails

**Look for these log patterns:**

```bash
🔧 Starting repository setup
✅ Validated format
✅ Acknowledgment sent
🚀 Invoking agent manager
# ... should continue with main agent creation
```

**If setup fails:**

1. **Check CLAUDE_API_KEY:**
   ```bash
   echo $CLAUDE_API_KEY
   # Should start with sk-ant-api03-...
   ```

2. **Check main agent prompt file exists:**
   ```bash
   ls -la specs/0003-system-prompt.md
   # Should exist
   ```

3. **Check gh CLI is authenticated:**
   ```bash
   gh auth status
   # Should show: ✓ Logged in to github.com
   ```

4. **Check workspace is writable:**
   ```bash
   mkdir -p ~/.slack_coder/test
   echo "test" > ~/.slack_coder/test/file
   # Should succeed
   ```

### Issue 4: Events Not Reaching Bot

**Verify Event Subscriptions:**

Your Slack app MUST have these subscribed:

```
Event Subscriptions → Enable Events: ON
Subscribe to bot events:
  ✅ app_mention
  ✅ message.channels
  ✅ message.groups
  ✅ message.im
```

**After changing subscriptions:**
1. Click "Save Changes"
2. May need to reinstall app
3. **Restart the bot** (important!)

### Issue 5: Permission Errors

**If you see "missing_scope" error:**

```
Error: SlackApi("missing_scope\nneeded:groups:read")
```

**Fix:**
1. Go to OAuth & Permissions
2. Add the missing scope (e.g., `groups:read`)
3. **Reinstall App to Workspace**
4. **Update SLACK_BOT_TOKEN** (it changes after reinstall!)
5. Restart bot

**Required scopes checklist:**
```
✅ app_mentions:read
✅ channels:history
✅ channels:read
✅ chat:write
✅ groups:history
✅ groups:read
✅ im:history
✅ im:read
✅ im:write
```

## Debug Logging Levels

### Default (Info Level)

```bash
cargo run
# or
RUST_LOG=info cargo run
```

Shows: ✅ ❌ 🚀 🔧 📨 🔔 💬 (main events only)

### Debug Level (Recommended for troubleshooting)

```bash
RUST_LOG=debug cargo run
```

Shows: Everything above + detailed flow + API calls

### Trace Level (Very verbose)

```bash
RUST_LOG=trace cargo run
```

Shows: Everything including Slack API internals

### Module-Specific Logging

```bash
# Only slack-coder logs
RUST_LOG=slack_coder=debug cargo run

# Only event handling
RUST_LOG=slack_coder::slack::events=debug cargo run

# Only agent operations
RUST_LOG=slack_coder::agent=debug cargo run

# Multiple modules
RUST_LOG=slack_coder::slack=debug,slack_coder::agent=info cargo run
```

## Testing Checklist

Use this checklist to systematically diagnose issues:

### Pre-Start Checks

- [ ] `.env` file exists
- [ ] All required env vars are set (SLACK_BOT_TOKEN, SLACK_APP_TOKEN, CLAUDE_API_KEY)
- [ ] Tokens don't have extra spaces or quotes
- [ ] `gh auth status` shows logged in
- [ ] Rust version is 2024 edition (`rustc --version`)

### Startup Checks

Run with debug logging and verify:

- [ ] ✅ Configuration loaded
- [ ] ✅ Workspace initialized
- [ ] ✅ Slack client created
- [ ] ✅ Agent manager created
- [ ] 🔍 Scanning channels... (should list channels)
- [ ] ✅ Connected! Listening for events

### Slack App Checks

In https://api.slack.com/apps → Your App:

- [ ] Socket Mode: Enabled
- [ ] App-level token exists with `connections:write`
- [ ] OAuth & Permissions: All 9 scopes added
- [ ] App installed to workspace
- [ ] Event Subscriptions: Enabled with 4 events
- [ ] App is not suspended or restricted

### Channel Checks

In your Slack workspace:

- [ ] Bot appears in Apps section
- [ ] Bot shows as "Active" (green dot)
- [ ] Bot was invited to the channel (`/invite @bot-name`)
- [ ] You're using @mention (not just typing the name)
- [ ] Message format is correct for setup: `@bot owner/repo`

## Quick Diagnostic Commands

### Check Bot Health

```bash
# Bot should be running
ps aux | grep slack-coder

# Check it's listening
lsof -i -P | grep slack-coder
```

### Check Workspace

```bash
# Workspace should exist
ls -la ~/.slack_coder/

# Check structure
tree -L 2 ~/.slack_coder/
# Should show repos/ and system/ directories
```

### Test Slack API Connection

```bash
# Test bot token
curl -X POST https://slack.com/api/auth.test \
  -H "Authorization: Bearer $SLACK_BOT_TOKEN"

# Should return: "ok": true, "user_id": "U00BOTID"
```

### Check Event Flow

Watch logs in real-time:

```bash
# Terminal 1: Run bot with debug logs
RUST_LOG=debug cargo run

# Terminal 2: Tail system logs (if using systemd)
journalctl -u slack-coder -f

# In Slack: @mention the bot
# Watch Terminal 1 for events flowing through
```

## Example Debug Session

Here's what a successful interaction looks like in logs:

```
# Bot starts
🚀 Starting Slack Coder Bot
✅ Configuration loaded
✅ Workspace initialized at "~/.slack_coder"
✅ Slack client created
✅ Agent manager created
🔍 Scanning Slack channels for existing setups...
📋 Fetching channel list from Slack API...
Received 1 total channels
  Channel: C07V58FQVPH (member: true)
Found 1 channels where bot is a member
  - C07V58FQVPH
📊 Total channels to check: 1
  Channel C07V58FQVPH not setup yet
📈 Restored 0 agents from disk
✅ Channels scanned and agents restored
🎧 Starting event handler (Socket Mode)...
🔧 Initializing event handler components...
🔌 Connecting to Slack via Socket Mode...
✅ Connected! Listening for Slack events...
📱 Bot is ready to receive messages. Invite it to a channel and @mention it!

# User mentions bot with: @slack-coder tyrchen/rust-lib-template
📨 Received push event: AppMention(...)
🔔 App mentioned in channel: C07V58FQVPH by user: U07UZE8R8SN
📝 Original text: '<@U07V9K2M7JE> tyrchen/rust-lib-template'
🧹 Cleaned text: 'tyrchen/rust-lib-template'
🔧 Detected setup request: tyrchen/rust-lib-template
🔧 Starting repository setup
  Channel: C07V58FQVPH
  Repository: tyrchen/rust-lib-template
✅ Validated format: owner=tyrchen, repo=rust-lib-template
✅ Acknowledgment sent
🚀 Invoking agent manager to setup channel...
🎬 Setting up channel C07V58FQVPH with repository tyrchen/rust-lib-template
✅ Main agent created
🔗 Connecting main agent to Claude...
✅ Connected to Claude
🚀 Running repository setup (this may take 1-2 minutes)...
[Progress updates via TodoWrite hook]
✅ Repository setup completed
🤖 Creating repository-specific agent...
✅ Repository agent created and cached for channel C07V58FQVPH
✅ Agent setup completed
🎉 Setup workflow completed successfully
```

## Still Not Working?

If you've tried everything above:

1. **Capture full logs:**
   ```bash
   RUST_LOG=debug cargo run 2>&1 | tee bot-debug.log
   ```

2. **In Slack, try:**
   - Creating a new channel
   - Inviting bot with `/invite @slack-coder`
   - Mentioning: `@slack-coder test`
   - Check bot-debug.log for the "📨 Received push event" line

3. **Check Slack App Status:**
   - https://api.slack.com/apps → Your App
   - Look for any warnings or errors at the top
   - Check "Install App" shows as installed (not pending)

4. **Try reinstalling:**
   - OAuth & Permissions → "Reinstall App"
   - Update `SLACK_BOT_TOKEN` with new token
   - Restart bot

5. **Open an issue:**
   - Include the bot-debug.log
   - Include your Slack app configuration (scopes, events)
   - Include the exact steps you're taking
