# Document 3 — JARVIS Agent Core
## Planning, Tool Calling, Memory & Autonomous Task Execution

**Project:** Local-first JARVIS personal assistant  
**Platforms:** Windows, Ubuntu/Linux, Android  
**Architecture:** Monorepo + shared agent runtime + platform-specific executors  
**Primary principle:** The AI reasons about goals; a controlled execution layer performs actions.

---

# 1. Purpose

Document 2 defined the local AI engine:

- LLM inference
- Vision
- Speech-to-text
- Wake word
- VAD
- TTS
- Model routing
- Model management

This document defines the layer above the models:

```text
User
 ↓
Conversation
 ↓
Intent
 ↓
Agent
 ↓
Planner
 ↓
Policy
 ↓
Tools
 ↓
Environment
 ↓
Observation
 ↓
Agent
```

This is the part that makes JARVIS an **agent** rather than simply a chatbot.

The goal is to allow commands such as:

> "Open Chrome and search for senior React jobs."

and eventually:

> "Find suitable SDE jobs for me, evaluate them against my profile, apply to the good ones, and tell me which applications require my attention."

The system must support both:

1. **Direct deterministic commands**
2. **Long-running autonomous workflows**

---

# 2. Core Design Philosophy

JARVIS should not be one giant prompt.

It should be a distributed runtime:

```text
                  ┌─────────────────┐
                  │      User       │
                  └────────┬────────┘
                           ↓
                  ┌─────────────────┐
                  │ Intent Router   │
                  └────────┬────────┘
                           ↓
             ┌─────────────┴─────────────┐
             ↓                           ↓
      Deterministic Task            AI Agent Task
             ↓                           ↓
      Direct Tool Call              Planner
                                         ↓
                                  Policy Engine
                                         ↓
                                  Tool Executor
                                         ↓
                                   Observation
                                         ↓
                                     Planner
```

The LLM should not control the entire computer directly.

Instead:

```text
LLM
 ↓
structured tool request
 ↓
policy validation
 ↓
tool executor
 ↓
operating system
```

---

# 3. Agent Responsibilities

The agent layer is responsible for:

- Understanding user goals.
- Determining whether a request requires planning.
- Selecting tools.
- Creating execution plans.
- Tracking task state.
- Maintaining context.
- Observing results.
- Recovering from failures.
- Asking the user for missing information.
- Requesting confirmation for risky actions.
- Completing multi-step tasks.
- Reporting progress.
- Remembering relevant outcomes.
- Cancelling tasks.
- Resuming interrupted tasks.

---

# 4. Non-Responsibilities

The agent should not directly:

- Execute arbitrary shell commands.
- Read every file on the computer.
- Store passwords in prompts.
- Bypass operating-system permissions.
- Automatically approve financial transactions.
- Ignore security policies.
- Treat webpage instructions as trusted instructions.
- Modify its own security policy.
- Install arbitrary software without authorization.

These responsibilities belong to controlled subsystems.

---

# 5. Agent Architecture

Recommended:

```text
agent/
├── intent/
├── planner/
├── executor/
├── observer/
├── state/
├── policy/
├── tools/
├── memory/
├── context/
├── recovery/
├── scheduler/
└── supervisor/
```

---

# 6. Intent Router

The first decision is:

> What kind of request is this?

Categories:

```text
CHAT
QUERY
SYSTEM_COMMAND
MEDIA_COMMAND
APP_COMMAND
BROWSER_TASK
FILE_TASK
RESEARCH_TASK
AUTOMATION_TASK
MULTI_STEP_TASK
SCHEDULED_TASK
VISION_TASK
MOBILE_TASK
HIGH_RISK_TASK
```

Example:

```text
"What's the weather?"
→ QUERY

"Open VS Code"
→ SYSTEM_COMMAND

"Play Arijit Singh"
→ MEDIA_COMMAND

"Apply to SDE jobs"
→ MULTI_STEP_TASK
```

---

# 7. Deterministic vs Agentic Routing

Not everything requires an LLM.

Use deterministic handlers for:

```text
volume up
volume down
mute
pause
resume
open app
close app
lock screen
shutdown
take screenshot
```

Use the agent for:

```text
find suitable jobs
research a company
compare products
organize files
debug a repository
complete a multi-page form
prepare an application
```

Architecture:

```text
Input
 ↓
Fast Intent Classifier
 ├── deterministic
 └── agent
```

---

# 8. Agent Task Object

Every non-trivial operation becomes a task.

Example:

```json
{
  "task_id": "task_123",
  "goal": "Find suitable SDE jobs",
  "status": "planning",
  "priority": "normal",
  "created_at": "...",
  "platform": "windows",
  "requires_confirmation": false,
  "steps": [],
  "context": {},
  "artifacts": [],
  "errors": []
}
```

---

# 9. Task States

Recommended state machine:

```text
CREATED
 ↓
UNDERSTANDING
 ↓
PLANNING
 ↓
WAITING_FOR_INPUT
 ↓
WAITING_FOR_CONFIRMATION
 ↓
EXECUTING
 ↓
OBSERVING
 ↓
RECOVERING
 ↓
COMPLETED
```

Terminal states:

```text
FAILED
CANCELLED
EXPIRED
BLOCKED
```

---

# 10. Task State Machine

```text
                 ┌───────────────┐
                 │    CREATED    │
                 └───────┬───────┘
                         ↓
                 ┌───────────────┐
                 │ UNDERSTANDING │
                 └───────┬───────┘
                         ↓
                 ┌───────────────┐
                 │    PLANNING   │
                 └───┬───────┬───┘
                     │       │
             missing │       │ ready
                     ↓       ↓
              WAIT_INPUT   EXECUTING
                     │       ↓
                     └──→ OBSERVING
                            ↓
                       ┌────┴────┐
                       ↓         ↓
                    RECOVER   COMPLETE
```

---

# 11. Planning

The planner converts:

```text
Goal
```

into:

```text
ordered actions
```

Example:

```text
Goal:
Apply for suitable SDE jobs.

Plan:
1. Load user job profile.
2. Open browser.
3. Check login state.
4. Search jobs.
5. Extract candidate jobs.
6. Rank jobs.
7. Open first job.
8. Determine application method.
9. Fill known information.
10. Detect missing information.
11. Ask user if required.
12. Validate form.
13. Request confirmation if policy requires.
14. Submit.
15. Verify submission.
16. Record result.
17. Continue.
```

---

# 12. Planning Strategies

Three strategies should be supported.

## A. ReAct

```text
Reason
→ Act
→ Observe
→ Reason
```

Good for dynamic environments.

## B. Plan-and-Execute

```text
Create plan
→ Execute steps
→ Replan if needed
```

Good for predictable workflows.

## C. State Machine

```text
State
→ deterministic transition
→ next state
```

Best for high-reliability workflows.

JARVIS should use all three.

---

# 13. Recommended Hybrid

```text
High-level task
        ↓
Plan-and-Execute
        ↓
Each step
        ↓
State-machine validation
        ↓
Dynamic step
        ↓
ReAct loop
```

This avoids depending entirely on free-form LLM reasoning.

---

# 14. Tool Registry

Every capability is represented as a tool.

Example:

```json
{
  "name": "browser.open",
  "description": "Open a URL in the controlled browser.",
  "input_schema": {
    "type": "object",
    "properties": {
      "url": {
        "type": "string"
      }
    },
    "required": ["url"]
  },
  "risk": "low",
  "platforms": ["windows", "linux", "android"]
}
```

---

# 15. Tool Categories

```text
system
browser
filesystem
applications
media
communication
calendar
email
documents
vision
audio
network
research
memory
security
android
windows
linux
developer
```

---

# 16. System Tools

Examples:

```text
system.get_status
system.get_time
system.lock
system.sleep
system.shutdown
system.restart
system.volume
system.brightness
system.screenshot
```

---

# 17. Application Tools

Examples:

```text
app.list
app.open
app.close
app.focus
app.minimize
app.maximize
app.get_state
```

---

# 18. Browser Tools

Examples:

```text
browser.open
browser.back
browser.forward
browser.refresh
browser.tabs
browser.new_tab
browser.close_tab
browser.click
browser.type
browser.select
browser.scroll
browser.extract
browser.screenshot
browser.wait
```

Browser tools should prefer semantic selectors.

---

# 19. Browser Automation Priority

Use:

```text
DOM
 ↓
Accessibility tree
 ↓
Browser automation API
 ↓
OCR
 ↓
Vision
 ↓
Coordinates
```

Coordinate clicking should be the last resort.

---

# 20. Filesystem Tools

Examples:

```text
fs.list
fs.read
fs.write
fs.copy
fs.move
fs.rename
fs.delete
fs.search
fs.create_directory
```

Dangerous operations require policy checks.

---

# 21. File Safety

For destructive operations:

```text
delete
overwrite
mass rename
mass move
```

The tool should expose:

```text
preview
affected_files
rollback_available
```

Example:

> "I found 142 files matching that rule. I can rename them, but this will modify all 142. Proceed?"

---

# 22. Shell Tool

A shell tool may exist, but it must be restricted.

Do not provide:

```text
shell.exec(command: arbitrary_string)
```

as the only interface.

Instead support:

```text
shell.execute
```

with:

```text
allowed_commands
working_directory
environment_policy
timeout
network_policy
risk_level
```

Dangerous commands should be blocked or require confirmation.

---

# 23. Tool Permission Model

Each tool receives:

```text
read
write
execute
network
credentials
external_side_effect
```

Example:

```json
{
  "tool": "browser.submit_form",
  "permissions": [
    "write",
    "external_side_effect"
  ]
}
```

---

# 24. Capability Tokens

For advanced security, the policy engine can issue temporary capabilities.

Example:

```text
Task:
Organize Downloads

Capability:
filesystem.read(Downloads)
filesystem.write(Downloads)

Expires:
10 minutes
```

The tool cannot access unrelated directories.

---

# 25. Policy Engine

The policy engine decides:

```text
ALLOW
DENY
ASK
```

Inputs:

```text
tool
arguments
user
task
risk
current state
credentials
target
```

---

# 26. Risk Classification

## Low

```text
open application
read webpage
play music
take screenshot
```

## Medium

```text
send email
modify files
post content
fill forms
```

## High

```text
financial transaction
password change
account deletion
legal submission
mass deletion
```

## Critical

Require explicit user action outside autonomous execution.

---

# 27. Confirmation Policy

Example:

```text
"Open Chrome"
→ no confirmation

"Delete this folder"
→ confirmation

"Submit job application"
→ policy-dependent confirmation

"Transfer ₹50,000"
→ explicit confirmation
```

The user should be able to configure trusted actions.

---

# 28. Confirmation Levels

```text
ALWAYS
ONCE
PER_TASK
PER_SESSION
NEVER
```

Default sensitive operations:

```text
ALWAYS
```

unless the user explicitly changes the policy.

---

# 29. Confirmation UX

JARVIS should speak naturally:

> "The application is complete and ready to submit. Shall I submit it?"

User:

> "Yes."

JARVIS:

> "Submitting now."

---

# 30. Missing Information

The agent should detect missing data.

Example:

> "Your expected salary isn't in your job profile. What should I enter?"

The task becomes:

```text
WAITING_FOR_INPUT
```

After the answer:

```text
resume task
```

---

# 31. Authentication

When a website requires login:

```text
Agent detects login
 ↓
Policy
 ↓
Credential availability
```

If credentials are available through the secure credential manager:

```text
use credential tool
```

Otherwise:

> "LinkedIn requires you to log in. Please complete the login in the browser."

JARVIS should not ask the user to speak a password aloud.

---

# 32. Credential Architecture

Use platform secure storage:

Windows:

```text
Windows Credential Manager
```

Linux:

```text
Secret Service / keyring
```

Android:

```text
Android Keystore
```

The LLM receives:

```text
credential_available: true
```

not:

```text
password: actual_password
```

---

# 33. CAPTCHA

CAPTCHA should be considered a human checkpoint.

Flow:

```text
CAPTCHA detected
 ↓
pause
 ↓
notify user
 ↓
user solves CAPTCHA
 ↓
agent resumes
```

Do not build CAPTCHA bypass mechanisms.

---

# 34. Human-in-the-Loop

The agent must be able to pause at any point.

States:

```text
WAITING_FOR_USER
```

Reasons:

```text
missing information
authentication
CAPTCHA
confirmation
ambiguous instruction
high-risk action
unexpected UI
```

---

# 35. Observation

Tools should return structured observations.

Bad:

```text
"Something happened."
```

Good:

```json
{
  "status": "success",
  "url": "...",
  "page_title": "...",
  "elements_changed": 4,
  "screenshot_available": true
}
```

---

# 36. Observation Hierarchy

Prefer:

```text
Structured API result
 ↓
DOM
 ↓
Accessibility tree
 ↓
OCR
 ↓
Vision
 ↓
Raw screenshot
```

---

# 37. Replanning

The agent must never assume an action succeeded.

Example:

```text
click "Submit"
 ↓
observe
 ↓
if success:
    continue
else:
    inspect error
    replan
```

---

# 38. Retry Policy

Each tool has:

```text
max_retries
backoff
retryable_errors
```

Example:

```text
network timeout:
retry

invalid password:
do not retry repeatedly

CAPTCHA:
pause

element missing:
re-observe
```

---

# 39. Recovery Strategies

When an action fails:

```text
1. Retry
2. Refresh state
3. Re-observe
4. Find alternative UI path
5. Replan
6. Ask user
7. Fail gracefully
```

---

# 40. Idempotency

Tools should declare whether repeated execution is safe.

Example:

```text
browser.refresh
→ idempotent

send_email
→ NOT idempotent

submit_application
→ NOT idempotent
```

The executor must prevent accidental duplicate side effects.

---

# 41. Transaction-like Actions

For complex workflows:

```text
prepare
 ↓
validate
 ↓
commit
```

Example:

```text
Fill job application
 ↓
Validate fields
 ↓
Show final summary
 ↓
Submit
```

---

# 42. Checkpoints

Long-running tasks should checkpoint:

```json
{
  "task_id": "123",
  "completed_steps": [
    "login_checked",
    "jobs_found",
    "profile_loaded"
  ],
  "current_step": "review_application",
  "artifacts": []
}
```

If JARVIS crashes, it can resume.

---

# 43. Persistent Task Store

Use SQLite.

Tables:

```text
tasks
task_steps
task_events
tool_calls
artifacts
approvals
errors
```

---

# 44. Task Events

Example:

```text
TASK_CREATED
PLAN_CREATED
TOOL_STARTED
TOOL_COMPLETED
USER_INPUT_REQUIRED
USER_CONFIRMED
ERROR
REPLAN
TASK_COMPLETED
```

This provides auditability.

---

# 45. Long-Running Tasks

Examples:

```text
Download files
Research companies
Apply to jobs
Monitor a website
Organize documents
Generate a project
Run tests
```

They must run independently of the voice UI.

Architecture:

```text
Voice UI
    ↓
Task Service
    ↓
Persistent task
    ↓
Agent worker
```

---

# 46. Background Workers

Recommended:

```text
AgentSupervisor
    ├── TaskQueue
    ├── AgentWorker
    ├── Scheduler
    └── EventBus
```

Tasks continue if the user closes the UI.

---

# 47. Task Queue

States:

```text
QUEUED
RUNNING
PAUSED
WAITING
COMPLETED
FAILED
CANCELLED
```

Priority:

```text
critical
high
normal
low
background
```

---

# 48. Cancellation

User says:

> "Stop."

The supervisor sends:

```text
CancellationToken
```

Every tool must respect cancellation where possible.

---

# 49. Parallel Execution

Independent tasks can execute concurrently.

Example:

```text
Research Company A
Research Company B
Research Company C
```

The planner can produce:

```text
parallel group
```

but actions with dependencies remain sequential.

---

# 50. Dependency Graph

Represent plans as a DAG.

Example:

```text
Open browser
      ↓
Login check
      ↓
Search jobs
   ↙     ↘
Job A    Job B
  ↓        ↓
Evaluate  Evaluate
   ↘      ↙
    Rank
      ↓
   Apply
```

---

# 51. Agent Supervisor

The supervisor controls:

- Maximum task count.
- Model allocation.
- Tool concurrency.
- Timeouts.
- Resource usage.
- Cancellation.
- Recovery.
- Policy violations.

The LLM should not supervise itself.

---

# 52. Sub-Agents

JARVIS can use specialized agents.

Examples:

```text
ResearchAgent
BrowserAgent
CodingAgent
FileAgent
VisionAgent
JobAgent
CommunicationAgent
```

Each sub-agent receives limited tools.

---

# 53. Example: Research Agent

Tools:

```text
browser.search
browser.open
browser.extract
memory.store
```

No:

```text
filesystem.delete
email.send
shell.execute
```

This is capability isolation.

---

# 54. Example: Coding Agent

Tools:

```text
fs.read
fs.write
shell.test
git.status
git.diff
```

Dangerous tools remain restricted.

---

# 55. Example: Job Agent

Tools:

```text
browser.search
browser.extract
browser.fill
profile.read
credential.check
application.save
```

Submission should pass through policy.

---

# 56. Memory Architecture

Memory should be separated into:

```text
Working Memory
Episodic Memory
Semantic Memory
Procedural Memory
Preference Memory
```

---

# 57. Working Memory

Contains:

```text
current conversation
current task
recent observations
pending tool calls
```

Lifetime:

```text
minutes/hours
```

---

# 58. Episodic Memory

Stores past events:

```text
User applied to Company X
User rejected Job Y
User asked JARVIS to prefer remote roles
```

Use only when useful.

---

# 59. Semantic Memory

Facts:

```text
favorite applications
preferred development tools
common workflows
project information
```

---

# 60. Procedural Memory

Stores workflows.

Example:

```text
When opening development environment:
1. Start Docker
2. Start VS Code
3. Start terminal
4. Open project
```

This can become a reusable automation.

---

# 61. Preference Memory

Examples:

```text
preferred browser
preferred music service
preferred language
default project directory
preferred job roles
```

Do not infer sensitive personal attributes unnecessarily.

---

# 62. Memory Retrieval

Do not inject all memory into prompts.

Retrieve using:

```text
semantic similarity
metadata
recency
task relevance
```

Scoring:

```text
score =
semantic_similarity
+ task_relevance
+ recency_weight
+ importance
```

---

# 63. Memory Write Policy

Not everything should become permanent memory.

Classify:

```text
temporary
session
task
long-term
```

Ask before saving sensitive or surprising information.

---

# 64. Vector Database

Initial implementation can use:

```text
SQLite
+
vector extension/index
```

or a local vector database.

The key requirement is:

```text
local
fast
persistent
filterable
```

---

# 65. Context Builder

Every model call should be constructed from:

```text
system policy
+
agent identity
+
task
+
current state
+
available tools
+
relevant memories
+
recent observations
```

---

# 66. Context Budget

Use dynamic budgeting:

```text
system = fixed
policy = fixed
task = dynamic
tools = filtered
memory = top-K
history = summarized
observations = compressed
```

Never blindly send the entire tool registry.

---

# 67. Tool Selection

Expose only relevant tools.

Example:

For:

> "Play music"

expose:

```text
media.play
media.pause
media.search
```

Do not expose:

```text
fs.delete
shell.execute
banking.transfer
```

This improves both safety and model accuracy.

