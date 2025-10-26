# System Prompt Restructure

## Summary

Reorganized system prompts into modular components with comprehensive git workflow best practices.

## Problems Addressed

### Git Workflow Gaps

Previous workflow didn't handle common scenarios:
1. ❌ User has uncommitted changes when starting new work
2. ❌ User is on a feature branch when starting new work
3. ❌ Main branch not synced with remote before starting
4. ❌ Unclear what to do after PR submission
5. ❌ No guidance on branch cleanup
6. ❌ Risk of losing user's work

### System Prompt Organization

- Mixed repository-specific and workflow instructions
- Duplication across all generated prompts
- Hard to update workflow requirements globally

## Solution

### New Structure

```
prompts/
├── main-agent-system-prompt.md      # Main agent (repo setup)
└── repo-agent-workflow.md           # Common workflow (loaded for ALL repos)

~/.slack_coder/system/<channel>/
└── system_prompt.md                 # Repository-specific only
```

### Loading Strategy

**Repo Agent System Prompt = Repository-Specific + Common Workflow**

```rust
// Load repo-specific prompt
let mut system_prompt = workspace.load_system_prompt(&channel_id).await?;

// Append common workflow (includes TodoWrite, git workflows, PR creation)
system_prompt.push_str(include_str!("../../prompts/repo-agent-workflow.md"));
```

## Comprehensive Git Workflow

### Pre-Work Git State Check

**Always performed before ANY work:**

1. **Check Status**: `git status`
2. **Handle Uncommitted Changes**:
   - **NEVER drop user work**
   - Commit meaningful changes
   - Stash trivial/generated files
3. **Return to Main**: `git checkout main`
4. **Sync with Remote**: `git pull origin main`
5. **Clean Up** (optional): List merged branches

### Workflow 1: Information Requests

**Git Actions**: NONE

Just answer the question - no branches, commits, or PRs.

**Examples**:
- "How does auth work?"
- "Does the MySQL implementation complete?"

### Workflow 2: Documentation Requests

**Git Actions**:
1. Pre-work check
2. Create `docs/<name>` branch
3. Generate documentation
4. Commit changes
5. Push branch
6. Create PR
7. Return to main
8. Include PR link in response

**Examples**:
- "Create architecture documentation"
- "Generate API docs"

### Workflow 3: Feature Implementation

**Git Actions**:
1. Pre-work check
2. Create `feature/<name>` branch
3. Analyze codebase
4. **Create specification first** (in `./specs/`)
5. Commit spec
6. Implement fully (NO stubs)
7. Run quality checks (format, lint, test)
8. Commit implementation
9. Push branch
10. Create PR
11. Return to main
12. Include PR link in response

**Critical Requirements**:
- Always create spec before coding
- Never create stub/boilerplate code
- All quality checks must pass
- Always preserve user work

### Edge Cases Handled

1. **Branch already exists**: Check if merged, delete or use new name
2. **Uncommitted changes in submodules**: Handle appropriately
3. **Remote branch deleted**: Prune
4. **Detached HEAD state**: Create recovery branch
5. **Merge conflicts**: Abort and notify user

## Key Workflow Principles

1. **Always preserve user work** - Never drop uncommitted changes
2. **Always start from updated main** - Pull before branching
3. **Always create specs before coding** - Plan first, code second
4. **Always run quality checks** - Format, lint, test must pass
5. **Always create PRs** - For docs and features (not info requests)
6. **Always return to main** - Clean state after PR
7. **Always include PR link** - In response to user
8. **Never create stubs** - All code must be fully functional

## Specification Requirements

### Simple/Medium Features

Single file: `./specs/<seq>-<feature-name>.md`

Sections:
- Overview
- Requirements
- Architecture
- Implementation Steps
- Testing Strategy
- Acceptance Criteria

### Complex Features

Directory: `./specs/<seq>-<feature-name>/`
- `spec.md` - Detailed requirements, use cases
- `plan.md` - Implementation phases, testing plan

**Guidelines**:
- Keep HIGH-LEVEL - focus on WHAT, not HOW
- NO detailed code in specs
- Define interfaces and contracts
- Clear acceptance criteria

## Quality Checks Order

Must run in order, ALL must pass:

1. **Format**: `cargo fmt` / `npm run format` / etc.
2. **Lint**: `cargo clippy` / `npm run lint` / etc.
3. **Test**: `cargo test` / `npm test` / etc.
4. **Build** (if applicable)

If ANY fails, fix and re-run ALL from beginning.

## File Changes

### Created

- `prompts/main-agent-system-prompt.md` - Main agent prompt
- `prompts/repo-agent-workflow.md` - Common workflow (4924 lines)

### Modified

- `src/agent/main_agent.rs` - Load from prompts/
- `src/agent/repo_agent.rs` - Append workflow to loaded prompt
- `specs/0003-system-prompt.md` - Removed workflow, added note

### Updated

- `~/.slack_coder/system/C09NNKZ8SPP/system_prompt.md` - Removed workflow
- `~/.slack_coder/system/C09NNMDNJH3/system_prompt.md` - Removed workflow
- `~/.slack_coder/system/C09NRMS2A58/system_prompt.md` - Removed workflow

## Benefits

### For Maintenance

- ✅ Update workflow once, applies to all repos
- ✅ Repository prompts stay focused on repo-specific info
- ✅ Clear separation of concerns
- ✅ Easier to test and validate workflows

### For Users

- ✅ Consistent workflow across all repos
- ✅ Never lose uncommitted work
- ✅ Always start from updated main
- ✅ Clean git history
- ✅ Proper specs before coding
- ✅ Quality checks enforced
- ✅ PR links always included

### For Development

- ✅ Comprehensive edge case handling
- ✅ Clear decision trees
- ✅ Detailed error handling
- ✅ Common commands reference
- ✅ TodoWrite integration

## Example Scenarios

### Scenario 1: User on Feature Branch, Uncommitted Changes

**Before**:
```bash
# Current state
$ git branch
* feature/old-work
  main

$ git status
Changes not staged for commit:
  modified:   src/main.rs
```

**Workflow Handles**:
1. Detects uncommitted changes
2. Commits them with "WIP" message
3. Switches to main
4. Pulls latest
5. Creates new feature branch
6. User's work preserved ✅

### Scenario 2: Main Branch Out of Sync

**Before**:
```bash
$ git status
On branch main
Your branch is behind 'origin/main' by 5 commits
```

**Workflow Handles**:
1. Detects out-of-sync state
2. Pulls latest from origin
3. Creates feature branch from updated main
4. Avoids merge conflicts later ✅

### Scenario 3: Old Merged Branch Exists

**Before**:
```bash
$ git branch
  feature/user-auth  # Already merged
* main
```

**Workflow Handles**:
1. Checks if branch is merged
2. Safely deletes old branch
3. Creates fresh branch with same name
4. Clean git history ✅

## Testing

✅ Code compiles
✅ All tests pass
✅ Existing prompts updated (3 files)
✅ Workflow file included (625 lines)
✅ Build successful

## Migration

**Existing Repos**: Automatically get new workflow on next agent load
**New Repos**: Generated prompts exclude workflow (loaded separately)

No manual intervention needed!
