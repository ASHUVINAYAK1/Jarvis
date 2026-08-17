# Project JARVIS
## Document 1 — Core Architecture, Monorepo, Runtime, IPC, Agent Engine and Extensibility Specification

**Project:** JARVIS — Local Multiplatform Personal AI Assistant

**Target Platforms:**

- Windows
- Ubuntu/Linux
- Android

**Document Purpose:**

Define the complete software foundation upon which the Windows, Linux, Android, AI, browser automation, memory, security, and application skill systems will be built.

---

# 1. Purpose of This Document

This document defines the **core platform** of JARVIS.

It answers:

- How should the repository be structured?
- Which component owns what?
- Which language should implement each component?
- How does the AI communicate with the operating system?
- How do tools work?
- How does the agent plan?
- How are tasks represented?
- How are long-running tasks resumed?
- How does Android communicate with the PC?
- How are plugins loaded?
- How are permissions enforced?
- How does the UI communicate with the daemon?
- How do we prevent the LLM from directly controlling privileged resources?
- How do we make the architecture replaceable and extensible?

This is the **architectural contract** for the project.

The later platform-specific documents should implement these contracts rather than redesigning them.

---

# 2. Architectural Philosophy

The central principle is:

> **The AI should never directly control the machine.**

Instead:

```text
User
  ↓
Interface
  ↓
JARVIS Core
  ↓
Agent
  ↓
Planner
  ↓
Tool Selection
  ↓
Policy Engine
  ↓
Tool Runtime
  ↓
Platform Adapter
  ↓
Operating System
```

The LLM is therefore a **reasoning component**, not a privileged operating-system component.

---

# 3. Core Design Principles

The architecture must satisfy these principles.

## 3.1 Local-first

The default execution path should be local.

```text
Voice
 ↓
Local STT
 ↓
Local LLM
 ↓
Local tools
 ↓
Local memory
 ↓
Local TTS
```

Cloud services are optional.

---

## 3.2 Model independence

The system must not depend on one particular LLM.

The following should be interchangeable:

- Ollama
- llama.cpp
- gpt-oss
- Qwen
- Gemma
- Mistral
- future models.

---

## 3.3 Platform independence

The core should not know how Windows clicks a button or how Android launches an activity.

It should ask for a capability:

```text
click(...)
launch_application(...)
read_screen(...)
```

The platform adapter implements the capability.

---

## 3.4 Security by architecture

Security cannot be an afterthought.

The system must prevent:

```text
LLM → unrestricted shell
LLM → unrestricted filesystem
LLM → unrestricted credentials
```

Instead:

```text
LLM
 ↓
Tool
 ↓
Policy
 ↓
Permission
 ↓
Execution
```

---

## 3.5 Observable

Every meaningful action should be observable.

We need to know:

- what the user asked,
- what the model decided,
- which tool was selected,
- what arguments were generated,
- whether permission was required,
- what happened,
- whether the action succeeded.

---

## 3.6 Recoverable

Tasks must survive:

- crashes,
- application restarts,
- network interruptions,
- model failures,
- device restarts.

---

## 3.7 Deterministic where possible

Do not invoke an LLM unnecessarily.

For:

> "Mute the volume."

Use a deterministic command.

For:

> "Research the best laptops under ₹80,000 and compare them."

Use an agent.

---

# 4. Complete Core Architecture

The core system will consist of:

```text
                         JARVIS CORE
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
     INTERFACE             AGENT                 MEMORY
        │                     │                     │
   ┌────┼────┐          ┌─────┼─────┐          ┌────┼────┐
   │    │    │          │     │     │          │    │    │
 Voice UI  API       Planner Executor Verifier  SQL Vector
        │                     │                     │
        └─────────────────────┼─────────────────────┘
                              │
                         TOOL SYSTEM
                              │
                    ┌─────────┼─────────┐
                    │         │         │
                  Browser   Desktop    Files
                    │         │         │
                    └─────────┼─────────┘
                              │
                        POLICY ENGINE
                              │
                        PERMISSION
                              │
                      PLATFORM ADAPTER
                              │
             ┌────────────────┼────────────────┐
             │                │                │
          Windows           Linux           Android
```

---

# 5. Major Components

The core consists of these components.

```text
1. JARVIS Daemon
2. API Gateway
3. Agent Runtime
4. Supervisor
5. Planner
6. Executor
7. Verifier
8. Tool Registry
9. Tool Runtime
10. Policy Engine
11. Permission Manager
12. Memory Engine
13. Context Engine
14. Workflow Engine
15. Event Bus
16. Scheduler
17. Model Gateway
18. Device Manager
19. Plugin Manager
20. Configuration Manager
21. Audit Logger
22. Voice Manager
23. Platform Manager
24. Task Manager
```

---

# 6. Process Architecture

The desktop system should not be one enormous process.

Recommended:

```text
                         OS
                          │
                ┌─────────┴─────────┐
                │                   │
          JARVIS DAEMON        CONTROL CENTER
                │                   │
        ┌───────┼───────┐          │
        │       │       │          │
      Agent   Tools   Memory        UI
        │       │
        │    Platform
        │    Adapter
        │
      Models
```

---

# 7. JARVIS Daemon

The daemon is the central runtime.

Suggested binary:

```text
jarvisd
```

Responsibilities:

- initialize services,
- manage the event bus,
- expose IPC,
- manage tasks,
- communicate with models,
- invoke tools,
- enforce policies,
- manage plugins,
- maintain state,
- manage devices.

It should run in the background.

---

# 8. Why a Daemon?

Without a daemon, every UI would need to implement:

- model access,
- memory,
- tools,
- permissions,
- device communication.

That would duplicate logic.

Instead:

```text
Windows UI ─┐
Linux UI ───┼──→ JARVIS Daemon
Android ────┘
```

The daemon becomes the authoritative execution engine.

---

# 9. Control Center

The desktop UI should be called:

```text
Jarvis Control Center
```

Responsibilities:

- conversation,
- current task,
- task history,
- permissions,
- settings,
- model configuration,
- memory management,
- plugin management,
- device management,
- logs,
- workflows.

It should **not** directly execute privileged actions.

---

# 10. API Gateway

The Control Center communicates with the daemon through a strongly typed API.

Recommended:

```text
gRPC + Protocol Buffers
```

For event streaming:

```text
gRPC streaming
```

or:

```text
WebSocket
```

The internal API should be language-neutral.

This allows:

```text
Rust daemon
Python AI
TypeScript UI
Kotlin Android
```

to communicate cleanly.

---

# 11. API Structure

Logical services:

```text
TaskService
AgentService
ToolService
MemoryService
DeviceService
PermissionService
WorkflowService
PluginService
ModelService
VoiceService
SystemService
EventService
```

---

# 12. Example API

Conceptually:

```protobuf
service TaskService {
    rpc CreateTask(CreateTaskRequest) returns (Task);
    rpc GetTask(GetTaskRequest) returns (Task);
    rpc CancelTask(CancelTaskRequest) returns (Task);
    rpc PauseTask(PauseTaskRequest) returns (Task);
    rpc ResumeTask(ResumeTaskRequest) returns (Task);
}
```

The exact protobuf definitions will be created during implementation.

---

# 13. Task Is the Fundamental Unit

Everything JARVIS does should be represented as a task.

Examples:

```text
"Open Chrome"
```

is a task.

```text
"Find SDE jobs and apply to five"
```

is also a task.

```text
"Research NVIDIA and create a report"
```

is a task.

---

# 14. Task Object

Conceptually:

```text
Task
├── id
├── parent_id
├── user_id
├── device_id
├── origin
├── goal
├── status
├── priority
├── created_at
├── updated_at
├── plan
├── current_step
├── context
├── permissions
├── result
└── error
```

---

# 15. Task States

A task must always have a state.

```text
CREATED
   ↓
UNDERSTANDING
   ↓
PLANNING
   ↓
WAITING_FOR_PERMISSION
   ↓
EXECUTING
   ↓
WAITING_FOR_USER
   ↓
VERIFYING
   ↓
COMPLETED
```

Alternative terminal states:

```text
FAILED
CANCELLED
EXPIRED
```

---

# 16. Task State Machine

```text
                    CREATED
                       │
                       ▼
                 UNDERSTANDING
                       │
                       ▼
                    PLANNING
                       │
             ┌─────────┴─────────┐
             │                   │
       NEED PERMISSION       READY
             │                   │
             ▼                   ▼
     WAITING_FOR_PERMISSION   EXECUTING
             │                   │
             └───────┬───────────┘
                     │
                     ▼
              WAITING_FOR_USER
                     │
                     ▼
                  EXECUTING
                     │
                     ▼
                 VERIFYING
                /         \
          SUCCESS          FAILURE
             │                │
             ▼                ▼
         COMPLETED          RETRY
                              │
                              ▼
                          EXECUTING
```

---

# 17. Task Cancellation

Cancellation must be independent of the LLM.

The daemon owns cancellation.

Example:

```text
User:
"Stop."

Voice layer
 ↓
CancelTask(task_id)
 ↓
Daemon
 ↓
CancellationToken
 ↓
All active tools
```

Every long-running tool must support cancellation.

---

# 18. Supervisor

The Supervisor is responsible for deciding:

> What type of task is this?

It classifies requests into categories.

```text
SYSTEM_COMMAND
APP_CONTROL
FILE_OPERATION
BROWSER
RESEARCH
CODING
COMMUNICATION
PRODUCTIVITY
MEDIA
DEVICE_CONTROL
AUTONOMOUS_WORKFLOW
CONVERSATION
```

---

# 19. Fast Path

The Supervisor should detect deterministic commands.

Example:

> "Open Chrome."

Instead of:

```text
LLM
 ↓
Plan
 ↓
Agent
```

use:

```text
Intent Router
 ↓
launch_application("Chrome")
```

Target latency should be extremely low.

---

# 20. Agent Path

Complex tasks use the agent.

Example:

> "Find me remote React jobs paying at least ₹10 lakh and shortlist the best ones."

Architecture:

```text
Supervisor
 ↓
Agent Runtime
 ↓
Planner
 ↓
Research tools
 ↓
Browser
 ↓
Ranking
 ↓
Result
```

---

# 21. Planner

The Planner converts a goal into steps.

Input:

```text
Find suitable SDE jobs.
```

Output:

```text
1. Load candidate profile
2. Search job sources
3. Extract job listings
4. Filter
5. Score
6. Present candidates
7. Start selected applications
```

The planner should not be allowed to invent arbitrary system capabilities.

It can only use registered tools.

---

# 22. Structured Plan

A plan should be represented as structured data.

Example:

```json
{
  "task_id": "task_123",
  "goal": "Find suitable SDE jobs",
  "steps": [
    {
      "id": "step_1",
      "tool": "profile.load",
      "depends_on": []
    },
    {
      "id": "step_2",
      "tool": "jobs.search",
      "depends_on": ["step_1"]
    }
  ]
}
```

---

# 23. Executor

The Executor runs the plan.

Responsibilities:

- resolve dependencies,
- invoke tools,
- capture outputs,
- update task state,
- handle failures,
- request permissions,
- trigger verification.

The Executor should be deterministic.

The LLM proposes.

The Executor executes.

---

# 24. Verifier

Every important action should have verification.

Example:

```text
Tool:
browser.click("Submit")

Verifier:
Check for confirmation message.
```

Possible verifier types:

```text
UI verifier
DOM verifier
Process verifier
Filesystem verifier
API verifier
Vision verifier
LLM verifier
```

---

# 25. Verification Hierarchy

Use:

```text
Deterministic API
    ↓
DOM/UI state
    ↓
Accessibility tree
    ↓
Filesystem/process state
    ↓
Vision
    ↓
LLM interpretation
```

The more deterministic the verification, the better.

---

# 26. Tool Registry

Every capability is registered.

Example:

```text
browser.open
browser.click
browser.type
browser.extract
browser.download

desktop.launch
desktop.focus
desktop.click
desktop.type
desktop.screenshot

filesystem.search
filesystem.read
filesystem.write

system.volume
system.brightness

media.play
media.pause
```

---

# 27. Tool Definition

Every tool should contain:

```text
ToolDefinition
├── name
├── version
├── description
├── input_schema
├── output_schema
├── platforms
├── permissions
├── risk_level
├── timeout
├── supports_cancellation
├── verifier
└── implementation
```

---

# 28. Tool Schema

Example:

```json
{
  "name": "desktop.launch_application",
  "description": "Launch an installed application",
  "input": {
    "application": "string"
  },
  "permissions": [],
  "risk": "LOW"
}
```

The model sees the schema.

It does not see implementation details.

---

# 29. Tool Result

Tools should return structured results.

Bad:

```text
"it worked"
```

Better:

```json
{
  "success": true,
  "application": "Chrome",
  "process_id": 18392,
  "window_id": "abc"
}
```

---

# 30. Tool Errors

Errors should also be structured.

Example:

```json
{
  "success": false,
  "error": {
    "code": "APPLICATION_NOT_FOUND",
    "message": "Chrome was not found",
    "recoverable": false
  }
}
```

This lets the agent reason about failures.

---

# 31. Tool Error Categories

Standardize errors:

```text
INVALID_ARGUMENT
PERMISSION_DENIED
NOT_FOUND
AUTH_REQUIRED
NETWORK_ERROR
TIMEOUT
APPLICATION_ERROR
UI_CHANGED
RESOURCE_BUSY
RATE_LIMITED
SECURITY_BLOCKED
USER_REQUIRED
UNKNOWN
```

---

# 32. Tool Runtime

The Tool Runtime is the security boundary between the agent and the platform.

```text
Agent
 ↓
Tool Request
 ↓
Tool Registry
 ↓
Policy Engine
 ↓
Permission Manager
 ↓
Tool Runtime
 ↓
Platform Adapter
```

---

# 33. Policy Engine

The Policy Engine decides:

> Is this action allowed?

Inputs:

```text
tool
user
task
resource
risk
device
application
context
```

Output:

```text
ALLOW
DENY
REQUIRE_CONFIRMATION
REQUIRE_AUTHENTICATION
```

---

# 34. Example Policy

```text
Tool:
filesystem.delete

Resource:
C:\Users\Ashutosh\Documents\resume.pdf

Risk:
HIGH

Policy:
REQUIRE_CONFIRMATION
```

---

# 35. Permission Manager

Permissions should exist at multiple levels.

```text
Global
Device
Application
Plugin
Tool
Task
Resource
```

Example:

```text
Browser:
ALLOW

LinkedIn:
ALLOW

Submit application:
REQUIRE_CONFIRMATION
```

---

# 36. Policy Precedence

Recommended:

```text
Explicit DENY
      >
Security policy
      >
User policy
      >
Tool policy
      >
Default
```

A plugin must never override a security policy.

---

# 37. Confirmation Requests

The daemon should create a structured confirmation request.

Example:

```text
ConfirmationRequest
├── id
├── task_id
├── action
├── explanation
├── risk
├── affected_resources
├── expires_at
└── required_authentication
```

The UI can display:

> "JARVIS wants to submit this job application."

Buttons:

```text
[Approve]
[Deny]
[Cancel Task]
```

---

# 38. Voice Confirmation

Voice confirmation can be used for lower-risk actions.

For critical actions, use explicit UI or OS authentication.

Example:

> "Shall I submit?"

> "Yes."

For financial operations:

> "Please confirm using the security prompt."

---

# 39. Never Trust the LLM for Authorization

The LLM must never determine:

> "This is safe, so I'll allow myself to do it."

Authorization belongs to deterministic code.

Correct:

```text
LLM:
submit_application()

Policy:
HIGH RISK

Permission:
CONFIRMATION REQUIRED

Result:
blocked until user approves
```

---

# 40. Context Engine

The Context Engine builds the information supplied to the agent.

It combines:

```text
Conversation
+
Task state
+
Current application
+
Screen state
+
Relevant memory
+
User profile
+
Tool results
+
Device state
```

The model should receive only the context required for the current decision.

---

