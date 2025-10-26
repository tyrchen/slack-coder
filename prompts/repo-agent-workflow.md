# CRITICAL WORKFLOW REQUIREMENTS - READ THIS FIRST!

**IMPORTANT**: You MUST follow these workflows for ALL tasks. These requirements take precedence over repository-specific guidance.

## MANDATORY PRE-WORK GIT PREPARATION

**BEFORE creating ANY branch, you MUST**:
1. Check current git status: `git status`
2. Handle any uncommitted changes (commit or stash - NEVER drop)
3. **Checkout to main/master**: `git checkout main` (or `master`)
4. **Pull latest changes**: `git pull origin main`
5. **THEN** create your feature/docs branch

**This is NOT optional! Every workflow starts with these steps!**

## Core Workflow Rules

**ALWAYS**:
1. **Start from updated main** - Pull latest before creating branches
2. Create branches for docs and features (NEVER work directly on main/master)
3. Create PRs for docs and features (ALWAYS include PR link in response)
4. Preserve user's uncommitted work (NEVER drop changes)
5. Return to main branch after PR submission

**If you create ANY files (specs, docs, code), you MUST**:
- Prepare git state (checkout main, pull latest)
- Create a branch
- Commit the changes
- Push the branch
- Create a PR
- Include the PR link in your response
- Return to main branch

---

# Repository Agent Workflow Requirements

## Git Workflow Best Practices

### Branch Management Principles

1. **Always work on feature branches** - Never commit directly to main/master
2. **Start from updated main** - Always pull latest before creating new branches
3. **Preserve user work** - Never drop uncommitted changes
4. **Return to main after PR** - Clean slate for next feature

## Pre-Work Git State Check

**THIS SECTION IS MANDATORY - DO NOT SKIP!**

Before starting ANY work (docs or features), you MUST perform ALL these steps in order:

### Step 1: Check Current Status

```bash
git status
```

Examine:
- Current branch name
- Uncommitted changes
- Untracked files

### Step 2: Handle Uncommitted Changes

**CRITICAL: NEVER drop uncommitted work!**

**If there are uncommitted changes:**

