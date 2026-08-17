# JARVIS — Document 13
# Agent / Planner / Workflow Engine

**Document status:** Detailed implementation specification  
**Purpose:** Define the reasoning, planning, task execution, workflow, recovery, and orchestration layer that turns natural-language JARVIS requests into safe, persistent, verifiable actions.

---

## 1. Purpose

The Agent / Planner / Workflow Engine is the brain between:

```text
User
 ↓
Voice/Text Interface
 ↓
Intent Understanding
 ↓
Agent / Planner
 ↓
Workflow Engine
 ↓
Policy
 ↓
Skills / Tools
 ↓
Computer / Browser / OS / Android / APIs
 ↓
Verification
 ↓
Planner
 ↓
Response
```

The engine answers:

- What does the user actually want?
- What capabilities are required?
- What information is missing?
- What should happen first?
- Which tool or skill should perform each step?
- Which steps can run in parallel?
- Which steps require confirmation?
- What happens if an action fails?
- What state must be persisted?
- How should the task resume after interruption?
- How does JARVIS know that the task actually succeeded?

The fundamental rule is:

> **The LLM proposes plans. The workflow engine owns execution.**

The model must not have unrestricted authority over the operating system.

---

# 2. Example

User:

> "Jarvis, find suitable SDE jobs in Bangalore, fill applications for the ones matching my profile, and ask me before submitting."

The system should create:

```text
Task
 ├── Load profile
 ├── Open job platform
 ├── Verify authentication
 ├── Search jobs
 ├── Extract jobs
 ├── Score jobs
 ├── Select candidates
 ├── Open applications
 ├── Fill known fields
 ├── Detect unknown questions
 ├── Prepare application
 ├── Ask confirmation
 ├── Submit
 └── Verify submission
```

The user should not need to know any of these internal steps.

---

# 3. Core Design Principles

1. LLMs reason; deterministic systems execute.
2. Every task has explicit state.
3. Every side effect is policy checked.
4. Every important action has verification.
5. Failed actions trigger bounded recovery.
6. Long tasks are checkpointed.
7. Tasks can be cancelled.
8. Tasks can be resumed.
9. Tools are capability-limited.
10. Web content is untrusted.
11. User data is provided on a minimum-required basis.
12. High-risk actions require explicit authorization.
13. The system must never claim success without evidence.
14. Background work must not block interactive commands.
15. The architecture must work locally first.

---

# 4. Agent Architecture

Recommended:

```text
                    JARVIS CORE
                         │
                ┌────────┴────────┐
                │                 │
             Planner            Policy
                │                 │
                └────────┬────────┘
                         │
                    Task Manager
                         │
                ┌────────┴─────────┐
                │                  │
             Workflow            Tools
              Engine               │
                │                  │
         ┌──────┼──────┐     ┌─────┼─────┐
         ▼      ▼      ▼     ▼     ▼     ▼
      Browser  OS   Android  API  Skills Memory
         │      │      │
         └──────┴──────┘
                │
            Verification
                │
             Recovery
```

---

# 5. Agent vs Workflow

These are different concepts.

## Agent

Handles:

```text
uncertainty
reasoning
planning
tool selection
replanning
```

## Workflow

Handles:

```text
known sequence
state
dependencies
retries
timeouts
approval
persistence
```

JARVIS needs both.

---

# 6. Why Pure Agent Architecture Is Not Enough

A fully autonomous loop:

```text
LLM → tool → LLM → tool → ...
```

can:

- loop indefinitely,
- repeat actions,
- hallucinate success,
- lose state,
- perform unintended actions,
- exceed budgets.

Therefore use:

```text
Agent
+
Workflow Engine
+
Policy
+
Verification
```

---

# 7. Task Model

Every user request becomes a task.

Example:

```json
{
  "task_id": "task_01JARVIS",
  "user_goal": "Find SDE jobs",
  "status": "RUNNING",
  "priority": "INTERACTIVE",
  "created_at": "...",
  "deadline": null,
  "parent_task_id": null
}
```

---

# 8. Task Status

Recommended states:

```text
CREATED
QUEUED
PLANNING
RUNNING
WAITING_FOR_TOOL
WAITING_FOR_USER
WAITING_FOR_CONFIRMATION
PAUSED
RECOVERING
COMPLETED
FAILED
CANCELLED
EXPIRED
UNKNOWN
```

---

# 9. Task Identity

Each task needs:

```text
task_id
user_id
device_id
session_id
parent_task_id
workflow_id
workflow_version
```

For a single-user local JARVIS, `user_id` can initially be a local installation identity.

---

# 10. Task Context

A task context contains:

```text
user request
current state
relevant memory
active tools
permissions
workflow
observations
previous results
pending questions
pending confirmations
```

It should not contain the entire personal knowledge base.

---

# 11. Context Budget

The planner should receive only information relevant to the current step.

Bad:

```text
entire conversation
entire memory
entire DOM
all application data
```

Better:

```text
current goal
current workflow state
relevant observation
required profile fields
recent decisions
```

---

# 12. Context Layers

Use:

```text
System Context
 ↓
Policy Context
 ↓
Task Context
 ↓
Step Context
 ↓
Tool Observation
```

Each layer has different trust.

---

# 13. Trust Hierarchy

Recommended:

```text
System Policy
    ↓
User Intent
    ↓
Application Policy
    ↓
Workflow State
    ↓
Tool Results
    ↓
External Content
```

External webpage text must never override policy.

---

# 14. Intent Understanding

The first model call converts:

> "Jarvis, find me some good SDE jobs and apply to them."

into a structured intent.

Example:

```json
{
  "goal": "find_and_apply_jobs",
  "role": "software_development_engineer",
  "location": "unspecified",
  "application_mode": "prepare_and_confirm",
  "source": "user"
}
```

If required information is missing, ask before executing.

---

# 15. Intent vs Plan

Intent:

```text
Find SDE jobs.
```

Plan:

```text
1. Determine preferred location.
2. Open job source.
3. Search SDE.
4. Filter.
5. Rank.
6. Prepare applications.
```

Never confuse the two.

---

# 16. Planner Responsibilities

The planner decides:

- task decomposition,
- tool choice,
- ordering,
- conditional branching,
- when to ask,
- when to confirm,
- when to stop,
- when to replan.

It does not directly execute OS primitives.

---

# 17. Executor Responsibilities

The executor:

- validates tool calls,
- checks permissions,
- executes,
- captures results,
- verifies outcomes,
- emits events,
- updates state.

---

# 18. Policy Responsibilities

The policy layer:

- checks capability permissions,
- classifies risk,
- enforces confirmation,
- blocks prohibited operations,
- enforces domain restrictions,
- enforces resource limits.

Policy must remain deterministic.

---

# 19. Workflow Engine

The workflow engine maintains:

```text
nodes
edges
state
conditions
retries
timeouts
outputs
```

A workflow is a stateful execution graph.

---

# 20. Workflow Graph

Example:

```text
START
  │
  ▼
LOAD_PROFILE
  │
  ▼
SEARCH_JOBS
  │
  ▼
SCORE_JOBS
  │
  ▼
APPLICATIONS?
  │
 ┌┴───────┐
No        Yes
 │         │
DONE    FILL_FORM
           │
           ▼
       REVIEW
           │
           ▼
       CONFIRM
           │
           ▼
        SUBMIT
           │
           ▼
        VERIFY
```

---

# 21. Workflow Node

Example:

