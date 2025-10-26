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