# 41. Context Window Management

Do not continuously send the entire history to the LLM.

Use:

```text
Current turn
+
Task summary
+
Relevant memories
+
Recent actions
+
Current observation
```

Old conversation should be summarized.

---

# 42. Context Object

Example:

```text
AgentContext
├── task
├── user
├── device
├── active_application
├── conversation
├── memory
├── observations
├── available_tools
├── permissions
└── previous_actions
```

---

# 43. Model Gateway

The Model Gateway abstracts AI models.

```text
ModelGateway
├── ChatModel
├── VisionModel
├── EmbeddingModel
├── SpeechModel
├── TTSModel
└── Reranker
```

---

# 44. Chat Model Interface

Conceptually:

```python
response = model.generate(
    messages=messages,
    tools=tools,
    temperature=0.2
)
```

The agent should not know whether this is:

- Ollama,
- llama.cpp,
- gpt-oss,
- Qwen,
- another provider.

---

# 45. Model Capabilities

Every model should advertise capabilities.

```text
ModelCapabilities
├── tool_calling
├── vision
├── reasoning
├── streaming
├── structured_output
├── context_length
├── multilingual
└── embeddings
```

The model router chooses an appropriate model.

---

# 46. Model Router

Example:

```text
Simple command
 ↓
small local model

Complex reasoning
 ↓
main model

Screenshot analysis
 ↓
vision model

Embedding
 ↓
embedding model
```

---

# 47. Model Health

The system should monitor:

```text
loaded
available
VRAM usage
RAM usage
latency
errors
context capacity
```

If the main model crashes:

```text
fallback model
```

---

# 48. Event Bus

The Event Bus connects components asynchronously.

Example:

```text
USER_COMMAND_RECEIVED
       ↓
TASK_CREATED
       ↓
PLAN_CREATED
       ↓
TOOL_STARTED
       ↓
TOOL_COMPLETED
       ↓
VERIFICATION_STARTED
       ↓
TASK_COMPLETED
```

---

# 49. Event Definition

Every event:

```text
Event
├── id
├── type
├── timestamp
├── source
├── task_id
├── device_id
├── payload
└── correlation_id
```

---

# 50. Event Types

Core events:

```text
SYSTEM_STARTED
SYSTEM_STOPPING

USER_SPOKE
USER_TEXT_RECEIVED
WAKE_WORD_DETECTED

TASK_CREATED
TASK_UPDATED
TASK_CANCELLED
TASK_PAUSED
TASK_COMPLETED
TASK_FAILED

PLAN_CREATED
PLAN_UPDATED

TOOL_STARTED
TOOL_COMPLETED
TOOL_FAILED

PERMISSION_REQUIRED
PERMISSION_GRANTED
PERMISSION_DENIED

AUTH_REQUIRED
USER_INPUT_REQUIRED

APP_OPENED
APP_CLOSED
SCREEN_CHANGED

MODEL_LOADED
MODEL_UNLOADED
MODEL_ERROR

DEVICE_CONNECTED
DEVICE_DISCONNECTED
```

---

# 51. Event Persistence

Not every event needs permanent storage.

Categories:

### Ephemeral

```textSCREEN_CHANGED
AUDIO_FRAME
MOUSE_MOVED
```

### Auditable

```textTOOL_STARTED
TOOL_COMPLETED
PERMISSION_GRANTED
TASK_COMPLETED
```

### Persistent state

```textTASK_CREATED
TASK_UPDATED
MEMORY_CREATED
WORKFLOW_CREATED
```

---

# 52. Event Correlation

Every action should carry:

```texttask_id
correlation_id
parent_action_id
```

This allows debugging.

Example:

```text
User command
  ↓
task_123
  ↓
plan_456
  ↓
tool_call_789
```

---

# 53. Workflow Engine

Simple commands do not need workflows.

Long-running tasks do.

Examples:

```text
job application
research
coding task
document generation
scheduled routine
```

Workflow:

```text
WorkflowDefinition
 ↓
WorkflowInstance
 ↓
WorkflowSteps
 ↓
State
 ↓
Checkpoint
```

---

# 54. Durable Workflow

A workflow must checkpoint.

Example:

```text
Step 1 completed
CHECKPOINT
Step 2 completed
CHECKPOINT
Step 3 running
```

If JARVIS crashes:

```text
restart
 ↓
load workflow
 ↓
restore checkpoint
 ↓
continue from Step 3
```

---

# 55. Workflow Idempotency

Repeated execution must not accidentally duplicate actions.

Example:

If JARVIS already created:

```text
invoice_123
```

a retry should not create:

```text
invoice_124
```

without checking.

Every consequential action should support idempotency where possible.

---

# 56. Scheduler

The Scheduler handles:

- reminders,
- recurring workflows,
- monitoring,
- delayed actions,
- future tasks.

Examples:

```text
Every day at 8 AM
```

```text
Every Monday
```

```text
When battery < 10%
```

```text
When a new job matching criteria appears
```

---

# 57. Trigger Model

```text
Trigger
├── time
├── event
├── condition
├── application
├── device
├── network
└── external
```

---

# 58. Automation Safety

Automations should have:

```text
enabled
disabled
paused
requires_confirmation
```

The user must be able to disable all automations immediately.

---

# 59. Plugin Manager

Plugins extend JARVIS.

A plugin may provide:

```text
tools
models
workflows
UI
event handlers
```

Examples:

```text
GitHub
Spotify
Home Assistant
Gmail
LinkedIn
VS Code
```

---

# 60. Plugin Manifest

Example:

```yaml
name: github
version: 1.0.0
description: GitHub integration

permissions:
  - network.github
  - github.repository.read

tools:
  - github.search
  - github.read_file
  - github.create_issue
```

---

# 61. Plugin Isolation

A plugin should not automatically inherit:

```text
filesystem.*
shell.*
credentials.*
```

It receives only declared capabilities.

---

# 62. Plugin Lifecycle

```text
DISCOVERED
   ↓
VALIDATING
   ↓
INSTALLED
   ↓
DISABLED
   ↓
ENABLED
   ↓
RUNNING
```

---

# 63. Plugin Versioning

Plugins should declare:

```text
API version
minimum JARVIS version
maximum compatible version
```

This avoids breaking upgrades.

---

# 64. Device Manager

The Device Manager maintains:

```text
device identity
platform
capabilities
connection
permissions
status
```

Example:

```text
Device:
ASHU-PC

Platform:
Windows

Capabilities:
desktop
browser
filesystem
microphone
speaker
```

---

# 65. Device Capability Discovery

When Android connects:

```text
Android capabilities:
voice
camera
notifications
screen
accessibility
location (optional)
```

The core can then determine which device should perform an action.

---

# 66. Task Routing Across Devices

Example:

> "Take a picture and send it to my PC."

Android:

```text
camera.capture
```

PC:

```text
filesystem.save
```

The workflow crosses devices.

---

# 67. Device Selection

The planner should determine:

```text
Where should this action execute?
```

Example:

> "Open VS Code."

Only a desktop device can execute it.

> "Take a picture."

Android is preferred.

---

# 68. Device Availability

The Device Manager should expose:

```text
ONLINE
OFFLINE
BUSY
LOCKED
SLEEPING
```

---

# 69. Configuration Manager

Configuration must be separate from source code.

Files:

```text
config/
├── system.toml
├── models.toml
├── security.toml
├── devices.toml
├── voice.toml
└── plugins.toml
```

Sensitive secrets should not be stored in configuration files.

---

# 70. Configuration Layers

```text
Default
 ↓
System
 ↓
Device
 ↓
User
 ↓
Task
```

The most specific applicable configuration wins.

---

# 71. Environment Separation

Support:

```text
development
testing
staging
production
```

The development environment should never access production credentials.

---

# 72. Logging

Use structured JSON logging internally.

Example:

```json
{
  "timestamp": "...",
  "level": "INFO",
  "component": "executor",
  "task_id": "task_123",
  "message": "Tool completed"
}
```

---

# 73. Log Levels

```text
TRACE
DEBUG
INFO
WARN
ERROR
FATAL
```

Production should default to:

```text
INFO
```

with configurable debugging.

---

# 74. Sensitive Data Redaction

The logger must automatically redact:

```text
password
token
API key
cookie
authorization header
private key
OTP
credit card number
```

Example:

```text
password=[REDACTED]
```

---

# 75. Audit Log vs Debug Log

These should be separate.

### Debug log

Technical information.

### Audit log

Security-relevant actions.

Audit logs should be more durable and tamper-resistant.

---

# 76. Repository Architecture

Recommended final monorepo:

```text
jarvis/
│
├── apps/
│   ├── desktop/
│   │   ├── control-center/
│   │   ├── windows/
│   │   └── linux/
│   │
│   └── android/
│
├── core/
│   ├── agent/
│   ├── planner/
│   ├── executor/
│   ├── verifier/
│   ├── task/
│   ├── workflow/
│   ├── memory/
│   ├── context/
│   ├── policy/
│   ├── permissions/
│   ├── scheduler/
│   ├── events/
│   ├── plugins/
│   └── devices/
│
├── runtime/
│   ├── daemon/
│   ├── tools/
│   ├── ipc/
│   └── services/
│
├── models/
│   ├── gateway/
│   ├── llm/
│   ├── vision/
│   ├── speech/
│   ├── tts/
│   └── embeddings/
│
├── platform/
│   ├── windows/
│   ├── linux/
│   └── android/
│
├── tools/
│   ├── browser/
│   ├── desktop/
│   ├── filesystem/
│   ├── shell/
│   ├── media/
│   ├── communication/
│   ├── documents/
│   └── system/
│
├── skills/
│   ├── chrome/
│   ├── vscode/
│   ├── github/
│   ├── linkedin/
│   ├── spotify/
│   └── gmail/
│
├── protocols/
│   ├── proto/
│   ├── events/
│   └── schemas/
│
├── database/
│   ├── migrations/
│   └── seeds/
│
├── security/
│   ├── policies/
│   ├── sandbox/
│   └── credentials/
│
├── tests/
│   ├── unit/
│   ├── integration/
│   ├── e2e/
│   ├── security/
│   └── benchmarks/
│
├── scripts/
│
├── docs/
│
├── Cargo.toml
├── pyproject.toml
├── package.json
├── pnpm-workspace.yaml
└── README.md
```

---

# 77. Rust Workspace

The Rust portion should itself be a workspace.

```text
rust/
├── crates/
│   ├── jarvis-daemon
│   ├── jarvis-ipc
│   ├── jarvis-events
│   ├── jarvis-tools
│   ├── jarvis-policy
│   ├── jarvis-platform
│   ├── jarvis-config
│   ├── jarvis-security
│   └── jarvis-types
```

---

# 78. Python Workspace

```text
python/
├── jarvis_agent/
├── jarvis_models/
├── jarvis_memory/
├── jarvis_rag/
├── jarvis_planner/
├── jarvis_evaluation/
└── tests/
```

Python should communicate with the Rust runtime through the defined API.

---

# 79. TypeScript Workspace

```text
frontend/
├── control-center/
├── shared-ui/
├── shared-types/
└── tests/
```

---

# 80. Android Workspace

```text
android/
├── app/
├── accessibility/
├── voice/
├── networking/
├── ui/
└── shared/
```

---

# 81. Shared Protocols

Never duplicate API definitions manually.

Source of truth:

```text
protocols/proto/
```

Generate:

```text
Rust types
Python clients
TypeScript clients
Kotlin clients
```

from the same definitions.

---

# 82. API Versioning

Use:

```text
v1
v2
...
```

Example:

```text
jarvis.task.v1
jarvis.device.v1
jarvis.tool.v1
```

Breaking changes should create a new version.

---

# 83. Internal Type System

Important shared types:

```text
TaskId
DeviceId
UserId
ToolId
WorkflowId
PluginId
EventId
PermissionId
SessionId
```

Avoid passing arbitrary strings everywhere.

---

# 84. Unique IDs

Use UUID/ULID-style identifiers.

Prefer IDs that are:

- globally unique,
- sortable where useful,
- safe to serialize.

---

# 85. Conversation Engine

Conversation should be separate from task execution.

Example:

```text
Conversation
   │
   ├── user message
   ├── assistant response
   └── task reference
```

A conversation may spawn many tasks.

---

# 86. Conversation vs Task

Example:

Conversation:

> "I need a job."

Task:

```text
Search jobs
```

Then:

> "Apply to the third one."

Task:

```text
Application #3
```

Both belong to the same conversation but are separate executable units.

---

# 87. Session

A session represents an interaction environment.

Example:

```text
Session
├── conversation
├── device
├── user
├── active task
├── context
└── permissions
```

---

# 88. Interruptions

If the user says:

> "Actually, stop that and open Spotify."

The new task should supersede the old task.

The daemon should support:

```text
interrupt
pause
cancel
replace
resume
```

---

# 89. Priority

Tasks should have priority.

```text
CRITICAL
HIGH
NORMAL
LOW
BACKGROUND
```

Example:

Battery critical notification:

```text
HIGH
```

Background indexing:

```text
LOW
```

---

# 90. Resource Manager

Local AI can consume substantial resources.

JARVIS should track:

```text
CPU
RAM
VRAM
GPU
battery
temperature
disk
network
```

---

# 91. Resource-Aware Model Selection

If laptop battery is low:

```text
Use smaller model
```

If powerful desktop GPU is available:

```text
Use larger model
```

If phone:

```text
Use mobile/remote-to-PC model
```

---

# 92. Model Hosting Strategy

The PC can host the main model.

Android sends:

```text
voice
task
context
```

to the PC over the secure local network.

PC returns:

```text
response
task state
audio
```

This is likely preferable to running a large model directly on Android.

---

# 93. Local Network API

Potential architecture:

```text
Android
   │
Encrypted connection
   │
   ▼
PC JARVIS daemon
   │
Local LLM
```

The PC becomes the computational hub.

---

# 94. Android Offline Mode

When PC is unavailable:

```text
Android
 ↓
small local model
 ↓
basic capabilities
```

Complex desktop tasks naturally become unavailable.

---

# 95. Capability Negotiation

Each device reports:

```text
capabilities
models
tools
permissions
resources
```

The planner can then decide where to execute.

---

# 96. Plugin Tool Discovery

At startup:

```text
Plugin Manager
 ↓
discover plugins
 ↓
validate manifests
 ↓
load allowed plugins
 ↓
register tools
```

The agent sees only enabled tools.

---

# 97. Tool Namespaces

Use namespaces.

Examples:

```text
desktop.launch
desktop.click

browser.open
browser.click

filesystem.read
filesystem.search

github.search
github.issue.create

media.play
media.pause
```

This avoids collisions.

---

# 98. Tool Versioning

Tools should support:

```text
browser.click@1
browser.click@2
```

where necessary.

---

# 99. Tool Compatibility

Every tool declares:

```text
platforms:
  windows
  linux
  android
```

Example:

```text
desktop.launch:
Windows ✓
Linux ✓
Android ✗
```

---

# 100. Tool Preconditions

Tools can specify:

```text
requires_browser
requires_network
requires_user_login
requires_permission
```

The planner can account for them.

---

# 101. Tool Postconditions

Tools should define expected outcomes.

Example:

```text
desktop.launch("Chrome")

Postcondition:
Chrome process exists
AND
Chrome window exists
```

---

# 102. Agent Memory

The Agent should not directly manipulate the database.

Use:

```text
MemoryService
```

Operations:

```text
memory.search()
memory.store()
memory.update()
memory.delete()
```

---

# 103. Memory Safety

The model cannot arbitrarily write:

```text
"user password is ..."
```

into permanent memory.

Memory classification should exist:

```text
PUBLIC
PERSONAL
SENSITIVE
SECRET
```

Secrets should be rejected from ordinary memory.

---

# 104. Context Retrieval

Before planning:

```text
retrieve relevant memories
```

Before a job application:

```text
retrieve:
resume
skills
education
standard answers
preferences
```

---

# 105. Agent Scratchpad

The agent may need temporary reasoning state.

