# JARVIS — Document 22
# Complete API, IPC, Event Bus & Service Interface Specification

**Purpose:** Define the implementation contracts connecting every JARVIS subsystem.

**Scope:** Core runtime, AI services, voice, vision, tools, browser, platform adapters, memory, security, device mesh, Android, UI, and external skill services.

---

# 1. Why This Document Exists

JARVIS is a distributed local application rather than a single executable.

Even when installed on one computer, it should be treated as multiple logical services:

```text
JARVIS Supervisor
       │
       ├── Core Orchestrator
       ├── AI Runtime
       ├── Speech
       ├── Vision
       ├── Tool Runtime
       ├── Browser Runtime
       ├── Memory
       ├── Security
       ├── UI
       └── Device Mesh
```

If these components communicate through arbitrary ad-hoc calls, the system will become difficult to maintain.

Therefore every subsystem needs explicit contracts.

The central rule is:

> Components communicate through stable interfaces, not through knowledge of each other's implementation.

---

# 2. Communication Layers

JARVIS should use different communication mechanisms for different purposes.

```text
Same process
    ↓
direct function / trait interface

Same machine, separate process
    ↓
Unix domain socket / named pipe
or
local gRPC

Streaming
    ↓
WebSocket / streaming gRPC

PC ↔ Android
    ↓
authenticated encrypted network protocol

External integrations
    ↓
HTTPS / OAuth / provider APIs
```

Do not force one protocol everywhere.

---

# 3. Recommended Protocol Stack

## Core serialization

Use:

```text
Protocol Buffers
```

for strongly typed service contracts.

## RPC

Use:

```text
gRPC
```

for request/response and streaming service APIs.

## Event streaming

Use:

```text
gRPC streams
```

or:

```text
WebSocket
```

when browser/UI compatibility makes WebSocket preferable.

## Local IPC

Windows:

```text
Named Pipes
```

Linux:

```text
Unix Domain Sockets
```

The transport should be hidden behind a common IPC interface.

---

# 4. Protocol Design Principles

Every request should have:

```text
request_id
timestamp
source
destination
protocol_version
deadline
authorization context
```

Every response should contain:

```text
request_id
status
result or error
duration
```

---

# 5. Request ID

Every operation receives a globally unique identifier.

Example:

```text
req_01JARVIS8K...
```

The request ID is used for:

```text
logging
tracing
cancellation
debugging
audit
correlation
```

---

# 6. Task ID

A request and a task are not necessarily the same thing.

Example:

```text
request_id:
    voice request

task_id:
    multi-step LinkedIn application
```

One task can generate hundreds of requests.

---

# 7. Trace ID

A trace ID groups all operations belonging to a single user interaction.

```text
trace_id
    ├── STT
    ├── LLM
    ├── planner
    ├── browser
    ├── vision
    └── TTS
```

This makes end-to-end debugging possible.

---

# 8. Common Envelope

Conceptually:

```json
{
  "protocol_version": 1,
  "request_id": "req_123",
  "trace_id": "trace_456",
  "task_id": "task_789",
  "timestamp": "2026-08-17T12:00:00Z",
  "source": "voice",
  "destination": "core",
  "type": "command",
  "payload": {}
}
```

---

# 9. Message Types

At minimum:

```text
COMMAND
RESPONSE
EVENT
STREAM_START
STREAM_DATA
STREAM_END
ERROR
CANCEL
HEARTBEAT
HEALTH
APPROVAL_REQUEST
APPROVAL_RESPONSE
```

---

# 10. Command

Commands request an action.

Example:

```text
open_application
browser.navigate
memory.search
tts.speak
```

---

# 11. Response

Responses should be deterministic.

Example:

```json
{
  "request_id": "req_123",
  "status": "SUCCESS",
  "result": {
    "application": "chrome",
    "pid": 9320
  }
}
```

---

# 12. Error Contract

Every service must use structured errors.

Example:

```json
{
  "code": "APP_NOT_FOUND",
  "message": "Chrome was not found.",
  "retryable": false,
  "details": {}
}
```

Do not rely on arbitrary string exceptions as the API contract.

---

# 13. Error Categories

Recommended categories:

```text
INVALID_ARGUMENT
UNAUTHORIZED
FORBIDDEN
NOT_FOUND
TIMEOUT
CANCELLED
UNAVAILABLE
RESOURCE_EXHAUSTED
CONFLICT
DEPENDENCY_FAILURE
MODEL_FAILURE
PLATFORM_FAILURE
SECURITY_BLOCKED
USER_REQUIRED
UNKNOWN
```

