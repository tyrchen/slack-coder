# Instructions

## basic idea

build a slack bot that can listen to messages being sent to channel, and interact with the claude agent sdk (already included in the crate) to generate code based on the instructions provided.

When the bot is invited to a channel, it shall use a form to collect the following information:

- repo name: e.g. tyrchen/rust-lib-template

Then the bot shall pass this information to the mainclaude agent to use `gh` to see if the repo could be cloned, if not bail out the execution and send a message to the user to create the repo manually.
For cloned repo, the main claude agent shall look into the repo, review the code carefully, and generate a system prompt for this repo based on the learnings from the codebase. The system prompt shall be put to REPO/.slack_coder/sytem_prompt.md. And should include all the information that could help to generate code/docs for this repo well. Then it shall start a repo based claude agent to handle the code generation/docs generation requests for that channel/repo (should use a dashmap to store channel id: repo based claude agent mapping).

Once it is ready, the bot shall send a message to the channel to inform the user that the repo is ready to be used.

Then user can send message to the bot to generate code/docs, and the bot shall use claude agent to generate the code/docs based on the instructions provided. All the slash commands claude agent sdk support shall be supported by the bot. user could use them and get result.

Please think seriously about this idea and generate these specs:

- ./specs/0001-slack-bot-spec.md: the detailed requirements for this slack bot.
- ./specs/0002-slack-bot-design.md: the design and implementation plan using rust. Do not include detailed code, just high level file structure, deps, struct, trait, public fn signature, etc.
- ./specs/0003-system-prompt.md: the system prompt for the main claude agent. It shall include the details on how to clone repo, how to review the codebase, and how to generate system prompt for this repo (structure of the system prompt for repo based claude agent).

## improve doc

a few improvement:

  1. detect and clone repo shall be handled by main claude agent. slack bot just deliver the repo name to the agent with user prompts. Thus interaction with gh shall all be done by claude agent.
  2. when interactive with claude agent, shall use PostToolUse hook for TodoWrite to extract todo list
  and todo list update, this info shall be sent to user in slack and updated accordingly.
  3. claude agent shall do all the heavy lifting, the app should mostly do the work of interactive with
  slack and agents to pass messages.
  4. upon initialization, bot shall scan all channels available to it (being invited), and start the
  relevant agent
  5. folder shall be ~/.slack_coder/repos/<channel_id>. Thus bot can find the right repo.
  6. system prompt shall be ~/.slack_coder/system/<channel_id>/system_prompt.md
  7. for design doc, the main claude agent is not managed by dashmap of repo agents.

please revise docs accordingly.

## implement plan

look at the usage of @examples/agent.rs and update @specs/0002-slack-bot-design.md accordingly. Then put an impl plan on ./specs/0004-initial-plan.md

## phase 1 and 2

implement phase 1.

make sure you use slack-morphism 2, refer @vendors/slack-morphism-rust/examples/socket_mode.rs for socket mode usage and @vendors/slack-morphism-rust/ for detailed usage. Now go to phase 2.

## invite to channel bug

invited to channel but no form is shown, no interaction with the bot.