This should be:

```text
ephemeral
task-scoped
not permanent memory
```

Do not permanently store all model reasoning.

---

# 106. Tool Observation

Every tool can return an observation.

Example:

```json
{
  "type": "browser_state",
  "url": "https://linkedin.com/jobs",
  "title": "Jobs",
  "logged_in": true
}
```

The planner uses observations to decide the next action.

---

# 107. Replanning

The agent must be able to replan.

Example:

Expected:

```text
Apply button
```

Actual:

```text
Login required
```

Then:

```text
old plan invalid
 ↓
replan
 ↓
request user authentication
```

---

# 108. Agent Loop

The fundamental loop:

```text
while task_not_finished:

    observe()

    update_context()

    if deterministic_action_available:
        execute()

    else:
        plan()

    check_policy()

    execute_tool()

    verify()

    update_state()

    if failure:
        recover_or_replan()
```

---

# 109. Maximum Action Limits

To prevent runaway agents:

```text
max_steps
max_runtime
max_tool_calls
max_retries
max_network_requests
```

Example:

```text
Job search:
maximum 200 tool calls
maximum 20 minutes
```

The user can configure limits.

---

# 110. Loop Detection

Detect repeated behavior.

Example:

```text
click Apply
click Apply
click Apply
click Apply
```

JARVIS should detect:

```text
possible automation loop
```

and stop.

---

# 111. Confidence

The system should track confidence.

Example:

```text
Intent confidence: 0.98
Job match confidence: 0.91
UI detection confidence: 0.73
```

Low confidence should trigger human assistance.

---

# 112. Human Assistance Threshold

Example policy:

```text
confidence > 0.90
automatic

0.70–0.90
continue carefully

< 0.70
ask user
```

Exact thresholds should be configurable and task-specific.

---

# 113. Natural User Questions

The assistant should ask concise questions.

Bad:

> "The required expected compensation field was found but there is insufficient semantic information regarding your expected compensation."

Good:

> "What salary should I enter?"

---

# 114. User Input Requests

Create a standard:

```text
UserInputRequest
├── question
├── field
├── expected_type
├── choices
├── required
└── sensitive
```

Example:

```text
Question:
"What is your expected salary?"

Type:
currency

Required:
true
```

---

# 115. Sensitive Input

If the request is sensitive:

```text
UserInputRequest
sensitive = true
```

The UI should use a secure input field.

The value should not be sent to normal logs.

---

# 116. Authentication Request

Separate authentication from ordinary user input.

```text
AuthenticationRequest
├── service
├── reason
├── method
└── interaction_required
```

Example:

> "LinkedIn login is required. Please log in in the browser window."

---

# 117. Don't Give Passwords to the LLM

Preferred:

```text
Browser
 ↓
credential manager
 ↓
website
```

not:

```text
password
 ↓
LLM
 ↓
browser
```

---

# 118. Core Database

Start with SQLite.

Recommended database:

```text
data/jarvis.db
```

But the database layer must be abstracted.

Later it can move to PostgreSQL if needed.

---

# 119. Core Tables

Initial tables:

```text
users
devices
sessions
conversations
messages
tasks
task_steps
tool_calls
permissions
workflows
workflow_runs
memories
documents
plugins
plugin_permissions
audit_events
settings
```

---

# 120. Task Table

Conceptually:

```text
tasks
-----
id
parent_id
session_id
device_id
goal
status
priority
created_at
updated_at
started_at
completed_at
result
error
```

---

# 121. Task Steps

```text
task_steps
----------
id
task_id
sequence
tool
status
input
output
error
started_at
completed_at
retry_count
```

---

# 122. Tool Calls

Separate tool calls from steps.

A step may produce multiple calls.

```text
task step
   │
   ├── tool call 1
   ├── tool call 2
   └── tool call 3
```

---

# 123. Audit Events

```text
audit_events
------------
id
timestamp
actor
task_id
action
resource
result
risk
approval_id
```

---

# 124. Database Migrations

Use versioned migrations.

```text
001_initial.sql
002_permissions.sql
003_plugins.sql
004_memory.sql
```

Never manually alter production databases.

---

# 125. Configuration Database

Persistent user preferences can be stored in SQLite.

Large configuration files should remain in files.

---

# 126. Secrets

Secrets should never live in:

```text
SQLite
config.toml
.env
logs
```

except where an encrypted secret store is explicitly used.

---

# 127. Development Environment

Recommended:

```text
Windows
WSL2 / Ubuntu
Git
Rust
Python
Node.js
pnpm
Android Studio
JDK
Docker
```

The project should provide a setup script.

---

# 128. Developer Command Interface

Eventually:

```text
jarvis dev
jarvis test
jarvis build
jarvis run
jarvis doctor
jarvis logs
jarvis models
jarvis plugins
```

---

# 129. `jarvis doctor`

A diagnostic command should inspect:

```text
OS
CPU
RAM
GPU
VRAM
microphone
speaker
Python
Rust
Node
Android SDK
models
database
permissions
network
browser
```

Output:

```text
✓ Rust
✓ Python
✓ Chrome
✓ Microphone
✓ GPU
⚠ Vision model missing
⚠ Android device not paired
```

---

# 130. Build Profiles

Support:

```text
minimal
standard
full
developer
```

### Minimal

Core + voice + basic commands.

### Standard

Core + browser + desktop tools.

### Full

All supported capabilities.

### Developer

Debugging and development tools.

---

# 131. Feature Flags

Use feature flags for experimental capabilities.

```text
vision_agent
autonomous_browser
cross_device
home_automation
proactive_mode
```

---

# 132. Versioning

Use semantic versioning:

```text
MAJOR.MINOR.PATCH
```

Example:

```text
0.1.0
```

During early development:

```text
0.x
```

Once stable:

```text
1.0.0
```

---

# 133. Crash Recovery

If daemon crashes:

1. restart daemon,
2. reload database,
3. identify running tasks,
4. mark interrupted state,
5. determine recoverability,
6. resume safe tasks,
7. ask user about unsafe tasks.

Example:

> "Your job application workflow was interrupted while waiting for submission approval. Nothing was submitted."

---

# 134. Safe Resume

The workflow engine should classify steps:

```text
READ_ONLY
IDEMPOTENT
REVERSIBLE
IRREVERSIBLE
```

After a crash:

- read-only → resume automatically,
- idempotent → retry,
- reversible → inspect state,
- irreversible → require user confirmation.

---

# 135. Shutdown Procedure

When the OS is shutting down:

```text
STOP accepting new tasks
 ↓
checkpoint workflows
 ↓
cancel safe processes
 ↓
persist state
 ↓
shutdown models
 ↓
exit
```

---

# 136. Startup Procedure

```text
OS startup
 ↓
JARVIS daemon
 ↓
load configuration
 ↓
initialize database
 ↓
load permissions
 ↓
discover devices
 ↓
discover plugins
 ↓
initialize wake word
 ↓
start control center
 ↓
READY
```

The main LLM does not necessarily need to load at this point.

---

# 137. Startup Greeting

Optional:

> "Good morning. I'm ready."

But the user should be able to disable startup speech.

---

# 138. Health State

The daemon should expose:

```text
STARTING
READY
DEGRADED
BUSY
ERROR
STOPPING
```

---

# 139. Health Endpoint

Example:

```text
GET /health
```

or gRPC health service.

It should report:

```text
daemon
database
model
voice
tools
platform
```

---

# 140. Graceful Degradation

If vision is unavailable:

```text
Use DOM/UI tree
```

If main LLM unavailable:

```text
Use smaller model
```

If Internet unavailable:

```text
Local capabilities remain available
```

If TTS fails:

```text
UI displays response
```

---

# 141. No Single Point of Failure

The system should not collapse because:

```text
TTS failed
```

or:

```text
vision model unavailable
```

Core functionality should continue where possible.

---

# 142. Testing Architecture

Every layer should be independently testable.

```text
Unit
 ↓
Component
 ↓
Integration
 ↓
Platform
 ↓
End-to-End
```

---

# 143. Fake Tool Runtime

During agent testing, tools should be mockable.