```json
{
  "node_id": "search_jobs",
  "type": "tool",
  "tool": "linkedin.search_jobs",
  "inputs": {
    "query": "software engineer"
  },
  "retry_policy": {
    "max_attempts": 2
  }
}
```

---

# 22. Node Types

Recommended:

```text
START
END
TOOL
LLM
CONDITION
PARALLEL
WAIT
HUMAN_INPUT
CONFIRMATION
TRANSFORM
SUBWORKFLOW
CHECKPOINT
```

---

# 23. Tool Node

Example:

```text
browser.navigate
```

The workflow engine invokes the tool through the capability layer.

---

# 24. LLM Node

An LLM node should be used for:

```text
classification
ranking
summarization
ambiguity resolution
planning
structured extraction
```

Not for arbitrary privileged execution.

---

# 25. Condition Node

Example:

```text
if authenticated:
    continue
else:
    request login
```

Conditions should preferably be deterministic.

---

# 26. Human Input Node

Used when:

```text
required data is missing
MFA required
CAPTCHA required
ambiguous choice
```

---

# 27. Confirmation Node

Used before high-impact operations.

Example:

```text
Submit application?
```

The confirmation is enforced by policy, not merely suggested by the model.

---

# 28. Parallel Node

Example:

```text
Search LinkedIn
Search company careers
Search another job source
```

can run in parallel if:

```text
no shared mutable resource
```

---

# 29. Checkpoint Node

Stores:

```text
workflow state
outputs
current node
important observations
```

This allows resume after restart.

---

# 30. Workflow Variables

Example:

```text
job_results
selected_jobs
profile
application_answers
confirmation_id
```

Variables should be typed.

---

# 31. Typed Workflow Data

Use schemas such as Pydantic models.

Example:

```python
class Job:
    title: str
    company: str
    location: str
    url: str
```

Avoid arbitrary unvalidated dictionaries everywhere.

---

# 32. Tool Registry

All tools register capabilities.

Example:

```text
browser.navigate
browser.click
browser.type
linkedin.search_jobs
linkedin.apply
filesystem.read
filesystem.write
media.play
android.send_notification
```

---

# 33. Tool Metadata

Every tool should declare:

```json
{
  "name": "browser.click",
  "description": "Click a browser target",
  "risk": "LOW",
  "requires_confirmation": false,
  "capabilities": ["browser.input"]
}
```

---

# 34. Capability-Based Security

A tool does not automatically get access to everything.

Example:

```text
linkedin.apply
```

may have:

```text
browser.read
browser.input
profile.read
```

but not:

```text
filesystem.delete
terminal.execute
credential.export
```

---

# 35. Tool Schema

Use structured tool definitions:

```json
{
  "name": "browser.click",
  "input_schema": {
    "type": "object",
    "properties": {
      "target": {
        "type": "object"
      }
    },
    "required": ["target"]
  }
}
```

---

# 36. Tool Validation

Before execution:

```text
schema validation
 ↓
policy validation
 ↓
resource validation
 ↓
execution
```

---

# 37. Tool Result

Standardize tool output:

```json
{
  "success": true,
  "data": {},
  "error": null,
  "observation_id": "obs_123",
  "side_effect": false
}
```

---

# 38. Error Result

```json
{
  "success": false,
  "error": {
    "code": "TARGET_NOT_FOUND",
    "message": "Apply button could not be located",
    "recoverable": true
  }
}
```

The planner can then select recovery.

---

# 39. ReAct-Style Loop

A useful execution pattern:

```text
Observe
 ↓
Reason
 ↓
Act
 ↓
Observe
 ↓
Verify
 ↓
Continue / Replan
```

But reasoning should be represented through structured model outputs rather than exposing hidden chain-of-thought.

---

# 40. Structured Planning

Ask the model for:

```json
{
  "goal": "...",
  "steps": [
    {
      "id": "step1",
      "action": "linkedin.search_jobs",
      "inputs": {}
    }
  ]
}
```

The system executes only validated steps.

---

# 41. Plan Validation

Before execution:

```text
schema
 ↓
tool existence
 ↓
permissions
 ↓
dependencies
 ↓
risk
 ↓
resource budget
```

Invalid plans are rejected.

---

# 42. Plan Execution

Do not execute every planned step blindly.

Use:

```text
step
 ↓
observe
 ↓
execute
 ↓
verify
 ↓
update state
 ↓
next step
```

---

# 43. Replanning

Replan when:

- website changed,
- authentication expired,
- tool failed,
- user changed requirements,
- new information appeared,
- expected result was not observed.

---

# 44. Replanning Example

Plan:

```text
Search LinkedIn
```

Result:

```text
LinkedIn unavailable
```

Planner may choose:

```text
company career pages
```

if the user's goal allows alternate sources.

---

# 45. Goal Preservation

Replanning must preserve the original user goal.

It should not silently change:

```text
"find SDE jobs"
```

into:

```text
"find any jobs"
```

---

# 46. Goal Constraints

Represent:

```text
must
should
prefer
avoid
never
```

Example:

```text
must: software engineering
prefer: Bangalore
avoid: unpaid
never: fabricate experience
```

---

# 47. Constraint Store

The task should maintain:

```json
{
  "hard_constraints": [],
  "soft_preferences": [],
  "prohibited_actions": []
}
```

---

# 48. Clarification

Ask only when necessary.

Bad:

> "What would you like me to do?"

after the user already gave a clear task.

Good:

> "Which locations should I target for the SDE jobs?"

---

# 49. Clarification Budget

Avoid asking one question at a time if multiple required fields are missing.

Ask:

> "I need two things before I start: your preferred locations and minimum salary."

---

# 50. User Preference Memory

Stable preferences may be retrieved from memory.

Example:

```text
preferred job locations
resume choice
preferred browser
music service
```

Sensitive information should require stricter access.

---

# 51. Planner Context Retrieval

The planner can request:

```text
memory.search("job application preferences")
```

rather than loading the entire memory store.

---

# 52. Model Routing

Use different models for different tasks.

Example:

```text
Fast model
 → classification
 → simple tool selection

Reasoning model
 → complex planning

Vision model
 → UI interpretation

Embedding model
 → retrieval
```

---

# 53. Local-First Routing

Preferred:

```text
local model
```

for normal operations.

Fallback:

```text
larger local model
```

then optionally:

```text
remote model
```

only if the user explicitly enables cloud fallback.

---

# 54. Planner Model

The planner model should support:

- structured output,
- tool calling,
- instruction following,
- long context,
- reasoning,
- low hallucination rate.

The actual model selection is defined in Document 2.

---

# 55. Fast Executor Model

Use a smaller model for:

```text
simple classification
routing
short extraction
```

This reduces latency.

---

# 56. No Need for Multiple Agents Everywhere

Do not create:

```text
research agent
browser agent
memory agent
email agent
music agent
```

for every tiny operation.

Use one orchestrator with specialized skills unless specialization provides real value.

---

# 57. When Multi-Agent Makes Sense

Use separate agents when:

- domains are substantially different,
- tasks run independently,
- specialized context is large,
- independent evaluation is useful.

Example:

```text
Research Agent
Security Review Agent
Execution Agent
```

---

# 58. Recommended Multi-Agent Architecture

```text
                    Supervisor
                        │
          ┌─────────────┼─────────────┐
          ▼             ▼             ▼
      Research       Planning       Execution
       Agent           Agent          Agent
          │             │             │
          └─────────────┼─────────────┘
                        ▼
                      Policy
```

However, keep the supervisor deterministic where possible.

---

# 59. Avoid Agent-to-Agent Chaos