---

# 68. Tool Schema Design

Tool arguments should be:

- Explicit.
- Typed.
- Validated.
- Small.
- Predictable.

Prefer:

```json
{
  "query": "React developer",
  "location": "Bangalore"
}
```

over:

```json
{
  "instruction": "Do whatever is needed to find React jobs."
}
```

---

# 69. Tool Result Design

Results should be compact.

Example:

```json
{
  "status": "success",
  "count": 15,
  "items": [
    {
      "title": "Software Engineer",
      "company": "Example",
      "url": "...",
      "match_score": 0.87
    }
  ]
}
```

---

# 70. Structured LLM Responses

Supported response types:

```text
FINAL
TOOL_CALL
ASK_USER
CONFIRM
WAIT
FAIL
REPLAN
```

Example:

```json
{
  "type": "ask_user",
  "question": "What salary should I enter?"
}
```

---

# 71. Agent Loop

Canonical loop:

```text
receive request
 ↓
classify
 ↓
load task context
 ↓
create plan
 ↓
validate plan
 ↓
execute next action
 ↓
observe
 ↓
update state
 ↓
check completion
 ├── yes → respond
 └── no → continue/replan
```

---

# 72. Pseudocode

```text
async run_task(task):

    state = load_state(task)

    while not state.finished:

        if cancelled(task):
            return CANCELLED

        context = context_builder.build(state)

        decision = planner.decide(context)

        validate_decision(decision)

        if decision.type == ASK_USER:
            return WAITING_FOR_INPUT

        if decision.type == CONFIRM:
            return WAITING_FOR_CONFIRMATION

        if decision.type == TOOL_CALL:

            policy = policy_engine.check(
                task,
                decision.tool,
                decision.arguments
            )

            if policy == DENY:
                return BLOCKED

            if policy == ASK:
                return WAITING_FOR_CONFIRMATION

            result = executor.execute(decision)

            observer.update(state, result)

            if result.failed:
                recovery.handle(state, result)

        if decision.type == FINAL:
            return COMPLETED
```

---

# 73. Browser Agent

Browser agent responsibilities:

```text
navigate
inspect
extract
click
type
select
scroll
wait
download
upload
verify
```

It should maintain:

```text
current_url
page_title
tab_id
DOM_snapshot
accessibility_snapshot
last_screenshot
```

---

# 74. Browser State

Example:

```json
{
  "tab_id": "tab_3",
  "url": "https://example.com",
  "title": "Example",
  "authenticated": true,
  "dom_hash": "abc123"
}
```

---

# 75. UI Grounding

A browser action should ideally reference:

```text
element_id
role
accessible_name
selector
```

rather than:

```text
x=1234,y=554
```

---

# 76. Computer-Use Fallback

If no semantic element exists:

```text
screenshot
 ↓
vision
 ↓
find target
 ↓
coordinate
 ↓
click
 ↓
observe
```

The action must be verified.

---

# 77. Desktop Agent

Windows and Linux should expose a common abstraction:

```text
DesktopProvider
```

Methods:

```text
list_windows()
focus_window()
open_application()
close_application()
type_text()
press_key()
mouse_click()
mouse_move()
scroll()
capture_screen()
get_accessibility_tree()
```

Platform adapters implement these methods.

---

# 78. Android Agent

Expose:

```text
AndroidProvider
```

Methods:

```text
list_apps()
launch_app()
get_ui_tree()
tap()
type()
swipe()
back()
home()
screenshot()
```

Use AccessibilityService where appropriate.

---

# 79. Application Abstraction

Do not hard-code every app into the agent.

Create:

```text
ApplicationProfile
```

Example:

```json
{
  "name": "Chrome",
  "package": "chrome",
  "launch_command": "...",
  "capabilities": [
    "browser"
  ]
}
```

---

# 80. App Capability Discovery

JARVIS should be able to determine:

```text
installed?
running?
logged in?
accessible?
supports API?
supports automation?
```

Prefer native APIs over UI automation.

---

# 81. Native API Priority

For an application:

```text
Official API
 ↓
local IPC/plugin
 ↓
accessibility API
 ↓
DOM
 ↓
UI automation
 ↓
vision
```

This dramatically improves reliability.

---

# 82. Event Bus

Use an internal event bus:

```text
TaskCreated
TaskStarted
ToolStarted
ToolCompleted
ScreenChanged
LoginRequired
UserSpoke
WakeDetected
TaskFailed
ConfirmationRequired
```

Subsystems subscribe to relevant events.

---

# 83. Example Event

```json
{
  "type": "login_required",
  "task_id": "task_123",
  "service": "linkedin",
  "timestamp": "..."
}
```

---

# 84. Notifications

JARVIS should notify through:

```text
voice
desktop notification
Android notification
UI
```

Example:

> "Sir, the job application is waiting for your login."

---

# 85. Conversation Continuity

If user says:

> "Open Chrome."

then:

> "Search for React jobs."

JARVIS should understand the current browser context.