---

# 14. Retry Policy

Each error must identify whether retrying is appropriate.

Examples:

```text
TIMEOUT
    retryable

INVALID_ARGUMENT
    not retryable

SECURITY_BLOCKED
    not automatically retryable

SERVICE_UNAVAILABLE
    retryable
```

The planner must not blindly retry.

---

# 15. Deadline

Every request should have a deadline.

Examples:

```text
wake-word:
    very short

LLM:
    configurable

browser navigation:
    medium

long-running workflow:
    task-level deadline
```

---

# 16. Cancellation

Every long-running operation must support cancellation.

Example:

```text
User:
    "JARVIS, search jobs."

User:
    "Stop."

Core
 ↓
cancel task
 ↓
browser stops
 ↓
planner stops
 ↓
TTS stops
```

---

# 17. Service Registration

Every service registers with the supervisor.

Example:

```json
{
  "service": "ai-runtime",
  "version": "1.2.0",
  "capabilities": [
    "chat",
    "tool_calling",
    "vision"
  ]
}
```

---

# 18. Health API

Every service exposes:

```text
health()
```

Response:

```json
{
  "status": "READY",
  "version": "1.2.0",
  "uptime_ms": 932822,
  "dependencies": {}
}
```

---

# 19. Readiness vs Liveness

Liveness:

```text
process is alive
```

Readiness:

```text
process is capable of serving requests
```

Example:

```text
AI process alive
but model loading

→ LIVENESS = true
→ READY = false
```

---

# 20. Core Service API

The Core should expose:

```text
SubmitCommand
CancelTask
GetTask
ListTasks
ApproveTask
RejectTask
GetSystemState
GetCapabilities
```

---

# 21. SubmitCommand

Conceptually:

```text
SubmitCommand(
    text,
    source,
    context
)
```

Example:

```json
{
  "text": "open chrome",
  "source": "voice",
  "context": {
    "device": "desktop"
  }
}
```

---

# 22. Core Response

The core should initially return:

```text
accepted
task_id
```

because the task may be asynchronous.

Example:

```json
{
  "accepted": true,
  "task_id": "task_123"
}
```

---

# 23. Task API

Support:

```text
GetTask(task_id)
```

Example:

```json
{
  "task_id": "task_123",
  "state": "EXECUTING",
  "progress": 0.45
}
```

---

# 24. Task States

Use:

```text
CREATED
QUEUED
PLANNING
EXECUTING
WAITING_FOR_USER
VERIFYING
COMPLETED
FAILED
CANCELLED
EXPIRED
```

---

# 25. Event Bus

The event bus is one of the most important JARVIS components.

It connects:

```text
voice
core
AI
tools
UI
memory
security
device mesh
```

without hard-coupling them.

---

# 26. Event Types

Examples:

```text
WakeWordDetected
SpeechStarted
SpeechEnded
TranscriptPartial
TranscriptFinal
CommandReceived
TaskCreated
TaskUpdated
ToolStarted
ToolCompleted
ToolFailed
ApprovalRequested
ApprovalGranted
ApprovalDenied
ScreenChanged
DeviceConnected
DeviceDisconnected
ModelLoaded
ModelUnloaded
```

---

# 27. Event Properties

Every event contains:

```text
event_id
event_type
timestamp
source
trace_id
task_id
payload
```

---

# 28. Event Delivery

Events should support:

```text
publish
subscribe
unsubscribe
filter
replay where appropriate
```

---

# 29. Event Reliability

Not all events need durable delivery.

Classify events:

```text
EPHEMERAL
IMPORTANT
DURABLE
```

Example:

```text
cursor moved
    EPHEMERAL

screen changed
    EPHEMERAL

approval granted
    DURABLE

task completed
    IMPORTANT
```

---

# 30. AI Runtime API

The AI service should expose:

```text
ListModels
LoadModel
UnloadModel
Generate
GenerateStructured
GenerateWithTools
Embed
CancelGeneration
GetModelStatus
```

---

# 31. Generate

Input:

```text
model
messages
generation parameters
context
```

Output:

```text
streamed tokens
final response
usage
```

---

# 32. Structured Generation

Critical outputs should use schemas.

Example:

```json
{
  "intent": "open_application",
  "application": "chrome"
}
```

Never parse arbitrary prose when a structured representation is possible.