Do not allow agents to recursively spawn unlimited agents.

Set:

```text
max_depth
max_agents
max_tokens
max_time
```

---

# 60. Background Agents

JARVIS can support persistent background workflows:

```text
job monitoring
price monitoring
system health
calendar reminders
```

These should be represented as scheduled workflows, not infinite LLM loops.

---

# 61. Trigger System

Triggers may be:

```text
voice
text
startup
schedule
notification
file event
application event
device event
```

---

# 62. Event-Driven Agent Execution

Example:

```text
New email
 ↓
event
 ↓
workflow matcher
 ↓
agent
 ↓
summarize
 ↓
notify user
```

---

# 63. Scheduled Workflow

Example:

```text
Every weekday at 9 AM:
Search for new SDE jobs.
```

Workflow:

```text
scheduler
 ↓
create task
 ↓
planner
 ↓
job search skill
 ↓
summarize
 ↓
notification
```

---

# 64. Task Scheduler

The scheduler should not call the LLM directly.

Instead:

```text
scheduler
 ↓
task creation
 ↓
normal task engine
```

This keeps execution consistent.

---

# 65. Task Persistence

Use a local database.

Initial recommendation:

**SQLite**

Later, if needed:

**PostgreSQL**

---

# 66. SQLite Tables

Possible:

```text
tasks
workflow_instances
workflow_nodes
task_events
tool_calls
confirmations
checkpoints
scheduled_tasks
task_outputs
```

---

# 67. Tasks Table

Example:

```sql
tasks(
    id,
    goal,
    status,
    priority,
    created_at,
    updated_at,
    deadline,
    parent_id
)
```

---

# 68. Task Events

Use append-only events:

```text
TASK_CREATED
PLAN_CREATED
STEP_STARTED
TOOL_CALLED
TOOL_RESULT
STEP_VERIFIED
USER_REQUESTED
CONFIRMATION_CREATED
CONFIRMATION_APPROVED
TASK_COMPLETED
TASK_FAILED
```

---

# 69. Event Sourcing

Full event sourcing is not required initially.

A hybrid approach is better:

```text
current state tables
+
append-only event log
```

This gives observability without unnecessary complexity.

---

# 70. Checkpoint Storage

Checkpoint:

```json
{
  "task_id": "...",
  "workflow_version": "1.2",
  "current_node": "fill_form",
  "variables": {},
  "completed_nodes": []
}
```

---

# 71. Resume

After restart:

```text
load task
 ↓
load checkpoint
 ↓
validate workflow version
 ↓
re-observe external state
 ↓
resume
```

Never blindly assume the external world is unchanged.

---

# 72. Workflow Versioning

Every workflow instance stores:

```text
workflow_id
workflow_version
```

If the workflow changes later, old tasks should continue using a compatible version or undergo explicit migration.

---

# 73. Idempotency

Every side-effecting tool should support an idempotency mechanism where possible.

Example:

```text
send email
```

should have:

```text
idempotency_key
```

to prevent accidental duplicates.

---

# 74. Browser Idempotency

Before repeating:

```text
submit application
```

check whether the application already exists.

---

# 75. File Idempotency

Before:

```text
create file
```

check:

```text
existing path
checksum
task ownership
```

---

# 76. API Idempotency

If an API supports idempotency keys, use them.

If not, verify state before retrying.

---

# 77. Retry Policy

Every tool can specify:

```text
max_attempts
backoff
retryable_errors
```

Example:

```text
network timeout → retry
invalid credentials → do not retry blindly
permission denied → stop
target not found → re-observe
```

---

# 78. Exponential Backoff

For transient errors:

```text
1s
2s
4s
8s
```

with a maximum.

---

# 79. Retry Classification

Errors should be classified:

```text
TRANSIENT
RECOVERABLE
USER_ACTION_REQUIRED
PERMISSION_DENIED
SECURITY_BLOCK
FATAL
UNKNOWN
```

---

# 80. Recovery Planner

A failed step can trigger:

```text
same action retry
different selector
different tool
alternate skill
alternate workflow
human handoff
abort
```

---

# 81. Recovery Must Respect Risk

For:

```text
send money
submit application
send message
```

do not automatically retry ambiguous external side effects.

First establish whether the original action occurred.

---

# 82. Unknown Outcome

Critical concept:

```text
UNKNOWN
```

Example:

```text
payment request timed out
```

Do not retry immediately.

First check whether payment succeeded.

---

# 83. Human Escalation

Escalate when:

```text
policy requires
confidence too low
state unknown
required information missing
authentication required
security anomaly
```

---

# 84. Confirmation Architecture

Confirmation is a first-class workflow node.

```text
WAITING_FOR_CONFIRMATION
```

contains:

```text
what
why
risk
target
consequence
expiry
```

---

# 85. Good Confirmation

> "The application for Software Engineer at Example Corp is filled and ready. It will submit your resume and answers to the employer. Submit it?"

---

# 86. Bad Confirmation

> "Continue?"

The user should know what side effect is about to occur.

---

# 87. Confirmation Binding

Confirmation must bind to:

```text
task_id
node_id
action
target
parameters
```

If any important parameter changes:

```text
new confirmation
```

---

# 88. Confirmation Expiration

Approvals should expire.

Example:

```text
5 minutes
```

for sensitive operations.

---

# 89. Confirmation Channels

Support:

```text
voice
desktop
Android
keyboard
```

The authorization layer validates the channel.

---

# 90. Voice Confirmation Security

For high-risk actions, voice alone may not be enough.

Use:

```text
voice confirmation
+
device presence
+
biometric confirmation
```

where appropriate.

---

# 91. User Interruptions

JARVIS is voice-driven.

User may say:

> "Stop."

or:

> "Actually, don't apply to those."

The active workflow must be interruptible.

---

# 92. Interrupt Semantics

```text
RUNNING
 ↓
INTERRUPTED
 ↓
PAUSED
```

The planner then determines:

```text
resume
modify
cancel
```

---

# 93. Voice Priority

Interactive voice commands should have highest priority.

Example:

```text
STOP > confirmation > new interactive request > background task
```

---

# 94. Cancellation

Cancellation should:

```text
stop planner
cancel current tool
release locks
close temporary resources
save checkpoint
```

---

# 95. Resume

Resume should:

```text
load checkpoint
 ↓
validate
 ↓
re-observe
 ↓
continue
```

---

# 96. Task Modification

User:

> "Actually, only apply to jobs above 8 LPA."

The workflow should update constraints and replan.

Do not restart the entire task unnecessarily.

---

# 97. Parallel Planning

If tasks are independent:

```text
Search LinkedIn
Search company sites
Search another source
```

run concurrently.

---

# 98. Dependency Graph

Represent:

```text
A → B
A → C
B,C → D
```

Then:

```text
A
├── B
└── C
    ↓
    D
```

---

# 99. DAG Validation

Before execution:

```text
detect cycles
validate dependencies
validate inputs
```

A workflow should not contain accidental infinite cycles.

---

# 100. Resource Locks

Example:

```text
Browser Context A
```

can only be owned by one interactive workflow at a time.

---

# 101. Lock Manager

Locks can include:

```text
browser:linkedin
desktop:chrome
microphone
camera
filesystem:path
android:device
```

---

# 102. Deadlock Prevention

Use:

```text
global lock ordering
timeouts
lease expiration
```

Avoid workflows holding locks unnecessarily.

---

# 103. Resource Arbitration

Example:

```text
Music task needs audio
Voice conversation needs microphone
```

