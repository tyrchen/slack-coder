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

## bot startup

at startup, if bot is invited to the channel, and we can find the relevant repos/<channel_id>, the repo agent shall be initiated and pick up message from that channel

## improve visual feedback

this is a slack bot app, if you need to understand more on deps please see ./vendors. I want to improve the todo list format, please make them elegant - can it be check list? upon processing can you put animated emoji? can you add more visual feedback (e.g. a realtime updated timer) since the process
could be long? Think this through, document your solution and plan, and then execute. Below is current message for todo list:

```
Progress: 1 / 5
:white_check_mark: Validate repository access
:hourglass_flowing_sand: Cloning repository to workspace
:white_medium_square: Analyze codebase
:white_medium_square: Generate system prompt
:white_medium_square: Save system prompt to disk (edited)
```

## improve the system prompt

For @specs/0003-system-prompt.md, add these requirements for system prompt for repo based agent:

- if user asks for information, return as it
- if user asks for generating plans/docs, it should always start with a new branch, and once finished, send a pr against main/master branch. Then include the pr link in the response.
- if user asks for building a feature, it should always start with a new branch, and then analyze exissting code, think hard on user's input, depending how complex it is, either generate a combined requirements and implementation plan under ./specs/<seq_number>-<feature-name>.md, or generate a full fledged requirements under ./specs/<seq_number>-<feature-name>/spec.md, and then an implementation plan under ./specs/<seq_number>-<feature-name>/plan.md. Do not include code details other than high level interface. Then start coding. Do not create bolerplate or stub code, make sure each part is fully implemented and tested. And once finished, make sure code format, linting, and tests are all passed. Commit the code, push and send a pr against main/master branch. Then include the pr link in the response.

Once you finished, also update existing system prompts in ~/.slack_coder/system/<channel_id>/system_prompt.md accordingly.

## further improve the system prompt

For existing repos, please think thoroughly on how the local repo sync with remote repo. There are cases:

1. currently in a branch, now user wants to develop a new feature, what should be the best practice?
2. currently have uncommitted changes, what should be the best practice? Should we just drop it?
3. should we always pull the latest changes from remote repo before the work? should we always checkout to main/master branch before the work?
4. after the PR is submitted should we always checkout to main/master branch?

Think all these questions through, and revisit if @specs/0003-system-prompt.md accordingly has other situations to consider. Then split the system prompt into two parts:

- the main part of the system prompt for main agent that do its work and generate system prompt for repo based agent. (put in ./prompts/main-agent-system-prompt.md)
- common workflow for repo based agent. (new file ./prompts/repo-agent-workflow.md)

For main agent, it should use the system prompt in ./prompts/main-agent-system-prompt.md.

Then when loading the system prompt for repo based agent it should be the generated one in ~/.slack_coder/system/<channel_id>/system_prompt.md, and the common workflow should be in ./prompts/repo-agent-workflow.md.

Once you finished, also update existing system prompts in ~/.slack_coder/system/<channel_id>/system_prompt.md accordingly. Remove the common workflow from the generated one.

## slack command

Please support the following slack commands:

- /help: show available commands
- /new-session: start a new session

sometimes the current conversation is finished, user want to do something else, with `/new-session`, the bot shall start a new session by using `query_with_session` (new in claude agent sdk v0.2.1). And notify the user that a new session is started.

With this change - upon start of repo agent, it shall always generate a new session id, send a message to the channel to inform the user that a new session is started. And use this session id for all the following conversations until the user issues `/new-session`.

Think hard and document this change to ./specs/0005-slack-new-session-command.md.

## graceful shutdown and result messages

this is a slack bot app, if you need to understand more on deps please see ./vendors. I want to add new features: 1) once the final result is sent to slack, slack bot should send another one with tokens consumed,
cost in usd, etc. This info could be found in the ResultMessage. 2) once the tasks are finished slack bot should send a notification so that user get alerted. 3) when slack bot app quit, it shall send to all channels
 that "Agent Gone\nSession ID: xxx ended" in the same format as Agent ready message. please think this through, document your plan in ./specs and then implement it.