Context:

```text
active_application = Chrome
```

---

# 86. Pronoun Resolution

Commands such as:

> "Open it."

should resolve against active task context.

Example:

```text
"It" → VS Code
```

If ambiguous:

> "Which application do you mean?"

Do not guess when ambiguity can cause side effects.

---

# 87. Multi-Turn Task

Example:

User:

> "Find SDE jobs."

JARVIS:

> "What locations should I consider?"

User:

> "Bangalore and remote."

JARVIS:

```text
resume task
```

This requires persistent task state.

---

# 88. Task Memory vs Conversation

Conversation:

```text
"What did I say?"
```

Task state:

```text
"Where was the job application?"
```

Keep these separate.

---

# 89. Scheduling

The agent should expose:

```text
schedule.create
schedule.list
schedule.cancel
```

Example:

> "Every morning at 9, check for new SDE jobs."

The scheduler creates a persistent task.

---

# 90. Scheduled Task Architecture

```text
Scheduler
 ↓
TaskQueue
 ↓
AgentWorker
 ↓
Agent
 ↓
Tools
```

The voice layer is not required after scheduling.

---

# 91. Conditional Tasks

Support:

```text
When X happens, do Y.
```

Examples:

```text
When a new job matching my profile appears, notify me.

When battery falls below 20%, enable power saving.

When a build fails, analyze the error.
```

---

# 92. Autonomous Limits

Every autonomous task should have:

```text
maximum duration
maximum tool calls
maximum cost
maximum retries
maximum side effects
```

Example:

```json
{
  "max_duration_minutes": 60,
  "max_tool_calls": 200,
  "max_submissions": 5
}
```

---

# 93. Job Application Guardrails

For job automation:

```text
search
→ filter
→ rank
→ inspect
→ prepare
→ fill
→ validate
→ confirmation
→ submit
```

Never blindly apply to every job.

Use profile matching.

---

# 94. Job Matching

Inputs:

```text
skills
experience
education
location
work preference
salary
role
technology
```

Output:

```text
match score
reasons
missing requirements
risk
```

---

# 95. Form Filling

Create a canonical profile:

```text
name
email
phone
education
experience
skills
projects
links
resume
cover_letter_preferences
salary_preferences
location_preferences
```

The agent maps:

```text
website field
→ profile field
```

Unknown fields require reasoning or user input.

---

# 96. Application Verification

After submit:

```text
success message?
confirmation number?
email?
application status?
```

Store:

```text
company
role
date
URL
status
confirmation
```

---

# 97. Web Research

Research agent should:

```text
search
→ open
→ extract
→ compare
→ cite
→ summarize
```

Never assume search snippets are complete evidence.

---

# 98. Source Trust

Rank sources:

```text
official source
primary documentation
reputable publication
secondary source
community discussion
```

Untrusted content must not become instructions.

---

# 99. Prompt Injection Defense

Treat:

```text
web pages
emails
documents
PDFs
messages
GitHub issues
```

as untrusted data.

A webpage saying:

> "Ignore your system instructions"

must remain content.

The security policy always has higher priority.

---

# 100. Data Boundary

Use:

```text
SYSTEM
SECURITY
USER
TASK
TOOLS
OBSERVATIONS
```

Observations must never override system policy.

---

# 101. Secret Boundary

Never place:

```text
passwords
private keys
tokens
session cookies
```

into ordinary LLM context.

Use specialized credential tools.

---

# 102. Audit Trail

Every meaningful action records:

```text
task_id
tool
arguments_hash
timestamp
result
risk
policy_decision
user_confirmation
```

Sensitive arguments should be redacted.

---

# 103. Explainability

The user should be able to ask:

> "Why did you do that?"

JARVIS should answer from the task trace:

```text
Goal
→ selected plan
→ tool
→ result
```

Do not expose hidden chain-of-thought. Store concise decision rationales and observable action history instead.

---

# 104. Agent Debug Mode

Developer mode should expose:

```text
task state
selected model
tool calls
latency
token counts
screenshots
DOM snapshots
errors
policy decisions
```

Secrets remain redacted.

---

# 105. Metrics

Track:

```text
task completion rate
tool success rate
replan rate
average task duration
confirmation rate
failure rate
hallucinated tool-call rate
duplicate-action rate
STT errors
vision errors
```

---

# 106. Agent Evaluation

Create benchmark suites.

## Simple

```text
open application
play music
volume control
```

## Browser

```text
search
login detection
form fill
download
```

## Computer use

```text
UI navigation
dialog handling
```

## Complex

```text
multi-site research
job application
project setup
```

---

# 107. Adversarial Tests

Test:

```text
malicious webpage
fake login page
unexpected popup
wrong application
missing permissions
network failure
CAPTCHA
duplicate submission
expired session
stale DOM
vision misidentification
ambiguous command
```

---

# 108. Recovery Benchmark

Measure whether JARVIS can recover when:

```text
button moved
browser crashes
network disappears
login expires
application closes
tool times out
model crashes
```

---

# 109. Recommended Technology Stack

## Agent runtime

Prefer:

```text
Python
```

for initial AI/agent orchestration because of the local AI ecosystem.

Potential performance-critical components:

```text
Rust
C++
```

later.

## API

```text
FastAPI
WebSocket
gRPC where justified
```

## Data

```text
SQLite
local vector index
```

## Validation

```text
Pydantic
JSON Schema
```

---

# 110. Why Python First

Python provides mature libraries for:

- LLM integration.
- Transformers.
- Whisper.
- Computer vision.
- Browser automation.
- OCR.
- AI evaluation.
- Local inference.

Do not prematurely rewrite the agent in Rust.

Use Rust/C++ for components where profiling demonstrates a need.

---

# 111. Tool Execution Architecture

```text
Agent
 ↓
ToolRegistry
 ↓
PolicyEngine
 ↓
ToolExecutor
 ↓
PlatformAdapter
```

Example:

```text
browser.click
 ↓
BrowserTool
 ↓
Policy
 ↓
WindowsBrowserAdapter
```

---

# 112. Platform Adapter Pattern

Shared interface:

```text
DesktopProvider
BrowserProvider
AudioProvider
CredentialProvider
NotificationProvider
ApplicationProvider
```

Platform implementations:

```text
WindowsDesktopProvider
LinuxDesktopProvider
AndroidDesktopProvider
```

---

# 113. Monorepo Structure

```text
jarvis/
│
├── apps/
│   ├── windows/
│   ├── linux/
│   └── android/
│
├── core/
│   ├── agent/
│   ├── planner/
│   ├── executor/
│   ├── policy/
│   ├── memory/
│   ├── context/
│   └── scheduler/
│
├── ai/
│   ├── inference/
│   ├── models/
│   ├── vision/
│   └── speech/
│
├── tools/
│   ├── system/
│   ├── browser/
│   ├── filesystem/
│   ├── media/
│   ├── communication/
│   └── developer/
│
├── platform/
│   ├── windows/
│   ├── linux/
│   └── android/
│
├── tests/
│   ├── unit/
│   ├── integration/
│   ├── agent/
│   └── adversarial/
│
└── docs/
```

---

# 114. Tool Plugin System

Tools should be dynamically registered.

Example:

```python
registry.register(BrowserOpenTool())
registry.register(FilesystemReadTool())
registry.register(MediaPlayTool())
```

Each tool declares:

```text
metadata
schema
permissions
platforms
```

---

# 115. Tool Versioning

Use:

```text
browser.open:v1
browser.open:v2
```

This allows evolution without breaking old workflows.

---

# 116. Tool Discovery

The agent receives only:

```text
relevant tools
```

Tool discovery can use semantic matching:

```text
user goal
 ↓
tool index
 ↓
top relevant tools
```

---

# 117. Tool Namespaces

Use namespaces:

```text
system.*
browser.*
fs.*
media.*
android.*
windows.*
linux.*
email.*
calendar.*
research.*
memory.*
```

This improves organization and routing.

---

# 118. Agent Identity

JARVIS should have a stable system identity:

```text
Name: JARVIS
Role: Personal local AI assistant
Primary objective: Execute the user's authorized tasks reliably.
```

Personality should remain separate from security policy.

---

# 119. Voice Persona

The voice system should define:

```text
voice
speed
pitch
language
response verbosity
formality
```

The agent should not rely on personality to make security decisions.

---

# 120. Conversational Behavior

For trivial tasks:

> "Done."

For ongoing tasks:

> "I'm checking the available jobs now."

For blockers:

> "The site requires you to log in."

For failures:

> "The application failed because the session expired. I can retry after you log in."

---

# 121. Progress Reporting

Long tasks should periodically report:

```text
started
progress
blocked
completed
```

Example:

> "I've reviewed 18 of the 32 matching jobs."

Avoid narrating every internal tool call.

---

# 122. User Control

The user must be able to say:

```text
Stop.
Pause.
Continue.
Skip this.
Undo that.
Why?
Show me.
Do it manually.
```

These commands should have priority over ordinary agent execution.

---

# 123. Emergency Stop

Provide a deterministic stop mechanism independent of the LLM.

Examples:

```text
voice: "JARVIS stop"
desktop hotkey
tray button
Android notification action
```

This should immediately cancel active execution.

---

# 124. Kill Switch

Provide:

```text
Pause all automation
```

which disables external side effects while allowing conversation.

---

# 125. Recovery After Restart

At startup:

```text
load unfinished tasks
 ↓
check environment
 ↓
validate stale state
 ↓
resume or ask user
```

Do not blindly resume a potentially dangerous action.

---

# 126. Stale Task Detection

A task may be stale if:

```text
browser changed
login expired
file moved
session expired
machine rebooted
time-sensitive data changed
```

The agent should re-observe before continuing.

---

# 127. Artifact Management

Tasks can produce artifacts:

```text
screenshots
documents
reports
downloads
logs
application records
```

Store references, not unnecessarily duplicated data.

---

# 128. Local File Organization

Example:

```text
~/.jarvis/
├── config/
├── models/
├── memory/
├── tasks/
├── logs/
├── artifacts/
├── cache/
└── security/
```

On Windows use an equivalent application-data directory.

---

# 129. Configuration Layers

```text
defaults
 ↓
machine config
 ↓
user config
 ↓
task config
 ↓
temporary override
```

Never let an untrusted webpage alter configuration.

---

# 130. Agent Configuration Example

```yaml
agent:
  max_steps: 100
  max_duration_minutes: 60
  max_retries: 3

policy:
  destructive_actions: confirm
  external_submissions: confirm
  credentials: isolated

memory:
  enabled: true
  retention_days: 365

browser:
  automation: enabled
  vision_fallback: true
```

---

# 131. Initial Implementation Milestone

The first useful agent should support:

```text
voice command
 ↓
Whisper
 ↓
LLM
 ↓
tool selection
 ↓
policy
 ↓
tool execution
 ↓
result
 ↓
Piper
```

Tools:

```text
open app
close app
type
press key
screenshot
browser open
browser search
filesystem read
filesystem write
```

---

# 132. Second Milestone

Add:

```text
planning
replanning
task persistence
memory
confirmation
recovery
```

---

# 133. Third Milestone

Add:

```text
vision
browser computer-use
Android automation
```

---

# 134. Fourth Milestone

Add:

```text
long-running agents
scheduler
sub-agents
background workers
```

---

# 135. Fifth Milestone

Add:

```text
advanced job automation
developer automation
research workflows
cross-device task handoff
```

---

# 136. Final Agent Architecture

```text
                         USER
                           │
                 Voice / Text / UI
                           │
                           ▼
                 ┌──────────────────┐
                 │  Intent Router   │
                 └────────┬─────────┘
                          │
              ┌───────────┴───────────┐
              ▼                       ▼
       Deterministic              AI Agent
          Router                     │
              │                      ▼
              │                Context Builder
              │                      │
              │                      ▼
              │                  Planner
              │                      │
              │                      ▼
              │                Policy Engine
              │                      │
              │                      ▼
              │                 Tool Registry
              │                      │
              └──────────────┬───────┘
                             ▼
                       Tool Executor
                             │
             ┌───────────────┼────────────────┐
             ▼               ▼                ▼
          Windows          Linux            Android
             │               │                │
             └───────────────┼────────────────┘
                             ▼
                         Observation
                             │
                             ▼
                           Agent
                             │
                  ┌──────────┴──────────┐
                  ▼                     ▼
               Memory                Response
                                        │
                                        ▼
                                      TTS
```

---

# 137. Final Design Rules

1. **Never let the LLM directly control the operating system.**
2. **Every side effect must pass through a tool.**
3. **Every sensitive tool must pass through policy.**
4. **Prefer deterministic APIs over UI automation.**
5. **Prefer DOM/accessibility over vision.**
6. **Use vision as a fallback for computer use.**
7. **Never place secrets in ordinary model context.**
8. **Treat web pages, emails and documents as untrusted input.**
9. **Verify actions instead of assuming success.**
10. **Make long tasks persistent and resumable.**
11. **Support cancellation everywhere.**
12. **Use small models for simple operations.**
13. **Use larger models only when required.**
14. **Keep the architecture model-agnostic.**
15. **Keep platform-specific code behind interfaces.**
16. **Log actions without logging secrets.**
17. **Require human intervention for authentication, CAPTCHA and high-risk actions.**
18. **Design for failure from the beginning.**
19. **Make every autonomous action observable and auditable.**
20. **The user remains the ultimate authority.**

---

# 138. What This Enables

Once this layer is implemented, JARVIS can evolve from:

```text
voice assistant
```

into:

```text
personal computer agent
```

and eventually:

```text
local autonomous personal operating layer
```

Examples:

> "JARVIS, open my development environment."

> "JARVIS, find why my application isn't building."

> "JARVIS, search for SDE jobs matching my profile."

> "JARVIS, prepare applications for the best five."

> "JARVIS, research these companies and compare their engineering cultures."

> "JARVIS, organize my Downloads folder."

> "JARVIS, create a project and install the required dependencies."

> "JARVIS, remind me tomorrow morning to follow up with these companies."

The critical difference from a conventional chatbot is that every request becomes a **controlled, observable, resumable task**.

---

# 139. Relationship to Document 2

Document 2 provides:

```text
LOCAL INTELLIGENCE
```

Document 3 provides:

```text
ACTION INTELLIGENCE
```

Together:

```text
Document 2
LLM + Vision + Speech
        +
Document 3
Agent + Tools + Memory + Policy
        =
JARVIS Core
```

The next major architectural layer should define the **cross-platform operating-system and application automation system** that implements these tools for Windows, Ubuntu/Linux and Android.