The system should coordinate rather than allow conflicts.

---

# 104. Workflow Priorities

Suggested:

```text
CRITICAL
INTERACTIVE
USER_BACKGROUND
SCHEDULED
MAINTENANCE
```

---

# 105. Agent Budget

Each task should have:

```text
max duration
max LLM calls
max tokens
max tool calls
max VLM calls
max browser pages
```

---

# 106. Loop Detection

Track repeated states:

```text
same observation
same action
same error
```

If repeated:

```text
abort/replan/human
```

---

# 107. Example Infinite Loop

Bad agent:

```text
click Apply
fails
click Apply
fails
click Apply
...
```

The engine should detect:

```text
same target
same state
same failure
```

and stop.

---

# 108. State Fingerprints

Generate a compact fingerprint from:

```text
URL
page title
relevant DOM
accessibility tree
task variables
```

Repeated fingerprints can indicate a loop.

---

# 109. LLM Hallucination Containment

Never trust a model statement:

> "The application was submitted."

Require:

```text
tool result
+
verification
```

---

# 110. Evidence-Based Completion

Task completion should require evidence:

```text
verification.status == VERIFIED
```

Only then:

```text
COMPLETED
```

---

# 111. Planner Output

The model should return structured data.

Example:

```json
{
  "decision": "continue",
  "reason_code": "AUTHENTICATED",
  "next_action": {
    "tool": "linkedin.search_jobs",
    "arguments": {}
  }
}
```

Avoid unrestricted natural-language execution instructions.

---

# 112. Planner Rejection

If the model proposes:

```text
filesystem.delete("/")
```

the tool registry/policy rejects it before execution.

---

# 113. Tool Discovery

The planner should receive only relevant tools.

If the user asks:

> "Play music."

Do not expose:

```text
filesystem.delete
browser.upload
system.shutdown
```

unless needed.

---

# 114. Dynamic Tool Loading

Tool sets can be assembled:

```text
intent
 ↓
required capabilities
 ↓
tool registry
 ↓
filtered tools
 ↓
planner
```

This reduces context size and risk.

---

# 115. Skill Discovery

If the user asks:

> "Apply to jobs."

The planner can discover:

```text
job_search skill
linkedin skill
resume skill
browser skill
```

---

# 116. Skill Metadata

A skill should declare:

```yaml
name: linkedin_jobs
description: Search and prepare job applications
domains:
  - linkedin.com
capabilities:
  - browser.read
  - browser.input
  - profile.read
risk:
  search: LOW
  application_submit: HIGH
```

---

# 117. Workflow Composition

A high-level skill can call lower-level workflows.

Example:

```text
job_application
 ├── browser_login
 ├── search_jobs
 ├── score_jobs
 └── apply_job
```

---

# 118. Subworkflows

Subworkflows should have:

```text
inputs
outputs
permissions
timeout
version
```

---

# 119. Example

```text
apply_job(job, profile)
```

returns:

```json
{
  "status": "READY_FOR_CONFIRMATION",
  "application_id": "...",
  "summary": {}
}
```

---

# 120. Workflow Contracts

Every workflow should define:

```text
input schema
output schema
failure states
side effects
required permissions
```

---

# 121. Workflow Side-Effect Declaration

Example:

```yaml
side_effects:
  - external_form_submission
  - file_upload
```

This lets policy evaluate risk before execution.

---

# 122. Agent Memory

The agent should maintain:

```text
task memory
```

separate from:

```text
long-term personal memory
```

---

# 123. Task Memory

Contains:

```text
what has happened
what remains
temporary observations
decisions
errors
```

Destroyed or compacted after task completion according to retention policy.

---

# 124. Long-Term Memory

Examples:

```text
preferred job locations
preferred music service
frequently used applications
stable workflow preferences
```

Handled by Document 9.

---

# 125. Memory Write Policy

Do not automatically write every task detail to long-term memory.

Memory writes should be:

```text
explicit
or
policy-approved
```

---

# 126. RAG Integration

For knowledge questions:

```text
retrieve
 ↓
rank
 ↓
context
 ↓
LLM
```

For action tasks:

```text
retrieve relevant profile/preferences
 ↓
planner
```

---

# 127. Agent Context Compression

Long tasks can exceed context limits.

Compress:

```text
completed actions
old observations
redundant tool results
```

Keep:

```text
current state
constraints
important decisions
pending actions
errors
```

---

# 128. Summary Checkpoint

Example:

```text
Task summary:
- LinkedIn authenticated.
- 23 jobs found.
- 5 matched profile.
- 2 applications prepared.
- Job #3 rejected because experience requirement was too high.
- Waiting for user approval for Job #1.
```

---

# 129. Planning Horizons

Do not always plan the entire task.

Use:

```text
short horizon
```

for dynamic environments.

Example:

```text
plan 2–5 actions
execute
observe
replan
```

---

# 130. Long-Horizon Tasks

For predictable tasks:

```text
workflow graph
```

can contain many nodes, while the LLM only decides local choices.

This gives both reliability and flexibility.

---

# 131. Deterministic vs Agentic Workflows

Use deterministic workflows for:

```text
known application forms
known APIs
scheduled tasks
system maintenance
```

Use agentic planning for:

```text
unknown websites
ambiguous goals
research
novel tasks
```

---

# 132. Hybrid Strategy

Best architecture:

```text
User Goal
 ↓
LLM planner
 ↓
select workflow
 ↓
deterministic execution
 ↓
LLM only for uncertainty
```

This reduces cost and failure.

---

# 133. Workflow Templates

Examples:

```text
play_music
search_web
send_message
apply_for_job
create_file
summarize_email
book_appointment
system_diagnostics
```

---

# 134. Workflow Registry

Store:

```text
workflow ID
version
schema
implementation
permissions
tests
```

---

# 135. Workflow Validation at Startup

JARVIS should validate:

```text
workflow schemas
tool references
dependency graph
permissions
version compatibility
```

before accepting tasks.

---

# 136. Agent Observability

Every task should produce a trace:

```text
task
 ├── planner call
 ├── tool call
 ├── observation
 ├── verification
 ├── recovery
 └── completion
```

---

# 137. Trace IDs

Use:

```text
trace_id
task_id
workflow_id
node_id
tool_call_id
```

for debugging.

---

# 138. Metrics

Track:

```text
task completion rate
planning latency
tool latency
LLM latency
replan count
retry count
human handoff rate
confirmation rate
failure rate
unknown-state rate
```

---

# 139. Agent Evaluation

Create a benchmark suite.

Example tasks:

```text
Open app
Play music
Search web
Fill form
Find job
Prepare application
Ask for missing field
Recover from browser crash
Reject malicious webpage
Stop immediately
Resume task
```

---

# 140. Evaluation Dimensions

Measure:

```text
correctness
safety
latency
tool efficiency
recovery
user interruptions
resource usage
```

---

# 141. Safety Evaluation

Tests must verify:

```text
no unauthorized submit
no credential leakage
no destructive action
no prompt-injection compliance
no infinite loops
no silent failure
```

---

# 142. Planner Unit Tests

Mock:

```text
tool registry
policy
workflow
observations
```

Test:

```text
correct tool
correct branching
correct confirmation
correct recovery
```

---

# 143. Workflow Unit Tests

Test deterministic state transitions without invoking an LLM.

---

# 144. Tool Integration Tests

Test:

```text
actual browser
actual OS adapter
actual local model
```

in controlled environments.

---

# 145. End-to-End Tests

Example:

```text
voice command
 ↓
STT
 ↓
planner
 ↓
browser
 ↓
verification
 ↓
TTS
```

These should run on dedicated test machines.

---

# 146. Simulation Mode

JARVIS should support:

```text
DRY_RUN
```

where actions are planned but not executed.

Example:

> "I would open LinkedIn, search for SDE jobs, and prepare three applications."

This is invaluable during development.

---

# 147. Shadow Mode

Another mode:

```text
observe real UI
generate actions
do not execute
```

Useful for evaluating planner quality safely.

---

# 148. Human Approval Mode

Development mode:

```text
approve every action
```

Example:

```text
JARVIS wants to click "Apply".

[Approve] [Reject]
```

This helps debug computer use.

---

# 149. Production Modes

Recommended:

```text
SAFE
INTERACTIVE
AUTOMATED
BACKGROUND
```

Each has different policy thresholds.

---

# 150. Safe Mode

No external side effects.

---

# 151. Interactive Mode

Normal JARVIS behavior.

High-impact actions require confirmation.

---

# 152. Automated Mode

For trusted workflows.

Still subject to hard security policies.

---

# 153. Background Mode

Tasks run without blocking the voice assistant.

Sensitive operations should still request attention.

---

# 154. Planner Failure

If planning fails:

```text
retry with smaller context
 ↓
use fallback model
 ↓
ask user
```

Do not execute an incomplete plan.

---

# 155. Model Failure

If local planner model crashes:

```text
restart model runtime
 ↓
retry
 ↓
fallback model
```

The workflow engine should survive model failure.

---

# 156. Tool Failure

Tool failures must not crash JARVIS Core.

Use isolated tool workers where appropriate.

---

# 157. Worker Isolation

Recommended:

```text
JARVIS Core
 ├── AI worker
 ├── Browser worker
 ├── Desktop worker
 └── Android bridge
```

---

# 158. Process Isolation

High-risk tools should be separate processes.

This limits crashes and permissions.

---

# 159. Local IPC

Use:

```text
Unix domain sockets
Windows named pipes
gRPC/local TLS
```

depending on subsystem.

---

# 160. Agent API

Core API:

```text
create_task()
get_task()
pause_task()
resume_task()
cancel_task()
approve_confirmation()
reject_confirmation()
```

---

# 161. Planner API

```text
plan(task_context)
replan(task_context, failure)
classify(intent)
select_workflow(intent)
```

---

# 162. Workflow API

```text
start(workflow)
pause(instance)
resume(instance)
cancel(instance)
get_state(instance)
```

---

# 163. Tool API

```text
discover()
describe()
validate()
execute()
cancel()
```

---

# 164. Event API

```text
subscribe()
publish()
```

Events should be typed.

---

# 165. Example Task Lifecycle

```text
USER
 ↓
CREATE TASK
 ↓
CLASSIFY INTENT
 ↓
RETRIEVE CONTEXT
 ↓
SELECT WORKFLOW
 ↓
PLAN
 ↓
VALIDATE
 ↓
EXECUTE
 ↓
OBSERVE
 ↓
VERIFY
 ↓
CHECK POLICY
 ↓
CONTINUE / REPLAN / ASK
 ↓
COMPLETE
```

---

# 166. Example: Play Music

User:

> "Jarvis, play some relaxing music."

Flow:

```text
Intent
 ↓
media.play
 ↓
select music source
 ↓
execute
 ↓
verify playback
 ↓
"Playing relaxing music."
```

No complex agent required.

---

# 167. Example: Open VS Code

```text
intent
 ↓
application.launch
 ↓
focus
 ↓
verify window
 ↓
complete
```

Deterministic workflow.

---

# 168. Example: Write a Document

User:

> "Create a project proposal and save it in Documents."

Flow:

```text
understand request
 ↓
generate content
 ↓
create file
 ↓
validate path
 ↓
save
 ↓
verify file
```

If an existing file would be overwritten:

```text
confirmation
```

---

# 169. Example: Search the Web

```text
intent
 ↓
research workflow
 ↓
browser
 ↓
search
 ↓
collect sources
 ↓
extract
 ↓
summarize
```

---

# 170. Example: Job Application

```text
intent
 ↓
job workflow
 ↓
profile retrieval
 ↓
browser
 ↓
authentication
 ↓
search
 ↓
ranking
 ↓
application
 ↓
unknown questions
 ↓
human input
 ↓
review
 ↓
confirmation
 ↓
submit
 ↓
verification
```

---

# 171. Example: System Diagnostic

User:

> "Jarvis, my PC is running slowly. Find out why."

Planner:

```text
collect CPU
collect RAM
collect disk
collect GPU
collect processes
inspect startup apps
diagnose
```

System changes should require separate authorization.

---

# 172. Example: Cross-Device Task

User:

> "Jarvis, I'm leaving. Continue the job search on my PC and tell me on my phone when applications are ready."

Flow:

```text
Android
 ↓
task update
 ↓
PC task continues
 ↓
confirmation state
 ↓
Android notification
 ↓
user approves
 ↓
PC submits
```

---

# 173. Supervisor

The supervisor coordinates:

```text
tasks
agents
workflows
resources
priorities
```

It should be mostly deterministic.

---

# 174. Agent Loop

Conceptually:

```text
while task.active:

    observe_state()

    if waiting_for_user:
        pause()

    if policy_block:
        escalate()

    decision = planner()

    validate(decision)

    result = executor(decision)

    verify(result)

    update_state()

    if success:
        continue

    if recoverable:
        replan()

    else:
        fail_or_escalate()
```

The actual implementation should use explicit state transitions rather than an uncontrolled `while` loop.

---

# 175. State Machine

At the core:

```text
CREATED
  ↓
PLANNING
  ↓
READY
  ↓
EXECUTING
  ↓
VERIFYING
  ├── SUCCESS → NEXT
  ├── RECOVER → RECOVERING
  ├── ASK → WAITING
  └── FAILURE → FAILED
```

---

# 176. Package Structure

Recommended:

```text
packages/
└── agent/
    ├── core/
    │   ├── task.py
    │   ├── state.py
    │   ├── events.py
    │   └── context.py
    │
    ├── planner/
    │   ├── planner.py
    │   ├── routing.py
    │   ├── prompts/
    │   └── schemas.py
    │
    ├── workflow/
    │   ├── engine.py
    │   ├── graph.py
    │   ├── nodes.py
    │   ├── checkpoint.py
    │   └── versioning.py
    │
    ├── tools/
    │   ├── registry.py
    │   ├── schemas.py
    │   └── permissions.py
    │
    ├── policy/
    │   ├── engine.py
    │   ├── risk.py
    │   └── confirmation.py
    │
    ├── recovery/
    │   ├── retry.py
    │   ├── recovery.py
    │   └── loop_detection.py
    │
    ├── scheduler/
    │   ├── scheduler.py
    │   └── triggers.py
    │
    └── persistence/
        ├── models.py
        ├── repository.py
        └── migrations/
```

---

# 177. Recommended Technologies

For the first implementation:

```text
Python
Pydantic
asyncio
SQLite
SQLAlchemy
FastAPI or local RPC
async task queues
structured LLM output
```

The exact AI runtime is defined in Document 2.

---

# 178. Why Python

Python is practical because JARVIS also needs:

- local model integration,
- computer vision,
- Playwright,
- OCR,
- agent orchestration,
- embeddings,
- RAG.

Native OS components can still be implemented in Rust/Kotlin/C++ where required.

---

# 179. Database Layer