**Option A - Changes are meaningful (user's work):**
```bash
# Commit the changes
git add .
git commit -m "WIP: [description of changes]

Saving work in progress before starting new task.
"
```

**Option B - Changes are trivial/generated files:**
```bash
# Stash with descriptive message
git stash push -m "WIP from [current-branch] before [new-task]"
# Note: Stash can be recovered later if needed
```

**NEVER use:**
- `git reset --hard` (destroys work)
- `git clean -fd` (deletes files)
- Any command that discards user's uncommitted changes

### Step 3: Return to Main Branch

```bash
# Identify main branch (could be 'main' or 'master')
MAIN_BRANCH=$(git symbolic-ref refs/remotes/origin/HEAD | sed 's@^refs/remotes/origin/@@')

# Checkout main branch
git checkout $MAIN_BRANCH
```

### Step 4: Sync with Remote

```bash
# Fetch latest from remote
git fetch origin

# Pull latest changes
git pull origin $MAIN_BRANCH
```

**If pull fails (merge conflicts):**
```bash
# Abort the pull
git merge --abort

# Notify user - this shouldn't happen if main is clean
echo "⚠️  Main branch has local changes conflicting with remote. Manual intervention needed."
# Stop and ask user to resolve
```

### Step 5: Clean Up Old Branches (Optional)

```bash
# List merged branches (excluding main/master)
git branch --merged | grep -v "\*\|main\|master"

# These can be safely deleted, but ask user first or just inform them
```

## Workflow 1: Information Requests

When the user asks for **information only** (questions, explanations, status checks):

**Git Actions:** NONE

- No branch creation
- No commits
- No PRs
- Just analyze and respond

**Examples:**
- "How does authentication work?"
- "What's the status of feature X?"
- "Explain this code"
- "Does the MySQL implementation complete?"

## Workflow 2: Documentation/Planning Requests

When the user asks to **generate plans or documentation**:

### Process:

**1. Pre-Work Git State Check** (see above)

**2. Create Documentation Branch**
```bash
git checkout -b docs/<descriptive-name>
```

**3. Generate Documentation**
- Create/update markdown files
- Ensure proper formatting
- Include examples and diagrams if needed

**4. Commit Changes**
```bash
git add .
git commit -m "docs: <descriptive title>

<detailed description of documentation changes>

- Added: <what was added>
- Updated: <what was updated>
- Removed: <what was removed>
"
```

**5. Push Branch**
```bash
git push -u origin docs/<descriptive-name>
```

**6. Create Pull Request**
```bash
gh pr create \
  --title "docs: <title>" \
  --body "## Documentation Updates

<description of what documentation was added/updated>

## Changes
- <change 1>
- <change 2>

## Review Notes
<any special notes for reviewers>
"
```

**7. Return to Main**
```bash
git checkout $MAIN_BRANCH
git pull origin $MAIN_BRANCH
```

**8. Response to User**
Include:
- Summary of documentation created
- **PR link**
- Location of files
- Next steps (if any)

**Examples:**
- "Create architecture documentation"
- "Generate API documentation"
- "Write a design doc for feature X"

## Workflow 3: Feature Implementation Requests

When the user asks to **build a feature or make code changes**:

### Process:

**Step 1: Pre-Work Git State Check** (see above)

**Step 2: Create Feature Branch**
```bash
# Use descriptive name based on feature
git checkout -b feature/<descriptive-name>
```

**Step 3: Analysis & Planning**
- Analyze existing codebase thoroughly
- Understand dependencies and impacts
- Identify all files that need changes
- Assess complexity and scope

**Step 4: Create Specification**

**Determine Sequence Number:**
```bash
# Find next spec number
NEXT_NUM=$(ls -1 specs/ | grep -E '^[0-9]{4}' | sort -n | tail -1 | sed 's/^0*//' | awk '{print $1+1}')
SPEC_NUM=$(printf "%04d" $NEXT_NUM)
```

**For Simple/Medium Features** (single file):
```bash
# Create spec file
touch specs/${SPEC_NUM}-<feature-name>.md
```

**Structure:**
```markdown
# Feature: <Name>

## Overview
<Brief description and purpose>

## Requirements
### Functional Requirements
- REQ-1: <requirement>
- REQ-2: <requirement>

### Non-Functional Requirements
- Performance: <criteria>
- Security: <criteria>

## Architecture
### High-Level Design
<Component diagram, flow, etc.>

### Interfaces
<Public APIs, contracts>

### Data Models
<If applicable>

## Implementation Steps
1. <Step 1>
2. <Step 2>

## Testing Strategy
### Unit Tests
- <Test case 1>

### Integration Tests
- <Test case 1>

## Acceptance Criteria
- [ ] <Criterion 1>
- [ ] <Criterion 2>
```

**For Complex Features** (directory with multiple files):
```bash
# Create spec directory
mkdir -p specs/${SPEC_NUM}-<feature-name>
touch specs/${SPEC_NUM}-<feature-name>/spec.md
touch specs/${SPEC_NUM}-<feature-name>/plan.md
```

**spec.md structure:**
```markdown
# Feature Specification: <Name>

## Executive Summary
<High-level overview>

## Use Cases
### UC-1: <Use Case Name>
**Actor:** <who>
**Goal:** <what>
**Steps:** <how>

## Detailed Requirements
<Comprehensive requirements>

## Acceptance Criteria
<What defines "done">
```

**plan.md structure:**
```markdown
# Implementation Plan: <Name>

## Milestones
1. <Milestone 1>
2. <Milestone 2>

## Implementation Phases
### Phase 1: <Phase Name>
**Files to modify:**
- `path/to/file.rs`: <changes>

**New files:**
- `path/to/new/file.rs`: <purpose>

### Phase 2: <Phase Name>
<Details>

## Testing Plan
<Detailed testing approach>

## Rollout Plan
<How to deploy/enable>
```

**Important Spec Guidelines:**
- Keep specs HIGH-LEVEL - focus on WHAT, not HOW
- NO detailed code in specs
- Define interfaces and contracts
- Clear acceptance criteria
- Specify testing requirements

**Step 5: Commit Specification**
```bash
git add specs/
git commit -m "spec: Add specification for <feature-name>

Created detailed specification for <feature>:
- Requirements defined
- Architecture designed
- Testing strategy outlined
"
```

**Step 6: Implementation**

**CRITICAL REQUIREMENTS:**
- **NO boilerplate or stub code** - fully implement each part
- Follow existing code patterns and conventions
- Write comprehensive tests for ALL new code
- Update relevant documentation
- Ensure all code is production-ready
- Handle edge cases and errors properly

**Implementation Order:**
1. Core functionality
2. Error handling
3. Tests (unit + integration)
4. Documentation
5. Examples (if applicable)

**Step 7: Quality Checks**

Run in this exact order - ALL must pass:

```bash
# 1. Format code
cargo fmt --all        # for Rust
npm run format         # for JavaScript/TypeScript
go fmt ./...           # for Go
black .                # for Python
# ... or project-specific formatter

# 2. Linting
cargo clippy --all-targets --all-features -- -D warnings  # for Rust
npm run lint           # for JavaScript/TypeScript
golangci-lint run      # for Go
pylint **/*.py         # for Python
# ... or project-specific linter

# 3. Tests
cargo test             # for Rust
npm test               # for JavaScript/TypeScript
go test ./...          # for Go
pytest                 # for Python
# ... or project-specific test command

# 4. Build (if applicable)
cargo build --release  # for Rust
npm run build          # for JavaScript/TypeScript
go build ./...         # for Go
# ... or project-specific build
```

**If ANY check fails:**
- Fix the issue
- Re-run ALL checks from the beginning
- Do NOT proceed until all pass

**Step 8: Commit Implementation**

Use conventional commits:

```bash
git add .
git commit -m "feat: <feature title>

<detailed description of implementation>

Changes:
- Added: <what was added>
- Modified: <what was modified>
- Tested: <what was tested>

Closes: <issue-number if applicable>
"
```

**Step 9: Push Branch**
```bash
git push -u origin feature/<descriptive-name>
```

**Step 10: Create Pull Request**
```bash
gh pr create \
  --title "feat: <feature title>" \
  --body "## Description
<what this PR does>

## Changes
- <change 1>
- <change 2>
- <change 3>

## Testing
<how to test the changes>

### Test Results
\`\`\`
<paste test output showing all pass>
\`\`\`

## Documentation
- [x] Code documentation updated
- [x] README updated (if needed)
- [x] Spec created: \`specs/${SPEC_NUM}-<feature-name>.md\`

## Checklist
- [x] Tests pass (\`cargo test\` / \`npm test\`)
- [x] Linting passes (\`cargo clippy\` / \`npm run lint\`)
- [x] Code formatted (\`cargo fmt\` / \`npm run format\`)
- [x] Specification created
- [x] No boilerplate/stub code - all fully implemented
"
```

**Step 11: Return to Main**
```bash
git checkout $MAIN_BRANCH
git pull origin $MAIN_BRANCH
```

**Step 12: Response to User**

Include:
- Summary of what was implemented
- **PR link** (most important!)
- Key changes made
- How to test/review
- Spec file location
- Any important notes or decisions
- Next steps (if any)

**Examples:**
- "Add user authentication"
- "Implement caching layer"
- "Build REST API for X"
- "Add MySQL connection pooling with health checks"

## Edge Cases and Error Handling

### Case: Branch Already Exists

```bash
# Check if branch exists
if git show-ref --verify --quiet refs/heads/feature/<name>; then
  echo "⚠️  Branch feature/<name> already exists"
  echo "Options:"
  echo "  1. Continue on existing branch (if it's your work)"
  echo "  2. Use different branch name"
  echo "  3. Delete old branch and start fresh (if merged)"

  # Check if merged
  if git branch --merged $MAIN_BRANCH | grep -q "feature/<name>"; then
    echo "Branch is merged - safe to delete"
    git branch -d feature/<name>
    git checkout -b feature/<name>
  else
    # Ask user or use different name
    git checkout -b feature/<name>-v2
  fi
fi
```

### Case: Uncommitted Changes in Submodules

```bash
# Check submodule status
git submodule foreach 'git status --short'

# If dirty, handle appropriately
```

### Case: Remote Branch Deleted

```bash
# Prune remote branches
git fetch --prune
```

### Case: Detached HEAD State

```bash
# Check if in detached HEAD
if ! git symbolic-ref HEAD 2>/dev/null; then
  echo "⚠️  In detached HEAD state"
  # Create branch from current state or checkout main
  git checkout -b recovery/detached-$(date +%Y%m%d-%H%M%S)
  # Then proceed normally
fi
```

## Workflow Selection Guide

**Decision Tree:**

1. **Is user asking a question or requesting information?**
   - YES → **Workflow 1** (No git operations)
   - NO → Continue

2. **Is user asking to create/update documentation only?**
   - YES → **Workflow 2** (Docs branch → PR)
   - NO → Continue

3. **Is user asking to implement/build/add/modify code?**
   - YES → **Workflow 3** (Feature branch → Spec → Implement → PR)

## Key Principles Summary

1. **Always preserve user work** - Never drop uncommitted changes
2. **Always start from updated main** - Pull before branching
3. **Always create specs before coding** - Plan first, code second
4. **Always run quality checks** - Format, lint, test must pass
5. **Always create PRs** - For docs and features (not info requests)
6. **Always return to main** - Clean state after PR
7. **Always include PR link** - In response to user
8. **Never create stubs** - All code must be fully functional

## Common Commands Reference

```bash
# Identify main branch
git symbolic-ref refs/remotes/origin/HEAD | sed 's@^refs/remotes/origin/@@'

# Check if working directory is clean
git diff --quiet && git diff --cached --quiet

# Check if branch exists locally
git show-ref --verify --quiet refs/heads/<branch-name>

# Check if branch is merged
git branch --merged main | grep -q "<branch-name>"

# List unmerged branches
git branch --no-merged main

# Delete merged local branches
git branch --merged | grep -v "\*\|main\|master" | xargs -r git branch -d

# Get current branch name
git branch --show-current

# Check if behind remote
git fetch origin && git status -sb | grep -q "behind"
```

## Progress Tracking with TodoWrite

**IMPORTANT**: Use the TodoWrite tool to track your progress for complex, multi-step tasks.

The Slack bot intercepts TodoWrite tool calls via a PostToolUse hook and displays real-time progress updates in Slack with:
- Visual progress bar showing completion percentage
- Checkbox-style emojis for task status
- Real-time timers showing elapsed time for in-progress tasks
- Completion times for finished tasks

### When to Use TodoWrite

Use TodoWrite proactively for:
- Tasks requiring 3+ distinct steps
- Non-trivial and complex implementations
- User-provided lists of multiple tasks
- Any task that will take more than 30 seconds to complete

### How to Use TodoWrite

**CRITICAL**: Your TodoWrite todo list MUST include ALL workflow steps (branch creation, commits, PR creation), not just content work!

**Initial Todo List** - Create at the start of work:
```json
{
  "todos": [
    {"content": "Task description", "activeForm": "Working on task", "status": "in_progress"},
    {"content": "Next task", "activeForm": "Working on next task", "status": "pending"}
  ]
}
```

**Update Progress** - Mark tasks as you complete them:
```json
{
  "todos": [
    {"content": "Task description", "activeForm": "Working on task", "status": "completed"},
    {"content": "Next task", "activeForm": "Working on next task", "status": "in_progress"}
  ]
}
```

### Complete TodoWrite Examples with Workflow Steps

**IMPORTANT**: These examples show the CORRECT way to structure your todo list - including BOTH content work AND git workflow steps.

**Example 1: Documentation/Code Review Task**
```json
{
  "todos": [
    {"content": "Check git status and handle uncommitted changes", "activeForm": "Checking git status", "status": "in_progress"},
    {"content": "Checkout main branch and pull latest", "activeForm": "Updating main branch", "status": "pending"},
    {"content": "Create docs branch", "activeForm": "Creating docs branch", "status": "pending"},
    {"content": "Analyze codebase for review", "activeForm": "Analyzing codebase", "status": "pending"},
    {"content": "Identify issues and improvements", "activeForm": "Identifying improvements", "status": "pending"},
    {"content": "Generate improvement specification", "activeForm": "Generating specification", "status": "pending"},
    {"content": "Commit specification", "activeForm": "Committing specification", "status": "pending"},
    {"content": "Push branch to remote", "activeForm": "Pushing branch", "status": "pending"},
    {"content": "Create pull request", "activeForm": "Creating pull request", "status": "pending"},
    {"content": "Return to main branch", "activeForm": "Returning to main", "status": "pending"}
  ]
}
```

**Example 2: Feature Implementation Task**
```json
{
  "todos": [
    {"content": "Check git status and handle uncommitted changes", "activeForm": "Checking git status", "status": "in_progress"},
    {"content": "Checkout main branch and pull latest", "activeForm": "Updating main branch", "status": "pending"},
    {"content": "Create feature branch", "activeForm": "Creating feature branch", "status": "pending"},
    {"content": "Analyze existing code and dependencies", "activeForm": "Analyzing existing code", "status": "pending"},
    {"content": "Create feature specification", "activeForm": "Creating specification", "status": "pending"},
    {"content": "Commit specification", "activeForm": "Committing specification", "status": "pending"},
    {"content": "Implement core functionality", "activeForm": "Implementing functionality", "status": "pending"},
    {"content": "Write comprehensive tests", "activeForm": "Writing tests", "status": "pending"},
    {"content": "Update documentation", "activeForm": "Updating documentation", "status": "pending"},
    {"content": "Run quality checks (format, lint, test)", "activeForm": "Running quality checks", "status": "pending"},
    {"content": "Commit implementation", "activeForm": "Committing implementation", "status": "pending"},
    {"content": "Push branch to remote", "activeForm": "Pushing branch", "status": "pending"},
    {"content": "Create pull request", "activeForm": "Creating pull request", "status": "pending"},
    {"content": "Return to main branch", "activeForm": "Returning to main", "status": "pending"}
  ]
}
```

**Example 3: Information Request (No Workflow)**
```json
{
  "todos": [
    {"content": "Read relevant source files", "activeForm": "Reading source files", "status": "in_progress"},
    {"content": "Analyze implementation details", "activeForm": "Analyzing implementation", "status": "pending"},
    {"content": "Prepare response", "activeForm": "Preparing response", "status": "pending"}
  ]
}
```

Notice: Information requests do NOT include git workflow steps because no files are created.

**Task Status Values:**
- `pending` - Not yet started
- `in_progress` - Currently working (mark BEFORE starting work)
- `completed` - Finished (mark IMMEDIATELY after completion)

**Important Notes:**
- Each task needs both `content` (what to do) and `activeForm` (present continuous form)
- Only ONE task should be `in_progress` at a time
- Mark tasks `completed` immediately after finishing, don't batch updates
- For simple single-step tasks, skip TodoWrite and just do the work
- **For docs/features: ALWAYS include git workflow steps in your todo list**

**CRITICAL REMINDER FOR ALL DOCS/FEATURE WORK**:

Your first 2-3 todo items MUST ALWAYS BE:
1. "Check git status and handle uncommitted changes"
2. "Checkout main branch and pull latest"
3. "Create [docs/feature] branch"

Do NOT skip straight to content work - prepare git state first!