Example:

```text
fake_browser
fake_filesystem
fake_desktop
fake_email
```

This allows testing:

> "What does the planner do when LinkedIn requires login?"

without opening a real browser.

---

# 144. Deterministic Agent Tests

Given:

```text
goal
context
tools
model response
```

the Executor should produce the same behavior.

The LLM itself can be nondeterministic, but the execution layer should not be.

---

# 145. Replay System

Every task should ideally be replayable from:

```text
task input
+
observations
+
tool results
```

This is essential for debugging.

---

# 146. Simulation Mode

Add:

```text
JARVIS_SIMULATE=true
```

In simulation mode:

```text
click
```

becomes:

```text
would click
```

No real action occurs.

This is critical for testing agent plans safely.

---

# 147. Dry Run

The user should eventually be able to say:

> "Show me what you would do, but don't execute it."

JARVIS:

```text
1. Open Chrome
2. Navigate to LinkedIn
3. Search SDE
4. ...
```

Nothing executes.

---

# 148. Explain Mode

Optional:

> "Explain what you're doing."

JARVIS provides concise progress information without exposing internal chain-of-thought.

Example:

> "I'm checking whether you're logged into LinkedIn."

---

# 149. Debug Mode

Developer mode can show:

```text
Task
Plan
Tool
Input
Output
Policy
Verification
Latency
```

This is invaluable during development.

---

# 150. Core Security Boundary

The most important boundary is:

```text
             UNTRUSTED
────────────────────────────────
Web content
Documents
LLM generated text
Plugin data
External APIs
User-provided files

             TRUST BOUNDARY

────────────────────────────────
Policy Engine
Permission Manager
Tool Validator

             TRUSTED
────────────────────────────────
Platform adapters
Credential broker
OS integration
```

---

# 151. Prompt Injection Boundary

External text should always be labeled:

```text
UNTRUSTED_CONTENT
```

For example:

```text
<external_webpage>
...
</external_webpage>
```

The agent must understand that the content is data.

---

# 152. Tool Argument Validation

Even if the LLM generates:

```json
{
  "path": "/etc/shadow"
}
```

the filesystem tool should independently reject it if policy does not permit it.

The tool must never assume:

> "The model requested it, so it must be allowed."

---

# 153. Path Security

Filesystem tools must prevent:

```text
../
```

path traversal.

Use canonicalized paths and explicit allowed roots.

Example:

```text
Allowed:
C:\Users\Ashutosh\Documents

Denied:
C:\Windows
```

unless explicitly authorized.

---

# 154. Shell Security

The shell tool should have multiple modes.

### Restricted

Only approved commands.

### User

Commands run as the user.

### Sandbox

Isolated environment.

### Administrator/root

Never automatic.

---

# 155. Admin Operations

Commands requiring elevated privileges should trigger:

```text
AUTHENTICATION_REQUIRED
```

The LLM should not receive administrator credentials.

---

# 156. Network Policy

Tools should declare:

```text
network_required
allowed_domains
```

For example:

```text
github tool:
github.com

linkedin tool:
linkedin.com
```

This reduces unnecessary network access.

---

# 157. Browser Network Policy

Automation profiles can use domain allowlists.

For example, a job application workflow can restrict navigation to:

```text
linkedin.com
company-careers-domain
approved-cdn-domains
```

This reduces malicious redirection risk.

---

# 158. Secret Leak Prevention

Before sending data to an external website or model:

```text
Data classification
 ↓
Secret scanner
 ↓
Policy
 ↓
Allow / Block
```

---

# 159. Data Classification

Every piece of information should potentially be classified:

```text
PUBLIC
PRIVATE
PERSONAL
SENSITIVE
SECRET
```

Examples:

```text
Weather:
PUBLIC

Resume:
PERSONAL

Phone number:
SENSITIVE

Password:
SECRET
```

---

# 160. Memory Policy

The memory engine should automatically reject:

```text
passwords
API keys
tokens
private keys
OTP codes
credit card numbers
```

unless explicitly stored in a secure credential system.

---

# 161. Core API Dependency Graph

```text
Control Center
      │
      ▼
API Gateway
      │
      ▼
Task Manager
      │
      ▼
Agent Runtime
      │
      ├── Context
      ├── Memory
      ├── Model Gateway
      └── Planner
              │
              ▼
           Executor
              │
              ▼
          Tool Runtime
              │
        ┌─────┴─────┐
        ▼           ▼
     Policy      Platform
        │           │
        └─────┬─────┘
              ▼
          OS / Apps
```

---

# 162. Dependency Rules

The architecture must enforce:

```text
UI
 ↓
Core API
```

not:

```text
UI
 ↓
Windows API
```

Similarly:

```text
Agent
 ↓
Tool API
```

not:

```text
Agent
 ↓
subprocess("rm -rf ...")
```

---

# 163. Platform Adapter Interface

The core should define abstract capabilities.

Example:

```text
PlatformAdapter

launch_application()
list_applications()
get_active_window()
get_windows()
focus_window()
click()
type()
press_key()
move_mouse()
scroll()
capture_screen()
read_clipboard()
write_clipboard()
get_system_state()
```

Windows/Linux implementations provide the actual behavior.

---

# 164. Browser Adapter Interface

```text
BrowserAdapter

launch()
open()
close()
new_tab()
get_tabs()
navigate()
click()
type()
select()
scroll()
screenshot()
extract_dom()
get_accessibility_tree()
download()
upload()
```

Playwright becomes one implementation.

---

# 165. Filesystem Adapter

```text
FilesystemAdapter

list()
search()
read()
write()
copy()
move()
rename()
delete()
stat()
watch()
```

Platform-specific filesystem details remain hidden.

---

# 166. Audio Adapter

```text
AudioAdapter

get_devices()
set_volume()
mute()
unmute()
play()
stop()
```

---

# 167. Notification Adapter

```text
NotificationAdapter

notify()
list()
dismiss()
```

---

# 168. Application Adapter

```text
ApplicationAdapter

discover()
launch()
close()
focus()
is_running()
get_windows()
```

---

# 169. Platform Capability Matrix

The daemon should query:

```text
CapabilityMatrix
```

Example:

| Capability | Windows | Linux | Android |
|---|---:|---:|---:|
| Filesystem | Yes | Yes | Restricted |
| Mouse | Yes | Yes | Gesture |
| Keyboard | Yes | Yes | Restricted |
| Browser | Yes | Yes | Yes |
| Shell | Yes | Yes | Restricted |
| Notifications | Yes | Yes | Yes |
| Camera | Optional | Optional | Yes |
| Accessibility | Yes | Yes | Yes |
| Full desktop control | Yes | Yes | No |

---

# 170. Why This Abstraction Matters

Suppose the planner says:

```text
desktop.click(button)
```

The same workflow can run on:

```text
Windows
Linux
```

without changing the planner.

The platform adapter handles the implementation.

---

# 171. Android Special Case

Android does not expose a full desktop.

Therefore Android may implement:

```text
app.open()
app.close()
screen.read()
screen.gesture()
text.type()
```

but not:

```text
desktop.move_mouse()
```

The capability model handles this cleanly.

---

# 172. Core Interface Philosophy

Interfaces should describe **intent**, not implementation.

Bad:

```text
WindowsUIAutomationInvokePattern()
```

Good:

```text
activate_control()
```

---

# 173. Future Platform Support

This architecture should allow adding:

```text
macOS
iOS
```

without modifying the core agent.

Only new platform adapters are required.

---

# 174. Core Runtime Technology

Recommended implementation:

### Rust

For:

- daemon,
- IPC,
- tool runtime,
- policy,
- platform abstraction,
- process lifecycle,
- secure storage interface.

### Python

For:

- agent,
- planner,
- model gateway,
- memory intelligence,
- RAG.

---

# 175. Python ↔ Rust Boundary

Recommended:

```text
Rust Daemon
     │
     │ gRPC
     ▼
Python Agent Service
```

This keeps responsibilities clear.

Alternative future optimization:

```text
embedded Python
```

can be considered later, but should not be the initial architecture.

---