---

# 33. Tool Calling

The LLM may propose:

```text
tool_name
arguments
```

But the AI service does NOT execute tools.

Correct:

```text
LLM
 ↓
tool proposal
 ↓
Core
 ↓
policy
 ↓
tool runtime
```

Incorrect:

```text
LLM
 ↓
direct shell execution
```

---

# 34. AI Service Boundary

The model is untrusted.

Therefore:

```text
model output
    ↓
schema validation
    ↓
policy
    ↓
execution
```

---

# 35. Model Manager API

Support:

```text
SearchModels
DownloadModel
VerifyModel
InstallModel
RemoveModel
ListInstalledModels
GetModelMetadata
SetDefaultModel
```

---

# 36. Model Metadata

Store:

```text
model ID
version
format
quantization
size
RAM requirement
VRAM requirement
context length
capabilities
checksum
license
source
```

---

# 37. Speech Service API

Expose:

```text
StartListening
StopListening
Transcribe
StreamTranscription
GetAudioDevices
SetAudioDevice
```

---

# 38. Voice Stream

The preferred flow:

```text
audio chunks
 ↓
VAD
 ↓
STT
 ↓
partial transcript
 ↓
final transcript
```

Partial transcripts should be streamed.

---

# 39. TTS API

Expose:

```text
Speak
StreamSpeech
StopSpeech
ListVoices
SetVoice
```

---

# 40. TTS Interruption

The UI/core must be able to issue:

```text
StopSpeech(task_id)
```

immediately.

Do not wait for the current sentence to finish.

---

# 41. Vision API

Expose:

```text
AnalyzeImage
AnalyzeScreenshot
FindElement
ReadScreen
OCR
DescribeRegion
```

---

# 42. Vision Request

Include:

```text
image
task
region
expected output
```

Example:

```text
Find:
    "Apply" button
```

rather than:

```text
"Describe this screenshot."
```

Task-specific vision is more efficient.

---

# 43. Browser Service API

Expose:

```text
CreateSession
Navigate
Back
Forward
Reload
Click
Type
Select
Scroll
ReadPage
GetDOM
Screenshot
Upload
Download
CloseSession
```

---

# 44. Browser Session

Every automation session should have:

```text
session_id
profile
browser
tabs
permissions
created_at
last_activity
```

---

# 45. Browser Context

Do not expose unlimited browser state to the LLM.

Provide:

```text
relevant DOM
visible text
selected elements
current URL
page title
```

Only include screenshots when needed.

---

# 46. Tool Service API

Expose:

```text
ListTools
GetTool
ExecuteTool
CancelTool
GetToolSchema
```

---

# 47. Tool Execution Contract

Input:

```text
tool ID
arguments
authorization context
task ID
deadline
```

Output:

```text
status
result
verification hints
```

---

# 48. Tool Manifest

Every tool should define:

```yaml
name:
description:
version:
platforms:
permissions:
risk_level:
input_schema:
output_schema:
timeout:
supports_cancel:
```

---

# 49. Risk Levels

Use:

```text
R0 — informational
R1 — reversible local action
R2 — external side effect
R3 — sensitive/high-impact
R4 — prohibited without explicit policy
```

Examples:

```text
get time:
    R0

open app:
    R1

send email:
    R2

submit job application:
    R2/R3 depending on context

transfer money:
    R3/R4
```

---

# 50. Policy API

Expose:

```text
EvaluateToolCall
RequestApproval
GetPermissions
GrantPermission
RevokePermission
```

---

# 51. Policy Decision

Possible outcomes:

```text
ALLOW
DENY
ASK_USER
ALLOW_WITH_CONSTRAINTS
```

---

# 52. Policy Example

```text
Tool:
    filesystem.delete

Input:
    /home/user/project/temp.txt

Policy:
    ASK_USER
```

---

# 53. Security Context

Every tool request should carry:

```text
user
device
application
task
permissions
risk level
authentication state
```

---

# 54. Credential API

Credentials should be referenced indirectly.

Example:

```text
GetCredentialHandle("linkedin")
```

The returned object should be an opaque handle.

The LLM never receives:

```text
username
password
token
cookie
```

unless explicitly permitted by a tightly controlled design, which should generally be avoided.

---

# 55. Memory API

Expose:

```text
StoreMemory
SearchMemory
GetMemory
UpdateMemory
DeleteMemory
ForgetUserData
```

---

# 56. Memory Search

Input:

```text
query
scope
limit
privacy constraints
```

Output:

```text
memory records
relevance scores
source metadata
```

---

# 57. Memory Permissions

Not every service can access all memory.

Example:

```text
browser skill:
    task-specific memory

core:
    broader context

voice:
    minimal context
```

---

# 58. UI API

The desktop UI should subscribe to:

```text
task events
approval requests
notifications
model state
device state
```

The UI should not directly control low-level tools.

---

# 59. UI Commands

UI can request:

```text
approve
deny
cancel
retry
pause
resume
```

through Core.

---

# 60. Platform API

Define platform-neutral operations:

```text
OpenApplication
CloseApplication
FocusWindow
ListWindows
KeyboardType
KeyboardPress
MouseMove
MouseClick
Screenshot
ClipboardRead
ClipboardWrite
Notify
GetSystemInfo
```

---

# 61. Windows Adapter

Maps:

```text
OpenApplication
```

to Windows mechanisms.

The core does not know the Windows implementation.

---

# 62. Linux Adapter

Maps the same interface to Linux desktop mechanisms.

Support:

```text
Wayland
X11
desktop portals
```

where available.

---

# 63. Android API

Android should expose only capabilities appropriate to Android.

Examples:

```text
Speak
Listen
Notify
Camera
Screen capture
Launch app
Get device status
Approve task
```

---

# 64. Device Mesh API

Expose:

```text
PairDevice
UnpairDevice
ListDevices
GetDeviceStatus
SendCommand
SendEvent
TransferTask
```

---

# 65. Device Identity

Each device receives:

```text
device_id
public key
display name
platform
capabilities
```

---

# 66. Pairing

Recommended:

```text
PC displays pairing code/QR
 ↓
Android scans
 ↓
keys exchanged
 ↓
user confirms
 ↓
device trusted
```

---

# 67. Cross-Device Task Transfer

A task checkpoint should contain:

```text
task ID
goal
current state
completed steps
pending steps
required permissions
context references
```

Do not transfer secrets unnecessarily.

---

# 68. Configuration API

Central configuration should support:

```text
GetConfig
SetConfig
ReloadConfig
ValidateConfig
```

Configuration categories:

```text
AI
voice
models
security
devices
UI
browser
memory
logging
```

---

# 69. Configuration Precedence

Recommended:

```text
compiled defaults
 ↓
system config
 ↓
user config
 ↓
environment
 ↓
session overrides
```

Secrets should not be stored in ordinary configuration files.

---

# 70. Logging API

Use structured logs.

Example:

```json
{
  "timestamp": "...",
  "level": "INFO",
  "service": "core",
  "event": "tool_started",
  "task_id": "...",
  "tool": "browser.navigate"
}
```

---

# 71. Log Levels

Use:

```text
TRACE
DEBUG
INFO
WARN
ERROR
FATAL
```

---

# 72. Sensitive Data Redaction

The logger must automatically redact:

```text
password
access_token
refresh_token
cookie
authorization
API key
private key
```

---

# 73. Metrics API

Track:

```text
request count
error count
latency
queue depth
model inference speed
STT latency
TTS latency
tool success rate
task success rate
```

---

# 74. Health Aggregation

Supervisor should aggregate:

```text
Core
AI
Voice
Vision
Browser
Memory
Security
Mesh
```

into:

```text
JARVIS overall health
```

---

# 75. Startup Sequence

Recommended:

```text
Supervisor
 ↓
Configuration
 ↓
Security
 ↓
IPC
 ↓
Core
 ↓
Memory
 ↓
AI
 ↓
Voice
 ↓
Vision
 ↓
Tools
 ↓
Browser
 ↓
UI
 ↓
Mesh
```

Some components can initialize concurrently after dependencies are ready.

---

# 76. Shutdown Sequence

Reverse dependency order:

```text
UI
 ↓
Mesh
 ↓
Browser
 ↓
Tools
 ↓
Vision
 ↓
Voice
 ↓
AI
 ↓
Memory
 ↓
Core
 ↓
IPC
 ↓
Security
 ↓
Supervisor
```

Long-running tasks should receive cancellation first.

---

# 77. Startup Recovery

If JARVIS crashes:

```text
restart
 ↓
load persisted task checkpoints
 ↓
inspect state
 ↓
mark interrupted actions
 ↓
ask whether unsafe tasks should resume
```

Do not blindly resume high-risk operations.

---

# 78. API Compatibility

Every public service API must have:

```text
version
compatibility policy
deprecation process
migration notes
```

---

# 79. Versioning

Use semantic versioning for services:

```text
MAJOR.MINOR.PATCH
```

Protocol changes should be backward compatible whenever possible.

---

# 80. Backward Compatibility

A newer core should ideally communicate with an older compatible service.

If incompatible:

```text
service rejected
 ↓
clear diagnostic
```

not mysterious failure.

---

# 81. API Authentication

Local IPC still requires authentication/authorization.

Do not assume:

```text
localhost = trusted
```

Use:

```text
process identity
IPC permissions
session credentials
capability checks
```

---

# 82. Android API Authentication

Every Android request must authenticate the device.

Recommended:

```text
device certificate/key
+
session nonce
+
message authentication
```

---

# 83. Replay Protection

Commands should contain:

```text
nonce
timestamp
request ID
```

Reject stale/replayed messages.

---

# 84. Network Loss

Device mesh should handle:

```text
disconnect
reconnect
queue
expiry
duplicate prevention
```

---

# 85. Streaming Architecture

For voice:

```text
Mic
 ↓
Audio stream
 ↓
VAD
 ↓
STT stream
 ↓
Core
 ↓
LLM token stream
 ↓
TTS sentence stream
 ↓
Speaker
```

The entire pipeline should support cancellation.

---

# 86. Streaming Backpressure

If TTS cannot consume generated text fast enough:

```text
buffer
or
pause producer
```

Do not allow unbounded queues.

---

# 87. Resource Manager

JARVIS needs a resource manager controlling:

```text
GPU
CPU
RAM
VRAM
model instances
camera
microphone
browser sessions
```

---

# 88. Model Scheduling

Example:

```text
LLM running
 ↓
vision requested
 ↓
resource manager
 ↓
check VRAM
 ↓
load vision model
```

If insufficient resources:

```text
unload idle model
or
use fallback
```

---

# 89. AI Request Priority

Recommended:

```text
P0 emergency / interruption
P1 active user interaction
P2 active task
P3 background task
P4 indexing / maintenance
```

---

# 90. Queue Architecture

Use separate queues:

```text
interactive
task
background
maintenance
```

Interactive voice commands must not wait behind indexing.

---

# 91. Tool Concurrency

Tools should declare:

```text
exclusive
parallel-safe
resource-bound
```

Example:

```text
camera:
    exclusive

filesystem read:
    parallel-safe

browser session:
    session-bound
```

---

# 92. Task Locking

Prevent conflicting tasks.

Example:

```text
Task A:
    controls Chrome

Task B:
    controls same Chrome session
```

The scheduler should detect conflict.

---

# 93. Browser Lock

Browser sessions should be addressable:

```text
browser_session_id
```

rather than relying on:

```text
"whatever Chrome is currently open"
```

---

# 94. Filesystem Safety

Filesystem tools should normalize paths and enforce:

```text
allowed roots
denylist
symlink handling
permission checks
```

---

# 95. Shell Execution

Shell execution should be a special high-risk capability.

Never allow:

```text
LLM → arbitrary shell
```

without policy enforcement.

Prefer structured tools:

```text
install_package
list_files
git_status
run_test
```

over arbitrary shell strings.

---

# 96. Browser Safety

Browser tools should distinguish:

```text
READ
NAVIGATE
WRITE
SUBMIT
DOWNLOAD
UPLOAD
```

because their risk levels differ.

---

# 97. External Side Effects

Any action affecting an external party should be observable.

Examples:

```text
send email
send message
submit form
publish post
purchase
delete cloud data
```

The tool result should clearly state what happened.

---

# 98. Verification Contract

Every important tool should define:

```text
verification method
```

Example:

```text
browser.click
```

verification:

```text
expected DOM state
```

---

# 99. Verification Result

Example:

```json
{
  "verified": true,
  "method": "dom_state",
  "evidence": "button changed to Applied"
}
```

---

# 100. Evidence

JARVIS should retain minimal task evidence:

```text
what action occurred
when
what result was observed
```

Do not retain unnecessary screenshots or sensitive content.

---

# 101. Audit API

Expose:

```text
GetAuditEvents
SearchAudit
ExportAudit
```

Audit events should be append-only from the application's perspective.

---

# 102. Privacy API

Expose:

```text
DeleteConversation
DeleteMemory
DeleteTaskHistory
DeleteScreenshots
ForgetApplicationData
ExportUserData
```