Use:

```text
SQLAlchemy
```

or another typed persistence layer.

Avoid scattering SQL throughout agent code.

---

# 180. Repository Pattern

Use:

```text
TaskRepository
WorkflowRepository
EventRepository
ConfirmationRepository
```

The agent engine should not know SQL details.

---

# 181. Event Bus

Use an internal event bus initially.

Possible implementation:

```text
asyncio queues
```

Later:

```text
NATS
Redis Streams
```

only if scale requires it.

---

# 182. Do Not Overengineer Early

Initial JARVIS should not require:

```text
Kafka
Kubernetes
microservices everywhere
distributed databases
```

It is a local personal assistant.

Start with:

```text
single machine
multiple processes
SQLite
local IPC
```

---

# 183. Process Model

Initial:

```text
jarvis-core
jarvis-ai
jarvis-browser
jarvis-desktop
jarvis-voice
```

These can run as local services.

---

# 184. Why Separate Processes

Benefits:

- crash isolation,
- security boundaries,
- model restarts,
- browser restarts,
- independent resource limits.

---

# 185. Agent Startup

At system startup:

```text
JARVIS daemon
 ↓
load configuration
 ↓
detect hardware
 ↓
start AI runtime
 ↓
start voice runtime
 ↓
start task manager
 ↓
register tools
 ↓
health checks
 ↓
READY
```

---

# 186. Agent Shutdown

Gracefully:

```text
stop accepting new tasks
 ↓
finish safe operations
 ↓
pause long tasks
 ↓
save checkpoints
 ↓
shutdown workers
```

---

# 187. Unexpected Shutdown

On restart:

```text
load incomplete tasks
 ↓
mark as RECOVERY_REQUIRED
 ↓
re-observe external systems
 ↓
resume only if safe
```

---

# 188. Configuration

Use:

```text
config.yaml
```

or structured TOML.

Separate:

```text
defaults
user settings
security policy
device configuration
model configuration
```

---

# 189. Secrets

Never store secrets in normal config.

Use:

```text
Windows Credential Manager
Linux Secret Service / keyring
Android Keystore
```

The agent only requests access through the credential subsystem.

---

# 190. Security Boundary

Agent planner should not directly read:

```text
credential database
```

Instead:

```text
credential tool
 ↓
policy
 ↓
secure operation
```

---

# 191. Prompt Architecture

Separate prompts:

```text
planner system prompt
tool-selection prompt
workflow extraction prompt
summarization prompt
vision prompt
```

Do not create one enormous prompt for everything.

---

# 192. Prompt Versioning

Store:

```text
prompt_id
version
model
task type
```

This makes regressions traceable.

---

# 193. Structured Outputs

All important LLM outputs should use schemas.

Examples:

```text
Intent
Plan
Decision
ToolArguments
Summary
Clarification
```

---

# 194. JSON Validation

If model returns invalid JSON:

```text
repair/structured-output retry
```

Do not execute malformed output.

---

# 195. Model Context

The planner should receive:

```text
goal
constraints
state
available tools
current observation
relevant memory
policy summary
```

Not:

```text
entire internal database
```

---

# 196. Tool Choice

Use tool descriptions that are:

```text
short
precise
non-overlapping
```

Ambiguous tools increase model errors.

---

# 197. Tool Naming

Prefer:

```text
browser.navigate
browser.observe
browser.click
```

rather than:

```text
do_browser_thing
```

---

# 198. Tool Composition

High-level skills should compose low-level tools.

Example:

```text
linkedin.apply
```

internally:

```text
browser.open
browser.observe
browser.click
browser.type
browser.upload
```

---

# 199. Agent Guardrails

Before each side effect:

```text
Does this action:
- match task?
- have permission?
- satisfy policy?
- have valid parameters?
- exceed budget?
- require confirmation?
```

---

# 200. Side Effect Classification

Every tool should declare:

```text
READ_ONLY
LOCAL_MUTATION
EXTERNAL_SIDE_EFFECT
DESTRUCTIVE
PRIVILEGED
```

---

# 201. Example

```text
browser.observe
→ READ_ONLY

browser.type
→ LOCAL_INPUT

browser.submit
→ EXTERNAL_SIDE_EFFECT

filesystem.delete
→ DESTRUCTIVE

system.shutdown
→ PRIVILEGED
```

---

# 202. Policy Decision

A policy decision should return:

```json
{
  "allowed": true,
  "requires_confirmation": false,
  "reason": "LOW_RISK"
}
```

or:

```json
{
  "allowed": false,
  "reason": "PRIVILEGE_REQUIRED"
}
```

---

# 203. Policy Must Be Deterministic

Do not ask the LLM:

> "Is this safe?"

The policy engine decides.

The LLM can explain the action.

---

# 204. User Authorization

Authorization can be:

```text
one-time
task-scoped
workflow-scoped
skill-scoped
persistent
```

Persistent authorization should be rare for high-risk operations.

---

# 205. Example Trusted Workflow

User may configure:

```text
Allow JARVIS to open Spotify and play music without confirmation.
```

This can become a policy rule.

---

# 206. Example Non-Trusted Workflow

Job application submission:

```text
always confirm
```

even if everything else is automated.

---

# 207. Planner and Security

If the model says:

```text
submit
```

but policy says:

```text
confirmation required
```

the workflow enters:

```text
WAITING_FOR_CONFIRMATION
```

---

# 208. User-Facing Narration

The agent should communicate at meaningful milestones:

```text
Starting
Found result
Need input
Waiting for approval
Completed
Failed
```

Avoid narrating every low-level action.

---

# 209. Narration Events

The workflow engine can emit:

```text
USER_VISIBLE_PROGRESS
USER_ACTION_REQUIRED
USER_CONFIRMATION_REQUIRED
TASK_COMPLETED
TASK_FAILED
```

The voice subsystem handles TTS.

---

# 210. Progress Reporting

Example:

> "I found 18 jobs. Five match your criteria. I'm preparing the first two applications."

---

# 211. Error Reporting

Do not say:

> "ToolException 0x91."

Say:

> "LinkedIn stopped responding, so I paused the application rather than risking a duplicate submission."

---

# 212. Developer Error

Internally retain:

```text
exception
stack
tool
node
trace
```

---

# 213. Task Result

A completed task should return:

```json
{
  "status": "COMPLETED",
  "summary": "...",
  "outputs": [],
  "evidence": []
}
```

---

# 214. Evidence

For a job application:

```text
application confirmation page
application ID
timestamp
company
job
```

If available.

---

# 215. No False Success

If evidence is missing:

```text
status = UNKNOWN
```

not:

```text
COMPLETED
```

---

# 216. Long-Running Workflow

Example:

```text
Apply to 20 jobs
```

should checkpoint after each application.

---

# 217. Batch Safety

For a batch:

```text
Job 1
 ↓
verify
 ↓
Job 2
 ↓
verify
```

Do not batch 20 submissions into one opaque action.

---

# 218. Batch Confirmation

Possible:

> "I have prepared applications for five jobs. Do you want me to submit all five?"

The system must list the scope clearly.

---

# 219. Partial Failure

If:

```text
5 applications
3 submitted
1 failed
1 blocked
```

the task result should reflect exactly that.

---

# 220. Partial Result

```json
{
  "completed": 3,
  "failed": 1,
  "waiting": 1
}
```

---

# 221. Workflow Compensation

Some operations can be reversed.

Example:

```text
create draft
```

can potentially be deleted.

Others cannot:

```text
send email
submit application
transfer money
```