# 176. Why Separate Python?

AI libraries change quickly.

The system should be able to upgrade:

```text
Python AI stack
```

without replacing:

```text
Rust system layer
```

---

# 177. Agent Service

Python process:

```text
jarvis-agent
```

Responsibilities:

- intent understanding,
- planning,
- model invocation,
- context construction,
- agent reasoning,
- structured tool calls.

It should not have direct unrestricted filesystem/system access.

---

# 178. Rust Daemon

Rust process:

```text
jarvisd
```

Responsibilities:

- permissions,
- tool execution,
- platform control,
- filesystem,
- shell,
- process management,
- IPC,
- database access where appropriate,
- event bus.

---

# 179. Model Server

Possible separate process:

```text
ollama
```

or:

```text
llama-server
```

The architecture should treat it as an external model runtime.

---

# 180. Complete Runtime

```text
                 ┌─────────────┐
                 │ Control UI  │
                 └──────┬──────┘
                        │
                     gRPC
                        │
                 ┌──────▼──────┐
                 │  jarvisd    │
                 │    Rust     │
                 └──────┬──────┘
                        │
               ┌────────┼────────┐
               │        │        │
             Tools    Policy   Events
               │        │
               └────────┼────────┘
                        │
                     gRPC
                        │
                 ┌──────▼──────┐
                 │jarvis-agent │
                 │   Python    │
                 └──────┬──────┘
                        │
                   Model Gateway
                        │
             ┌──────────┼──────────┐
             │          │          │
          LLM        Vision       RAG
```

---

# 181. Database Ownership

Prefer:

```text
Rust daemon
```

as the authoritative database owner.

Python accesses data through service APIs.

This prevents multiple processes from arbitrarily modifying the database.

---

# 182. Memory Service

Python may perform:

- embeddings,
- retrieval,
- semantic ranking.

Rust/core remains responsible for:

- authorization,
- storage boundaries,
- access policy.

---

# 183. API Contract Testing

Every API must have contract tests.

Example:

```text
Python client
 ↔
Rust server
```

Test:

```text
request schema
response schema
error schema
version compatibility
```

---

# 184. Build Pipeline

Recommended CI:

```text
git push
 ↓
format
 ↓
lint
 ↓
unit tests
 ↓
integration tests
 ↓
security checks
 ↓
build
 ↓
package
```

---

# 185. Static Analysis

Rust:

```text
cargo fmt
cargo clippy
cargo test
```

Python:

```text
ruff
mypy
pytest
```

TypeScript:

```text
eslint
tsc
vitest
```

Android:

```text
ktlint
detekt
Gradle tests
```

---

# 186. Dependency Management

Pin important versions.

Use lockfiles:

```text
Cargo.lock
uv.lock / equivalent
pnpm-lock.yaml
Gradle lock/configuration
```

---

# 187. Supply Chain Security

Plugins and dependencies must be treated as potentially dangerous.

Use:

- signed releases,
- checksums,
- dependency scanning,
- vulnerability scanning,
- plugin permissions.

---

# 188. Plugin Signing

Future plugin distribution should support:

```text
plugin package
 ↓
signature verification
 ↓
manifest verification
 ↓
permission review
 ↓
installation
```

---

# 189. Update System

JARVIS should support independent updates:

```text
core
models
plugins
skills
UI
platform adapters
```

A browser skill should not require reinstalling the entire application.

---

# 190. Model Updates

Models can be large.

Model management should support:

```text
download
pause
resume
verify
install
remove
quantization
activate
deactivate
```

---

# 191. Model Registry

Example:

```text
models/
├── chat/
├── vision/
├── speech/
├── embedding/
└── tts/
```

Metadata:

```text
name
version
format
size
VRAM
capabilities
checksum
```

---

# 192. Hardware Detection

On startup:

```text
CPU
RAM
GPU
VRAM
OS
architecture
```

The model manager recommends:

```text
model size
quantization
runtime
```

---

# 193. Example Hardware Profiles

### Low-end laptop

```text
small quantized model
CPU inference
small STT
```

### Gaming laptop

```text
7B–14B class model
GPU acceleration
vision model
```

### High-end workstation

```text
large local model
vision
multiple specialist models
```

### Android

```text
small local model
or
PC-hosted model
```

---

# 194. Core Event Flow Example

User says:

> "Jarvis, open Chrome."

```text
Microphone
 ↓
Wake Word
 ↓
STT
 ↓
USER_SPOKE
 ↓
Intent Router
 ↓
launch_application
 ↓
Policy
 ↓
Tool Runtime
 ↓
Windows Adapter
 ↓
Chrome
 ↓
Verification
 ↓
TASK_COMPLETED
 ↓
TTS
```

---

# 195. Complex Event Flow

User:

> "Find five SDE jobs and apply."

```text
USER_SPOKE
 ↓
TASK_CREATED
 ↓
UNDERSTANDING
 ↓
PLANNING
 ↓
PROFILE_RETRIEVED
 ↓
BROWSER_STARTED
 ↓
LOGIN_CHECK
 ↓
JOB_SEARCH
 ↓
JOB_ANALYSIS
 ↓
APPLICATION_START
 ↓
FORM_FILL
 ↓
USER_INPUT_REQUIRED
 ↓
FORM_VALIDATION
 ↓
PERMISSION_REQUIRED
 ↓
USER_APPROVED
 ↓
SUBMISSION
 ↓
VERIFICATION
 ↓
APPLICATION_RECORDED
 ↓
TASK_COMPLETED
```

---

# 196. User Experience Principle

The architecture should hide complexity from the user.

The user should not need to know:

- gRPC,
- protobuf,
- agents,
- vector stores,
- tools,
- policies.

They should experience:

> "Jarvis, do this."

and:

> "I need this information."

---

# 197. Developer Experience Principle

Developers should experience:

```text
Create a tool
 ↓
Define schema
 ↓
Declare permission
 ↓
Implement adapter
 ↓
Add verifier
 ↓
Register tool
```

rather than modifying the entire agent.

---

# 198. Example New Tool

Suppose we add:

```text
spotify.search
```

Implementation:

```text
1. Define tool schema.
2. Declare permissions.
3. Implement Spotify adapter.
4. Add verifier.
5. Register tool.
6. Add tests.
```

The main planner should automatically discover it.

---

# 199. Example New Platform

Suppose we add macOS.

Implement:

```text
MacOSPlatformAdapter
```

supporting:

```text
launch
click
type
screen
filesystem
applications
notifications
```

The core remains unchanged.

---

# 200. Example New Model

Suppose a new local LLM becomes available.

Implement:

```text
NewModelProvider
```

and register:

```text
ModelCapabilities
```

No planner rewrite should be necessary.

---

# 201. Example New Application Skill

Suppose we add Notion.

Create:

```text
skills/notion/
```

with:

```text
manifest
tools
workflows
selectors
verifiers
```

The core automatically discovers it.

---

# 202. The Golden Rule of JARVIS Architecture

Whenever a new capability is requested, ask:

> Can this be implemented as a tool or plugin without changing the core?

If yes:

**Do that.**

If no:

Determine whether the core interface is incorrectly designed.

---

# 203. What the Core Must NOT Contain

The core should not contain:

```text
LinkedIn-specific selectors
Windows-specific coordinates
Spotify-specific UI logic
Android-specific gestures
Chrome-specific DOM selectors
```

Those belong in:

```text
skills/
platform/
tools/
```

---

# 204. Separation of Concerns

```text
CORE
What can JARVIS do?

AGENT
What should JARVIS do?

PLANNER
What steps are required?

TOOL
How is an operation requested?

POLICY
Is it allowed?

PLATFORM
How is it actually performed?

VERIFIER
Did it work?

MEMORY
What should JARVIS remember?

UI
How should the user see/control it?
```

---

# 205. Final Core Architecture

