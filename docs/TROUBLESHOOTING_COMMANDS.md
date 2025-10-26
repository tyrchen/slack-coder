# Quick Troubleshooting Commands

## Run with Full Debug Logging

```bash
RUST_LOG=debug cargo run
```

## Check What the Bot Sees

### When bot starts, you should see:
```
🚀 Starting Slack Coder Bot
✅ Configuration loaded
✅ Workspace initialized
✅ Slack client created
✅ Agent manager created
🔍 Scanning Slack channels...
Found X channels where bot is a member
✅ Connected! Listening for Slack events...
📱 Bot is ready to receive messages
```

### When you @mention the bot:
```
📨 Received push event: AppMention
🔔 App mentioned in channel: C07V58FQVPH by user: U07UZE8R8SN
📝 Original text: '<@U07V9K2M7JE> tyrchen/rust-lib-template'
🧹 Cleaned text: 'tyrchen/rust-lib-template'
```

## If Nothing Shows Up When You @Mention Bot

### 1. Check Event Subscriptions

Go to: https://api.slack.com/apps → Your App → Event Subscriptions

Must have:
- ✅ Enable Events: ON
- ✅ `app_mention` subscribed
- ✅ "Save Changes" clicked

### 2. Check Socket Mode

Go to: Socket Mode

Must have:
- ✅ Enable Socket Mode: ON
- ✅ App-level token created with `connections:write`

### 3. Check Bot is Invited

In Slack channel:
```
/invite @slack-coder
```

You should see: "added an integration to this channel: slack-coder"

### 4. Test @Mention Format

Try exactly this:
```
@slack-coder test
```

Look for logs:
```
📨 Received push event
```

If you see this, events are working!

## If Bot Received Event But Doesn't Respond

Check logs for where it stops:

```
📨 Received push event       ← Got here? Event received ✅
🔔 App mentioned             ← Got here? Parsed correctly ✅
📝 Original text             ← Got here? Text extracted ✅
🧹 Cleaned text              ← Got here? Processed ✅
```

If it stops early, there may be a parsing issue.

## Common Quick Fixes

### Fix 1: Reinstall App

1. OAuth & Permissions → Reinstall App
2. Copy new SLACK_BOT_TOKEN
3. Update .env
4. Restart bot

### Fix 2: Reset Socket Mode

1. Socket Mode → Delete existing token
2. Create new token with `connections:write`
3. Copy new SLACK_APP_TOKEN
4. Update .env
5. Restart bot

### Fix 3: Verify Bot User ID

The bot mention looks like `<@U07V9K2M7JE>`. To find your bot's ID:

```bash
# Run this with your bot token
curl -X POST https://slack.com/api/auth.test \
  -H "Authorization: Bearer $SLACK_BOT_TOKEN" \
  | jq '.user_id'
```

## Verify Everything is Working

Run this test sequence:

```bash
# 1. Start bot with debug logs
RUST_LOG=debug cargo run

# 2. In another terminal, verify it's running
ps aux | grep slack-coder

# 3. In Slack:
#    - Go to channel where bot is invited
#    - Type: @slack-coder hello
#    - You should immediately see in logs:
#      📨 Received push event

# 4. If you see the push event, the integration works!
#    Now try setup:
#    @slack-coder your-username/your-repo
```

## Get Help

If still stuck:

1. Capture logs: `RUST_LOG=debug cargo run 2>&1 | tee debug.log`
2. Open issue at: https://github.com/tyrchen/slack-coder/issues
3. Include:
   - debug.log
   - Screenshot of Event Subscriptions page
   - Screenshot of OAuth & Permissions scopes
   - Exact steps you're taking