---

# 103. Data Retention

Every persistent data class should have a retention policy.

Examples:

```text
temporary audio:
    short-lived

screenshots:
    minimal retention

task logs:
    configurable

memories:
    user-controlled
```

---

# 104. API Testing

For every API:

```text
valid request
invalid request
missing fields
unauthorized request
timeout
cancellation
duplicate request
dependency failure
```

must be tested.

---

# 105. Contract Tests

Each service must pass contract tests against the protocol definition.

This prevents:

```text
Core expects A
Service sends B
```

---

# 106. Fuzz Testing

Fuzz:

```text
protobuf inputs
JSON inputs
tool arguments
browser content
filesystem paths
model structured outputs
```

especially security-sensitive interfaces.

---

# 107. Schema Validation

Use strict schemas.

Reject:

```text
unknown dangerous fields
incorrect types
invalid enum values
oversized payloads
```

---

# 108. Payload Limits

Every service should define limits for:

```text
message size
image size
audio duration
tool arguments
memory query size
browser content
```

This prevents resource exhaustion.

---

# 109. Large Content Handling

Do not send huge webpages directly into the LLM.

Use:

```text
extract
summarize
chunk
retrieve
```

---

# 110. Document Pipeline

For PDFs/documents:

```text
file
 ↓
parser
 ↓
text
 ↓
structure
 ↓
chunk
 ↓
retrieval
 ↓
LLM
```

Vision is used for pages that require visual interpretation.

---

# 111. Browser Content Pipeline

```text
URL
 ↓
DOM
 ↓
readability extraction
 ↓
relevant content
 ↓
LLM
```

Screenshot only when needed.

---

# 112. Context API

The Core should expose a context service:

```text
GetTaskContext
AddObservation
RemoveObservation
GetRelevantContext
```

---

# 113. Context Types

Separate:

```text
conversation context
task context
environment context
memory context
security context
```

Do not mix them into one uncontrolled prompt.

---

# 114. Prompt Construction

The LLM should receive:

```text
system policy
+
task
+
relevant context
+
tool schemas
+
observations
```

not raw internal databases.

---

# 115. Prompt Injection Boundary

External content must be labeled as:

```text
UNTRUSTED_CONTENT
```

The model must understand that:

```text
webpage instructions
email instructions
PDF instructions
```

are data, not JARVIS policy.

---

# 116. Tool Output Sanitization

Tool results should be bounded.

For example:

```text
browser.read
```

should not automatically dump:

```text
entire page
```

into the model.

---

# 117. Human-in-the-Loop API

Approval request:

```json
{
  "approval_id": "...",
  "task_id": "...",
  "action": "submit_application",
  "risk": "R3",
  "summary": "Submit LinkedIn application",
  "expires_at": "..."
}
```

---

# 118. Approval Expiration

Approvals should expire.

Example:

```text
approval valid for 60 seconds
```

or an appropriate task-specific duration.

Do not allow an old approval to authorize a modified action.

---

# 119. Approval Binding

Approval should bind to:

```text
exact tool
exact arguments
task
user
device
```

If the action changes materially:

```text
request approval again
```

---

# 120. Plugin API

Plugins should expose:

```text
manifest
capabilities
tools
permissions
version
health
```

---

# 121. Plugin Isolation

A plugin must not automatically gain:

```text
filesystem
shell
network
credential
camera
microphone
```

access.

Permissions are explicitly granted.

---

# 122. Plugin Lifecycle

```text
DISCOVERED
 ↓
VALIDATED
 ↓
INSTALLED
 ↓
ENABLED
 ↓
RUNNING
 ↓
DISABLED
 ↓
REMOVED
```

---

# 123. Plugin Update

Updates should verify:

```text
signature
checksum
compatibility
permissions changes
```

If permissions increase:

```text
ask user
```

---

# 124. Android Capability Negotiation

PC should know:

```text
Android supports:
    microphone
    camera
    TTS
    notifications
```

and not request unsupported operations.

---

# 125. Capability Discovery

Every device exposes:

```text
GetCapabilities()
```

Example:

```json
{
  "microphone": true,
  "camera": true,
  "screen_capture": true,
  "local_llm": false,
  "tts": true
}
```

---

# 126. Cross-Device Routing

A task can ask:

```text
best device for capability X
```

Example:

```text
camera capture
→ phone

large LLM
→ desktop GPU

notification
→ phone + desktop
```

---

# 127. Shared Event Model