The workflow should know whether an operation is reversible.

---

# 222. Compensation Metadata

```yaml
action:
  reversible: false
```

This affects recovery strategy.

---

# 223. Transaction Boundary

For multi-step external workflows:

```text
prepare
 ↓
review
 ↓
commit
```

Treat commit as the high-risk boundary.

---

# 224. Two-Phase Interaction

Example:

```text
Fill application
 ↓
Review
 ↓
User approves
 ↓
Submit
```

This pattern should be reused broadly.

---

# 225. Planning with Uncertainty

The planner should explicitly represent:

```text
known
unknown
assumption
```

Example:

```text
Known: user has a resume.
Unknown: desired salary.
Assumption: Bangalore is preferred.
```

An assumption that affects a high-impact action should be resolved.

---

# 226. Assumption Policy

Low-risk:

```text
assume default browser
```

High-risk:

```text
assume salary
```

must ask.

---

# 227. Confidence

Planner decisions can have confidence metadata.

However, confidence alone should never override policy.

---

# 228. Planning Cost

The planner should optimize:

```text
quality
latency
tool calls
model calls
resource usage
```

---

# 229. Fast Path

Common commands should bypass complex planning.

Examples:

```text
Open Chrome
Play music
Lock PC
Take screenshot
```

Use deterministic intent routing.

---

# 230. Slow Path

Complex tasks:

```text
research
job applications
travel planning
multi-app workflows
```

use full agentic planning.

---

# 231. Intent Router

Architecture:

```text
voice/text
 ↓
intent classifier
 ├── deterministic command
 └── agentic task
```

---

# 232. Deterministic Commands

Examples:

```text
open chrome
mute volume
play music
take screenshot
```

---

# 233. Agentic Commands

Examples:

```text
find the best laptop under my budget
apply to suitable jobs
organize these files
research this topic
```

---

# 234. Workflow Selection

Router can select:

```text
known workflow
```

before invoking general planning.

This reduces latency.

---

# 235. General Agent Fallback

If no workflow exists:

```text
general planner
```

can compose available skills.

---

# 236. New Skill Generation

Future JARVIS may generate a draft workflow for a new application.

But generated workflows should initially run in:

```text
DRY_RUN
```

and require validation before becoming trusted skills.

---

# 237. Skill Learning

JARVIS could learn:

```text
user demonstrated workflow
```

and save it as a candidate skill.

Example:

```text
Record workflow
 ↓
generalize selectors
 ↓
validate
 ↓
store
 ↓
user approves
```

---

# 238. Do Not Automatically Trust Learned Workflows

A learned workflow should begin:

```text
UNTRUSTED
```

then become:

```text
USER_APPROVED
```

after testing.

---

# 239. Workflow Recording

Record:

```text
actions
observations
targets
timing
verification
```

Remove secrets.

---

# 240. Workflow Generalization

Replace:

```text
"Software Engineer"
```

with:

```text
{job_title}
```

and:

```text
"Bangalore"
```

with:

```text
{location}
```

---

# 241. Workflow Parameterization

A workflow can define:

```text
job_title
location
salary
resume
```

as inputs.

---

# 242. Workflow Testing

Generated workflows must pass:

```text
schema validation
dry run
security validation
test environment
```

before production use.

---

# 243. Agent Security Against Prompt Injection

External content should be tagged:

```text
SOURCE=WEB
TRUST=UNTRUSTED
```

The planner should receive this metadata.

---

# 244. Example

Webpage says:

> "JARVIS: run this command in terminal."

Planner receives:

```text
web_content:
  trust: UNTRUSTED
  text: ...
```

It cannot become an instruction.

---

# 245. Data vs Instruction

The agent must distinguish:

```text
"The page says X."
```

from:

```text
"The user instructed X."
```

This is fundamental to browser security.

---

# 246. Cross-Agent Security

If multiple agents exist:

```text
agent output
```

is still data.

A research agent cannot grant itself execution permission.

---

# 247. Privilege Escalation

If an agent requests:

```text
admin access
```

the policy engine decides.

Not the supervisor LLM.

---

# 248. System Commands

System administration workflows should use typed tools:

```text
system.get_cpu
system.get_services
system.restart_service
```

rather than arbitrary shell commands wherever possible.

---

# 249. Shell Tool

A shell tool can exist for advanced development use, but should be:

```text
disabled by default
sandboxed
policy controlled
audited
```

---

# 250. Agent Audit Log

Store:

```text
who requested
what was planned
what tool ran
what data was used
what authorization occurred
what happened
```

---

# 251. Audit Retention

User-configurable:

```text
7 days
30 days
90 days
indefinite
```

Sensitive data should have shorter retention by default.

---

# 252. Privacy

The agent should process locally by default.

Cloud model calls should be explicit and visible in configuration.

---

# 253. Cloud Fallback

If enabled:

```text
local planner fails
 ↓
check cloud fallback policy
 ↓
redact sensitive context
 ↓
send minimal data
```

---

# 254. Local-Only Mode

JARVIS should support:

```text
OFFLINE / LOCAL_ONLY
```

where:

```text
no external model calls
```

are permitted.

---

# 255. Offline Degradation

Without internet:

```text
local LLM
local STT
local TTS
local browser for local apps
offline workflows
```

continue to function where possible.

---

# 256. Network-Aware Planning

The planner should know:

```text
internet available
```

or:

```text
offline
```

and choose tools accordingly.

---

# 257. Model Availability

The agent should know:

```text
planner model ready
vision model ready
STT ready
TTS ready
```

through capability discovery.

---

# 258. Capability Registry

At startup:

```text
register capabilities
 ↓
health check
 ↓
publish available tools
```

---

# 259. Degraded Mode

If VLM unavailable:

```text
DOM/OCR automation
```

can continue.

If LLM unavailable:

```text
deterministic commands
```

can still work.

---

# 260. Graceful Degradation

JARVIS should remain useful even when parts fail.

---

# 261. Example

Vision model crashes:

> "My visual interface module is unavailable. I can still operate websites through their accessibility interface."

---

# 262. Agent Startup Dependency Graph

```text
Core
 ├── Policy
 ├── Database
 ├── Event Bus
 └── Tool Registry
       ├── AI
       ├── Browser
       ├── Desktop
       ├── Voice
       └── Android
```

---

# 263. Health States

Each component:

```text
READY
DEGRADED
UNAVAILABLE
ERROR
```

---

# 264. Planner Capability Awareness

If:

```text
browser unavailable
```

planner should not propose browser actions unless recovery is possible.

---

# 265. Task Admission Control

Before starting a task:

```text
Can required capabilities be satisfied?
```

If no:

```text
explain limitation
```

rather than starting a doomed workflow.

---

# 266. Example

User:

> "Open LinkedIn."

Browser unavailable.

JARVIS:

> "My browser automation service is unavailable right now. I haven't started the task."

---

# 267. Agent Scheduling

Interactive tasks should preempt background tasks where safe.

---

# 268. Background Preemption

Example:

```text
Job monitor running
 ↓
user asks urgent task
 ↓
pause job monitor
 ↓
execute interactive task
 ↓
resume
```

---

# 269. Fairness

Background tasks should not starve forever.

Use:

```text
priority + aging
```

---

# 270. Resource Budgets

Per task:

```text
CPU
RAM
GPU
VRAM
LLM calls
VLM calls
network
disk
browser instances
```

---

# 271. Agent Resource Manager

```text
Task
 ↓
ResourceManager
 ↓
allocation
 ↓
execution
```