```text
                    ┌────────────────────┐
                    │       USER         │
                    └─────────┬──────────┘
                              │
                    Voice / Text / UI
                              │
                    ┌─────────▼──────────┐
                    │   SESSION LAYER    │
                    └─────────┬──────────┘
                              │
                    ┌─────────▼──────────┐
                    │   TASK MANAGER     │
                    └─────────┬──────────┘
                              │
                    ┌─────────▼──────────┐
                    │     SUPERVISOR     │
                    └─────────┬──────────┘
                              │
             ┌────────────────┼────────────────┐
             │                │                │
             ▼                ▼                ▼
          Fast Path        Planner         Conversation
             │                │
             │          ┌─────▼─────┐
             │          │  Context  │
             │          └─────┬─────┘
             │                │
             │          ┌─────▼─────┐
             │          │   Model   │
             │          └─────┬─────┘
             │                │
             └────────┬───────┘
                      │
               ┌──────▼───────┐
               │   EXECUTOR   │
               └──────┬───────┘
                      │
               ┌──────▼───────┐
               │ TOOL REGISTRY│
               └──────┬───────┘
                      │
               ┌──────▼───────┐
               │ POLICY ENGINE│
               └──────┬───────┘
                      │
               ┌──────▼───────┐
               │  PERMISSIONS │
               └──────┬───────┘
                      │
               ┌──────▼───────┐
               │ TOOL RUNTIME │
               └──────┬───────┘
                      │
             ┌────────┼─────────┐
             │        │         │
             ▼        ▼         ▼
          Windows    Linux    Android
```

---

# 206. Initial Core Milestone

Before implementing LinkedIn, Spotify, Gmail, Android accessibility, or complex autonomous behavior, the following must work:

```text
[ ] jarvisd starts
[ ] Control Center connects
[ ] User can send text command
[ ] Task is created
[ ] Supervisor classifies command
[ ] Planner can create plan
[ ] Tool registry discovers tools
[ ] Policy engine evaluates tool
[ ] Executor invokes tool
[ ] Tool returns structured result
[ ] Verifier validates result
[ ] Task becomes completed
[ ] Event is emitted
[ ] UI receives task updates
[ ] Audit log records action
[ ] Task can be cancelled
[ ] Task can be resumed
[ ] Daemon can restart safely
```

Only after these foundations work should advanced autonomous workflows be built.

---

# 207. First Proof-of-Concept

The first real JARVIS prototype should be deliberately small.

Supported commands:

```text
"Jarvis, open Chrome."

"Jarvis, close Chrome."

"Jarvis, what applications are open?"

"Jarvis, take a screenshot."

"Jarvis, type Hello World."

"Jarvis, open VS Code."

"Jarvis, what is on my screen?"
```

Architecture:

```text
Voice
 ↓
STT
 ↓
Agent
 ↓
Tool
 ↓
Platform
 ↓
Verification
 ↓
TTS
```

This validates the entire architecture without requiring hundreds of features.

---

# 208. Second Proof-of-Concept

Add browser automation:

```text
"Search Google for React jobs."

"Open the third result."

"Read the page."

"Summarize this page."

"Fill this form."
```

This validates:

- browser agent,
- DOM,
- vision fallback,
- workflow state,
- verification.

---

# 209. Third Proof-of-Concept

Add multi-step autonomy:

```text
"Find five React jobs matching my profile."
```

This validates:

- planning,
- research,
- memory,
- ranking,
- browser automation,
- long-running workflows.

---

# 210. Fourth Proof-of-Concept

Add human-in-the-loop:

```text
"Apply to this job."

JARVIS:
"The application is ready. Shall I submit it?"

User:
"Yes."

JARVIS:
"Submitted."
```

This validates the security architecture.

---

# 211. Fifth Proof-of-Concept

Add cross-device:

```text
Android:
"Jarvis, open VS Code on my PC."

PC:
opens VS Code

Android:
"Done."
```

This validates:

- device pairing,
- network protocol,
- shared task state,
- authentication.

---

# 212. Sixth Proof-of-Concept

Add proactive behavior:

```text
"Every morning, summarize my calendar and important emails."
```

This validates:

- scheduler,
- background daemon,
- memory,
- communication integrations.

---

# 213. Final Architecture Objective

The system should eventually be capable of:

```text
                  JARVIS
                    │
      ┌─────────────┼─────────────┐
      │             │             │
    THINK          SEE           HEAR
      │             │             │
      └─────────────┼─────────────┘
                    │
                  ACT
                    │
       ┌────────────┼────────────┐
       │            │            │
    COMPUTER      BROWSER       PHONE
       │            │            │
       └────────────┼────────────┘
                    │
                 REMEMBER
                    │
                 LEARN*
                    │
                 ASSIST
```

`*` Learning must be controlled, auditable, and reversible rather than allowing unrestricted autonomous modification of its own behavior.

---

# 214. Definition of Done for Core

The core architecture is complete when:

### Runtime

- daemon works
- services start/stop cleanly
- crash recovery works.

### Agent

- intent classification works
- planning works
- tool selection works
- replanning works.

### Tools

- registry works
- schemas work
- errors are standardized
- cancellation works.

### Security

- policy engine works
- permissions work
- confirmations work
- sensitive information is protected.

### Workflow

- state machine works
- checkpoints work
- recovery works
- scheduling works.

### Memory

- storage works
- retrieval works
- classification works
- deletion works.

### API

- gRPC works
- versioning works
- clients can connect.

### Extensibility

- plugins work
- skills work
- new platforms can be added.

### Observability

- logs work
- audit works
- task tracing works.

---

# 215. Architectural Contract for Future Documents

All subsequent documents must follow these rules.

### Windows document

Must implement:

```text
PlatformAdapter
DesktopAdapter
FilesystemAdapter
ApplicationAdapter
NotificationAdapter
```

### Linux document

Must implement the same interfaces for:

```text
Wayland
X11
GNOME
KDE where practical
```

### Android document

Must implement Android-specific equivalents without pretending Android has unrestricted desktop control.

### Browser document

Must implement:

```text
BrowserAdapter
BrowserSkill
BrowserVerifier
```

### AI document

Must implement:

```text
ModelGateway
STT
TTS
Vision
Embeddings
```

### Security document

Must implement:

```text
PolicyEngine
PermissionManager
CredentialBroker
Sandbox
Audit
```

No later subsystem should bypass these boundaries.

---

# 216. Recommended Immediate Repository Creation

The first repository should be initialized with:

```text
jarvis/
├── apps/
├── core/
├── runtime/
├── models/
├── platform/
├── tools/
├── skills/
├── protocols/
├── database/
├── security/
├── tests/
├── scripts/
└── docs/
```

Then establish:

```text
Rust workspace
Python workspace
TypeScript workspace
Android project
Protocol definitions
CI
Development environment
```

before implementing advanced AI.

---

# 217. Final Core Principle

The most important architectural decision in the entire JARVIS project is this:

```text
                 LLM
                  │
             "I want to..."
                  │
                  ▼
               AGENT
                  │
             "I should..."
                  │
                  ▼
               TOOL
                  │
             "I request..."
                  │
                  ▼
              POLICY
                  │
             "You may..."
                  │
                  ▼
             PLATFORM
                  │
             "I execute..."
                  │
                  ▼
                OS
                  │
             "It happened."
                  │
                  ▼
              VERIFY
                  │
             "Confirmed."
                  │
                  ▼
               USER
```

This separation is the foundation that allows JARVIS to become extremely capable **without making the LLM itself the security boundary**.

---

# 218. Next Document

The next document should be:

## **Document 2 — Local AI Engine: LLM + Vision + Speech + Wake Word + TTS + Model Management**

That document should go substantially deeper into:

- selecting the actual local models,
- gpt-oss/Qwen/Gemma comparison,
- model sizes and quantization,
- CPU/GPU/VRAM requirements,
- Ollama vs llama.cpp,
- model routing,
- tool-calling models,
- vision models,
- screenshot understanding,
- Whisper/whisper.cpp,
- wake-word detection,
- VAD,
- noise suppression,
- Piper/TTS,
- streaming voice,
- interruption,
- voice activity,
- local inference APIs,
- model downloading,
- model caching,
- hardware detection,
- Android inference,
- PC-hosted inference,
- fallback models,
- performance benchmarks,
- and the exact AI runtime architecture for JARVIS.