PC and Android should use compatible event schemas.

Example:

```text
TaskApprovalRequested
```

can be delivered to:

```text
desktop
Android
```

---

# 128. Offline Device Mode

If Android cannot reach the PC:

```text
local basic assistant
```

should still support selected capabilities.

Example:

```text
local timer
local alarm
local notes
local TTS
basic commands
```

---

# 129. Graceful Degradation

If the large model is unavailable:

```text
fallback model
```

If vision is unavailable:

```text
UI tree / OCR
```

If network is unavailable:

```text
local capabilities
```

If Android is offline:

```text
PC continues
```

---

# 130. API Documentation Generation

The protocol definitions should be the source of truth.

Generate:

```text
Rust types
Python types
TypeScript types
Kotlin types
documentation
```

where practical.

Do not manually maintain duplicate schemas.

---

# 131. Suggested Repository Layout for Interfaces

```text
crates/
└── protocol/
    ├── proto/
    │   ├── core.proto
    │   ├── task.proto
    │   ├── tool.proto
    │   ├── ai.proto
    │   ├── speech.proto
    │   ├── vision.proto
    │   ├── browser.proto
    │   ├── memory.proto
    │   ├── security.proto
    │   └── device.proto
    │
    ├── generated/
    └── build.rs
```

---

# 132. Example Service Boundaries

```text
core
  owns task state

ai
  owns inference

speech
  owns audio processing

vision
  owns visual inference

browser
  owns browser sessions

tools
  owns tool execution

memory
  owns persistent memory

security
  owns policy and authorization

mesh
  owns device communication
```

---

# 133. Ownership Rule

A component owns the state that belongs to it.

Examples:

```text
Browser service:
    browser sessions

AI service:
    loaded model state

Core:
    task state

Memory:
    memory records
```

Do not duplicate mutable state across services.

---

# 134. State Synchronization

When another service needs state:

```text
request it
```

or subscribe to:

```text
state-change event
```

Do not maintain hidden shadow copies.

---

# 135. Idempotency

Operations should declare whether they are idempotent.

Examples:

```text
read file:
    yes

navigate:
    usually yes

send email:
    no

submit application:
    no
```

Non-idempotent operations need stronger retry controls.

---

# 136. Exactly-Once vs At-Least-Once

Do not assume distributed communication is exactly once.

Use:

```text
idempotency keys
```

for operations where duplication is dangerous.

---

# 137. Example

For:

```text
submit_application
```

send:

```text
idempotency_key = task_id + action_hash
```

The system can detect duplicate submission attempts.

---

# 138. Queue Persistence

Long-running tasks may require durable queues.

At minimum persist:

```text
task ID
state
checkpoint
created time
deadline
permissions
```

---

# 139. Queue Recovery

After restart:

```text
load tasks
 ↓
classify interrupted tasks
 ↓
resume safe tasks
 ↓
ask user for unsafe tasks
```

---

# 140. Transaction Boundaries

Do not make an entire multi-hour workflow one transaction.

Use checkpoints:

```text
step 1 complete
step 2 complete
step 3 pending
```

---

# 141. Event Ordering

Events for the same task should preserve logical ordering.

Use:

```text
sequence_number
```

when required.

Example:

```text
ToolStarted seq=10
ToolCompleted seq=11
```

---

# 142. Clock Handling

Use:

```text
UTC timestamps
```

internally.

Convert to local time only for presentation.

---

# 143. Timeouts

Timeouts must exist at multiple layers:

```text
network timeout
RPC timeout
tool timeout
planner timeout
task deadline
```

---

# 144. Circuit Breakers

For unstable dependencies:

```text
browser
AI runtime
network integration
```

use circuit-breaker behavior.

If repeated failures occur:

```text
temporarily stop calls
 ↓
recover
```

---

# 145. Rate Limits

Protect:

```text
LLM
browser
network
external APIs
notifications
```

from runaway loops.

---

# 146. Agent Budget

Every autonomous task should have:

```text
max steps
max tokens
max model calls
max browser actions
max runtime
max retries
```

---

# 147. Planner ↔ Core Contract

Planner proposes:

```text
next action
```

Core decides:

```text
whether it is allowed
```

This separation is mandatory.

---

# 148. Planner Output

Example:

```json
{
  "action": "browser.click",
  "arguments": {
    "element_id": "apply_button"
  },
  "reason": "Open application form",
  "expected_result": "Application form visible"
}
```

---