---

# 272. GPU Arbitration

If local LLM and VLM both need GPU:

```text
scheduler
```

decides whether to:

```text
queue
offload
use smaller model
run sequentially
```

---

# 273. Agent Latency

Voice interactions should have:

```text
fast acknowledgement
```

before long planning.

Example:

> "Understood. I'm checking your job sources now."

Then work continues asynchronously.

---

# 274. Streaming Progress

The workflow can emit:

```text
PROGRESS
```

events.

The voice layer can narrate selected events.

---

# 275. Avoid Voice Spam

Only narrate:

```text
important milestone
user action
error
completion
```

---

# 276. Agent Conversation State

Conversation context and task state are related but not identical.

Example:

```text
Conversation:
"Yes, do it."

Task:
confirmation_123 approved
```

The approval should be resolved against the active confirmation.

---

# 277. Ambiguous "Yes"

If multiple confirmations exist:

> "Yes."

must not be accepted automatically.

Ask:

> "Do you mean the application for Example Corp?"

---

# 278. Conversation-to-Task Binding

Each confirmation prompt contains:

```text
active_task_id
confirmation_id
```

The voice router uses these to resolve responses.

---

# 279. Multiple Tasks

User may have:

```text
Task A: job search
Task B: music
Task C: file organization
```

The conversation manager must identify which task is being referenced.

---

# 280. Task Addressing

User can say:

> "Stop the job search."

The intent router resolves:

```text
task_type = job_search
```

---

# 281. Task Listing

User:

> "What are you doing?"

JARVIS can report:

```text
1. Preparing job applications.
2. Monitoring a download.
3. Playing music.
```

---

# 282. Task Control Commands

Support:

```text
stop
pause
resume
cancel
continue
show status
switch task
```

---

# 283. Task Persistence Across Devices

The same task ID can be synchronized:

```text
PC
 ↓
task state
 ↓
Android
```

Detailed cross-device design is Document 11/next cross-device document.

---

# 284. Agent API for Android

Android can request:

```text
task status
pause
resume
approve
cancel
```

It should not directly execute privileged PC tools.

---

# 285. Remote Confirmation

Android may receive:

```text
Submit job application?
```

The PC still performs the action after authorization.

---

# 286. Authorization Binding

Approval should be cryptographically associated with:

```text
task
action
device
user
expiry
```

---

# 287. Workflow Notifications

Examples:

```text
TASK_NEEDS_INPUT
TASK_NEEDS_CONFIRMATION
TASK_COMPLETED
TASK_FAILED
```

---

# 288. Notification Priority

High-risk confirmation:

```text
HIGH
```

routine progress:

```text
LOW
```

---

# 289. Agent Implementation Milestones

## Milestone 1

Implement:

```text
Task
Tool Registry
Planner
Basic executor
```

---

## Milestone 2

Implement:

```text
Workflow graph
State machine
Persistence
```

---

## Milestone 3

Implement:

```text
Policy
Confirmation
Human handoff
```

---

## Milestone 4

Implement:

```text
Recovery
Retries
Checkpoints
```

---

## Milestone 5

Implement:

```text
Browser/desktop integration
```

---

## Milestone 6

Implement:

```text
Background scheduler
Parallel execution
Resource manager
```

---

## Milestone 7

Implement:

```text
Observability
Evaluation
Security testing
```

---

# 290. First Prototype

Start with one deterministic workflow:

```text
"Open Chrome and search for React jobs."
```

Implement:

```text
intent
 ↓
workflow
 ↓
browser.navigate
 ↓
browser.type
 ↓
browser.press
 ↓
verify
```

---

# 291. Second Prototype

Add:

```text
tool calling
```

The model selects:

```text
browser.navigate
browser.type
```

but policy/executor controls execution.

---

# 292. Third Prototype

Add:

```text
dynamic replanning
```

If search UI changes:

```text
observe
 ↓
planner
 ↓
new target
```

---

# 293. Fourth Prototype

Add:

```text
human confirmation
```

for a harmless test side effect.

---

# 294. Fifth Prototype

Implement:

```text
long-running workflow
checkpoint
restart
resume
```

---

# 295. Sixth Prototype

Implement:

```text
multi-tasking
background scheduler
```

---

# 296. Seventh Prototype

Implement:

```text
multi-device task state
```

---

# 297. Production Agent Architecture

```text
                        USER
                         │
                 Voice / Text / Android
                         │
                         ▼
                   Intent Router
                         │
             ┌───────────┴───────────┐
             │                       │
        Fast Command            Agentic Task
             │                       │
             └───────────┬───────────┘
                         ▼
                    Task Manager
                         │
                         ▼
                     Planner
                         │
                    Plan Validator
                         │
                  Workflow Engine
                         │
               ┌─────────┴─────────┐
               │                   │
             Policy              Tools
               │                   │
               └─────────┬─────────┘
                         ▼
                    Executors
                         │
        ┌────────────────┼─────────────────┐
        ▼                ▼                 ▼
     Browser           Desktop          Android
        │                │                 │
        └────────────────┼─────────────────┘
                         ▼
                     Verify
                         │
               ┌─────────┴─────────┐
               ▼                   ▼
            Success              Recovery
               │                   │
               └─────────┬─────────┘
                         ▼
                    Task State
                         │
                         ▼
                     Response
```

---

# 298. Recommended Final Design

The JARVIS agent should use a **hybrid architecture**:

```text
Fast deterministic commands
        +
LLM planner
        +
deterministic workflow engine
        +
capability-based tools
        +
policy engine
        +
verification
        +
bounded recovery
        +
human confirmation
```

This is substantially safer and more reliable than attempting to build JARVIS as a single autonomous LLM loop.

---

# 299. Critical Rules

1. Never allow the LLM to directly execute arbitrary shell commands.
2. Never let the LLM bypass policy.
3. Never let webpage content become trusted instructions.
4. Never report success without verification.
5. Never retry ambiguous external side effects blindly.
6. Never allow unlimited agent loops.
7. Never allow unlimited agent spawning.
8. Never expose unnecessary credentials to models.
9. Never make high-risk confirmation merely advisory.
10. Always support cancellation.
11. Always checkpoint long-running workflows.
12. Always separate task state from long-term memory.
13. Always validate tool arguments.
14. Always enforce resource budgets.
15. Always preserve the user's original goal during replanning.
16. Prefer deterministic workflows for known tasks.
17. Use agentic planning for uncertainty.
18. Keep security policy outside the LLM.
19. Treat external content as untrusted.
20. Make the whole system local-first.

---

# 300. End State

When this document's architecture is implemented, JARVIS will no longer be:

```text
LLM + voice
```

It will be:

```text
                    JARVIS
                       │
              Understand the user
                       │
                 Create a task
                       │
              Select capabilities
                       │
                  Make a plan
                       │
              Validate the plan
                       │
                Execute safely
                       │
                Observe results
                       │
                 Verify outcome
                       │
          ┌────────────┴────────────┐
          │                         │
       Success                    Failure
          │                         │
          ▼                         ▼
       Continue                  Recover
          │                         │
          └────────────┬────────────┘
                       ▼
                 Update state
                       │
                       ▼
                Ask when needed
                       │
                       ▼
                  Complete
```

The key architectural principle is:

> **JARVIS should be an orchestrated local agent system, not an unrestricted autonomous model.**

The LLM provides intelligence and flexibility. The workflow engine provides determinism. The policy engine provides authority boundaries. The execution layer provides real-world computer control. Verification provides trust.