## better logging

For all tracing, it should show better formatted data in a dev-friendly way. Review all tracing (debug, info, etc.) and make sure they're easy to read and provide enough information for understanding the flow or problem.

```bash
2025-10-27T14:49:19.953146Z  INFO slack_coder::slack::events: 111: 📨 Received push event: Message(SlackMessageEvent { origin:
SlackMessageOrigin { ts: SlackTs("1761576558.852309"), channel: Some(SlackChannelId("C09NRMS2A58")), channel_type:
Some(SlackChannelType("channel")), thread_ts: None, client_msg_id: None }, content: Some(SlackMessageContent { text: Some(":robot_face:
*Agent Ready*\n\nSession ID: `session-C09NRMS2A58-1761576556-060edf`\n\nI'm ready to help with this repository! Type `/help` for available
commands."), blocks: Some([RichText(Object {"block_id": String("xv2"), "elements": Array [Object {"elements": Array [Object {"name":
String("robot_face"), "type": String("emoji"), "unicode": String("1f916")}, Object {"text": String(" "), "type": String("text")}, Object
{"style": Object {"bold": Bool(true)}, "text": String("Agent Ready"), "type": String("text")}, Object {"text": String("\n\nSession ID: "),
"type": String("text")}, Object {"style": Object {"code": Bool(true)}, "text": String("session-C09NRMS2A58-1761576556-060edf"), "type":
String("text")}, Object {"text": String("\n\nI'm ready to help with this repository! Type "), "type": String("text")}, Object {"style":
Object {"code": Bool(true)}, "text": String("/help"), "type": String("text")}, Object {"text": String(" for available commands."), "type":
String("text")}], "type": String("rich_text_section")}]})]), attachments: None, upload: None, files: None, reactions: None, metadata: None
}), sender: SlackMessageSender { user: Some(SlackUserId("U09NPJCJXU6")), bot_id: Some(SlackBotId("B09P4H5EN65")), username: None,
display_as_bot: None, user_profile: None, bot_profile: Some(SlackBotInfo { id: Some(SlackBotId("B09P4H5EN65")), name: "slack-coder",
updated: Some(SlackDateTime(2025-10-26T17:54:14Z)), app_id: "A09NK3EL5PD", user_id: Some("U09NPJCJXU6"), icons: Some(SlackIconImages {
resolutions: [] }) }) }, subtype: None, hidden: None, message: None, previous_message: None, deleted_ts: None })
```

Think this through, generate a implement plan doc in ./specs and implement it entirely

## The busy message *

This message should be a reply message to the user's original message, to make the message history clean and more readable.

```
:hourglass_flowing_sand: Agent is currently processing another request
Your message has been received, but the agent is busy with a previous task. Please wait for the current task to complete and try again in a moment.
Tip: Long-running tasks (like comprehensive code analysis or documentation) can take several minutes. You can check the latest progress update above.
```

## Main agent doesn't show todo tasks *

Help me understand why the main agent doesn't show todo tasks. If you identified the root cause, make a plan and execute it.

```
slack-coder
APP  7:58 AM
:wrench: Setting up repository tyrchen/slack-coder...
This may take a minute. I'll update you on progress.
```

logs:

```bash

2025-10-27T14:58:23.958931Z  INFO slack_coder::slack::events: 162: 🔔 App mentioned [C09NU1KFXHT] by user: U09JDBT2MCM
2025-10-27T14:58:23.958972Z  INFO slack_coder::slack::events: 185: 📝 Original text: '<@U09NPJCJXU6> tyrchen/slack-coder'
2025-10-27T14:58:23.958987Z  INFO slack_coder::slack::events: 186: 🧹 Cleaned text: 'tyrchen/slack-coder'
2025-10-27T14:58:23.959009Z  INFO slack_coder::slack::events: 206: 🔧 Detected setup request: tyrchen/slack-coder
2025-10-27T14:58:23.959025Z  INFO slack_coder::slack::forms: 38: 🔧 Starting repository setup [C09NU1KFXHT] repo=tyrchen/slack-coder
2025-10-27T14:58:24.140903Z  INFO slack_coder::slack::forms: 57: ✅ Acknowledgment sent
2025-10-27T14:58:24.141052Z  INFO slack_coder::slack::forms: 60: 🚀 Invoking agent manager to setup channel...
2025-10-27T14:58:24.141083Z  INFO slack_coder::agent::manager: 76: 🎬 Setting up [C09NU1KFXHT] repo=tyrchen/slack-coder
2025-10-27T14:58:24.141118Z  INFO slack_coder::agent::hooks: 12: 🎣 Creating TodoWrite hooks for [C09NU1KFXHT]
2025-10-27T14:58:24.141161Z  INFO slack_coder::agent::hooks: 74: ✅ TodoWrite hooks registered for [C09NU1KFXHT]
2025-10-27T14:58:24.141192Z  INFO slack_coder::agent::manager: 91: ✅ Main agent created
2025-10-27T14:58:24.141210Z  INFO slack_coder::agent::manager: 93: 🔗 Connecting main agent to Claude...
2025-10-27T14:58:24.742698Z  INFO slack_coder::slack::events: 111: 📨 Received push event: Message(SlackMessageEvent { origin: SlackMessageOrigin { ts: SlackTs("1761577104.097489"), channel: Some(SlackChannelId("C09NU1KFXHT")), channel_type: Some(SlackChannelType("channel")), thread_ts: None, client_msg_id: None }, content: Some(SlackMessageContent { text: Some(":wrench: Setting up repository `tyrchen/slack-coder`...\nThis may take a minute. I'll update you on progress."), blocks: Some([RichText(Object {"block_id": String("lM/f"), "elements": Array [Object {"elements": Array [Object {"name": String("wrench"), "type": String("emoji"), "unicode": String("1f527")}, Object {"text": String(" Setting up repository "), "type": String("text")}, Object {"style": Object {"code": Bool(true)}, "text": String("tyrchen/slack-coder"), "type": String("text")}, Object {"text": String("...\nThis may take a minute. I'll update you on progress."), "type": String("text")}], "type": String("rich_text_section")}]})]), attachments: None, upload: None, files: None, reactions: None, metadata: None }), sender: SlackMessageSender { user: Some(SlackUserId("U09NPJCJXU6")), bot_id: Some(SlackBotId("B09P4H5EN65")), username: None, display_as_bot: None, user_profile: None, bot_profile: Some(SlackBotInfo { id: Some(SlackBotId("B09P4H5EN65")), name: "slack-coder", updated: Some(SlackDateTime(2025-10-26T17:54:14Z)), app_id: "A09NK3EL5PD", user_id: Some("U09NPJCJXU6"), icons: Some(SlackIconImages { resolutions: [] }) }) }, subtype: None, hidden: None, message: None, previous_message: None, deleted_ts: None })
2025-10-27T14:58:24.742858Z  INFO slack_coder::slack::events: 239: 📬 Message event received
2025-10-27T14:58:26.524258Z  INFO slack_coder::agent::manager: 95: ✅ Connected to Claude
2025-10-27T14:58:26.524303Z  INFO slack_coder::agent::manager: 97: 🚀 Running repository setup (this may take 1-2 minutes)...
```

## improve logging

you probably need a struct to store metadata on ChannelInfo which contains channel_id, hashmap of user_id -> username, and other important metadata. Write it down and generate a doc with concrete design and implement plan in ./specs.

## notification improvement

starting (restoring agents) and stopping (graceful message) could all be done in parallel. Also remove channel specific (agent start/stop) notifications, only do one notification after all agents restored, and one notification after all agent shutdown. Also when a full task finished, right now send result, metrics, etc. And each of those have a notification, it should only generate one notification.
Think hard on it make a good plan and execute