2025-10-26T18:19:58.279183Z  INFO slack_coder::slack::events: 78: 📱 Bot is ready to receive messages. Invite it to a channel and @mention it!
2025-10-26T18:19:58.279196Z DEBUG slack_morphism::socket_mode::listener: 65: Starting all WSS clients
2025-10-26T18:19:58.279561Z DEBUG Slack API request{/slack/team_id="-"}: slack_morphism::hyper_tokio::connector: 117: Sending HTTP request to <https://slack.com/api/apps.connections.open> slack_uri="<https://slack.com/api/apps.connections.open>"
2025-10-26T18:19:58.388631Z DEBUG Slack API request{/slack/team_id="-"}: slack_morphism::hyper_tokio::connector: 134: Received HTTP response 200 OK slack_uri="<https://slack.com/api/apps.connections.open>" slack_http_status=200
2025-10-26T18:19:58.843552Z DEBUG slack_morphism::hyper_tokio::socket_mode::tungstenite_wss_client: 134: [0/0/0] Connected to wss://wss-primary.slack.com/link/?ticket=68ebdb63-b075-4450-8bf3-f8b4da40409e&app_id=70427329c4b6caccd454234579ae85fa3a5a92e8e8edff2ff9bf8ad95242e250 slack_wss_client_id="0/0/0"
2025-10-26T18:19:58.855947Z DEBUG slack_morphism::socket_mode::callbacks: 100: Received Slack hello for socket mode: SlackSocketModeHelloEvent { connection_info: SlackSocketModeConnectionInfo { app_id: SlackAppId("A09NK3EL5PD") }, num_connections: 1, debug_info: SlackSocketModeDebugInfo { host: "applink-7", started: None, build_number: Some(3), approximate_connection_time: Some(18060) } }
2025-10-26T18:20:03.281673Z DEBUG Slack API request{/slack/team_id="-"}: slack_morphism::hyper_tokio::connector: 117: Sending HTTP request to <https://slack.com/api/apps.connections.open> slack_uri="<https://slack.com/api/apps.connections.open>"
2025-10-26T18:20:03.395916Z DEBUG Slack API request{/slack/team_id="-"}: slack_morphism::hyper_tokio::connector: 134: Received HTTP response 200 OK slack_uri="<https://slack.com/api/apps.connections.open>" slack_http_status=200
2025-10-26T18:20:03.806222Z DEBUG slack_morphism::hyper_tokio::socket_mode::tungstenite_wss_client: 134: [1/1/0] Connected to wss://wss-primary.slack.com/link/?ticket=9eea456d-512a-4db7-907c-6dd1da68c8a6&app_id=70427329c4b6caccd454234579ae85fa3a5a92e8e8edff2ff9bf8ad95242e250 slack_wss_client_id="1/1/0"
2025-10-26T18:20:03.817082Z DEBUG slack_morphism::socket_mode::callbacks: 100: Received Slack hello for socket mode: SlackSocketModeHelloEvent { connection_info: SlackSocketModeConnectionInfo { app_id: SlackAppId("A09NK3EL5PD") }, num_connections: 2, debug_info: SlackSocketModeDebugInfo { host: "applink-3", started: None, build_number: Some(3), approximate_connection_time: Some(18060) } }
2025-10-26T18:20:17.873458Z  INFO slack_coder::slack::events: 90: 📨 Received push event: Message(SlackMessageEvent { origin: SlackMessageOrigin { ts: SlackTs("1761502816.706259"), channel: Some(SlackChannelId("C09NRMS2A58")), channel_type: Some(SlackChannelType("channel")), thread_ts: None, client_msg_id: None }, content: Some(SlackMessageContent { text: Some("<@U09NPJCJXU6> has joined the channel"), blocks: None, attachments: None, upload: None, files: None, reactions: None, metadata: None }), sender: SlackMessageSender { user: Some(SlackUserId("U09NPJCJXU6")), bot_id: None, username: None, display_as_bot: None, user_profile: None, bot_profile: None }, subtype: Some(ChannelJoin), hidden: None, message: None, previous_message: None, deleted_ts: None })
2025-10-26T18:20:17.873635Z  INFO slack_coder::slack::events: 158: Message received: SlackMessageEvent { origin: SlackMessageOrigin { ts: SlackTs("1761502816.706259"), channel: Some(SlackChannelId("C09NRMS2A58")), channel_type: Some(SlackChannelType("channel")), thread_ts: None, client_msg_id: None }, content: Some(SlackMessageContent { text: Some("<@U09NPJCJXU6> has joined the channel"), blocks: None, attachments: None, upload: None, files: None, reactions: None, metadata: None }), sender: SlackMessageSender { user: Some(SlackUserId("U09NPJCJXU6")), bot_id: None, username: None, display_as_bot: None, user_profile: None, bot_profile: None }, subtype: Some(ChannelJoin), hidden: None, message: None, previous_message: None, deleted_ts: None }

## slack message improvement

Got the following messages, looks good but can be improved. 1) use animated emoji for progress. 2) Output the result message in a well formatted way (its markdown). 3) once the main agent is done, start the repo based agent and it should handle the following requests for the channel/repo.

```
slack-coder
APP  11:24 AM
:wrench: Setting up repository tyrchen/http-tunnel...
This may take a minute. I'll update you on progress.
11:24
:wrench: Setting up repository tyrchen/http-tunnel...
This may take a minute. I'll update you on progress.
11:24
Progress:
:white_check_mark: Validate repository access
:white_check_mark: Clone repository to workspace
:white_check_mark: Analyze codebase structure
:white_check_mark: Generate system prompt
:white_check_mark: Save system prompt to disk (edited)
11:29
:white_check_mark: Repository tyrchen/http-tunnel is now ready!
You can now ask me to generate code, write documentation, or use commands like /help.
```