# 149. Core Decision

Core performs:

```text
schema validation
 ↓
permission evaluation
 ↓
resource check
 ↓
tool execution
```

---

# 150. Observation

After execution:

```json
{
  "success": true,
  "observation": {
    "page_state": "application_form"
  }
}
```

This goes back to the planner.

---

# 151. Agent Loop Contract

```text
GOAL
 ↓
PLAN
 ↓
VALIDATE
 ↓
AUTHORIZE
 ↓
EXECUTE
 ↓
OBSERVE
 ↓
VERIFY
 ↓
UPDATE STATE
 ↓
PLAN NEXT
```

---

# 152. Agent Termination

The planner must be able to return:

```text
SUCCESS
NEEDS_USER
BLOCKED
FAILED
TIMEOUT
CANCELLED
```

It must not plan indefinitely.

---

# 153. "Needs User" Is a First-Class State

Examples:

```text
password required
CAPTCHA detected
ambiguous question
sensitive form field
approval required
device unavailable
```

JARVIS should ask rather than guess.

---

# 154. Voice Interaction Contract

JARVIS should support:

```text
ACKNOWLEDGE
ASK
REPORT
INTERRUPT
CANCEL
```

Example:

```text
"One moment."

"I need your work authorization answer."

"I found three matching jobs."

"Stopping now."
```

---

# 155. Voice Priority

Speech output should be interruptible by:

```text
user speech
emergency command
system shutdown
high-priority notification
```

---

# 156. Final API Principle

The most important architectural rule is:

```text
LLM proposes.
Core decides.
Policy authorizes.
Tool executes.
Environment changes.
Verifier observes.
Memory records.
TTS communicates.
```

This separation prevents the AI model from becoming the operating system itself.

---

# 157. Implementation Completion Criteria

This document is implemented when:

```text
[ ] All core protobuf contracts exist
[ ] Service registration works
[ ] Health checks work
[ ] Request IDs work
[ ] Trace IDs work
[ ] Task IDs work
[ ] Cancellation works
[ ] Structured errors work
[ ] Event bus works
[ ] AI API exists
[ ] Speech API exists
[ ] TTS API exists
[ ] Vision API exists
[ ] Browser API exists
[ ] Tool API exists
[ ] Policy API exists
[ ] Memory API exists
[ ] Device API exists
[ ] Capability negotiation works
[ ] API versioning exists
[ ] Contract tests exist
[ ] Logging and metrics exist
```

---

# 158. Final Architecture

The complete communication topology becomes:

```text
                         ┌──────────────┐
                         │   Android    │
                         └──────┬───────┘
                                │
                         Secure Device Mesh
                                │
┌────────────────────────────────────────────────────┐
│                    JARVIS PC                       │
│                                                    │
│  ┌──────────────┐                                  │
│  │  Supervisor  │                                  │
│  └──────┬───────┘                                  │
│         │                                          │
│  ┌──────▼───────┐                                  │
│  │     Core     │                                  │
│  │ Orchestrator │                                  │
│  └──────┬───────┘                                  │
│         │                                          │
│    ┌────┼────────────┬───────────┐                 │
│    │    │            │           │                 │
│ ┌──▼─┐ ┌▼───┐    ┌──▼──┐    ┌───▼──┐              │
│ │ AI │ │Voice│    │Vision│    │Tools │              │
│ └────┘ └────┘    └─────┘    └───┬──┘              │
│                                  │                 │
│                             ┌────▼────┐            │
│                             │ Browser │            │
│                             └─────────┘            │
│                                                    │
│  ┌──────────┐  ┌──────────┐  ┌────────────┐       │
│  │ Security │  │  Memory  │  │ Device Mesh│       │
│  └──────────┘  └──────────┘  └────────────┘       │
│                                                    │
│              ┌──────────────┐                      │
│              │ Desktop UI   │                      │
│              └──────────────┘                      │
└────────────────────────────────────────────────────┘
```

The result is a modular local AI operating layer in which every major subsystem can be replaced, tested, secured, and scaled independently.

---

# 159. Next Implementation Priority

After defining these interfaces, implementation should begin with:

```text
1. protocol definitions
2. generated bindings
3. IPC transport
4. supervisor
5. Core task service
6. event bus
7. tool registry
8. health/metrics
9. AI provider adapter
10. first deterministic vertical slice
```

Only after these contracts are stable should the project aggressively expand into browser automation, autonomous planning, memory, and multi-device execution.

