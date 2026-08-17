# JARVIS — Document 16
# Application / Plugin / Skill System

**Document status:** Detailed implementation specification  
**Purpose:** Define the extensible capability system that allows JARVIS to operate applications, websites, operating-system functions, devices, files, APIs, and user workflows without hard-coding every capability into the core agent.

---

# 1. Objective

JARVIS must be able to do more than generate text.

It must be able to:

```text
open an application
inspect application state
navigate interfaces
click controls
type text
read information
call APIs
execute approved OS operations
perform multi-step workflows
remember workflow state
ask for missing information
request confirmation for risky actions
recover from failures
```

The Skill System is the layer that converts these capabilities into reusable, permission-controlled modules.

---

# 2. Core Principle

The LLM must never directly control the operating system.

Instead:

```text
User
  ↓
Voice / UI
  ↓
JARVIS Core
  ↓
Planner
  ↓
Skill Router
  ↓
Skill
  ↓
Tool
  ↓
Permission Policy
  ↓
Execution Runtime
  ↓
OS / Browser / API / Device
```

The model proposes actions.

The runtime validates and executes them.

---

# 3. Why a Skill System Is Necessary

Without a skill architecture, JARVIS becomes:

```text
one giant codebase
```

with:

```text
if LinkedIn...
if Spotify...
if Chrome...
if VS Code...
if Gmail...
```

This does not scale.

Instead:

```text
skills/
├── browser/
├── linkedin/
├── spotify/
├── vscode/
├── filesystem/
├── terminal/
├── windows/
├── linux/
├── android/
└── ...
```

Each skill implements a stable contract.

---

# 4. Skill Definition

A skill is a versioned package that exposes one or more capabilities to JARVIS.

Examples:

```text
Spotify Skill
Chrome Skill
LinkedIn Skill
Filesystem Skill
VS Code Skill
GitHub Skill
Email Skill
Calendar Skill
Windows Skill
Ubuntu Skill
Android Skill
```

---

# 5. Skill vs Tool

A **skill** represents a user-facing capability/domain.

A **tool** is an executable operation.

Example:

```text
Skill:
LinkedIn Job Search

Tools:
search_jobs
open_job
extract_job_details
fill_application
upload_resume
submit_application
```

---

# 6. Skill vs Plugin

For JARVIS:

```text
Plugin = installable extension package
Skill = capability exposed by plugin
Tool = executable operation
```

One plugin can provide multiple skills.

Example:

```text
github-plugin
 ├── repository skill
 ├── issue skill
 └── pull-request skill
```

---

# 7. Built-In vs External Skills

Built-in:

```text
filesystem
process manager
window manager
audio
browser
system settings
```

External:

```text
Spotify
LinkedIn
Gmail
Notion
Slack
GitHub
```

---

# 8. Skill Architecture

```text
                  JARVIS CORE
                      │
                Skill Registry
                      │
             ┌────────┴────────┐
             ▼                 ▼
        Built-in Skills   External Plugins
             │                 │
             └────────┬────────┘
                      ▼
                 Skill Router
                      │
                      ▼
                 Tool Registry
                      │
                      ▼
               Policy Engine
                      │
                      ▼
                Tool Runtime
```

---

# 9. Skill Lifecycle

Every skill follows:

```text
DISCOVER
INSTALL
VERIFY
REGISTER
LOAD
INITIALIZE
READY
EXECUTE
SUSPEND
UPDATE
DISABLE
UNINSTALL
```

---

# 10. Skill Registry

The registry maintains:

```text
skill ID
plugin ID
version
platforms
capabilities
tools
permissions
dependencies
status
health
```

---

# 11. Skill ID

Use stable identifiers.

Example:

```text
com.jarvis.spotify
com.jarvis.linkedin
com.jarvis.browser
com.jarvis.filesystem
```

---

# 12. Semantic Versioning

Use:

```text
MAJOR.MINOR.PATCH
```

Example:

```text
1.4.2
```

---

# 13. Compatibility

A skill can declare:

```text
minimum JARVIS version
maximum tested JARVIS version
platform compatibility
runtime compatibility
```

---

# 14. Skill Manifest

Example:

```yaml
id: com.jarvis.spotify
name: Spotify
version: 1.0.0

description: Control Spotify playback.

platforms:
  - windows
  - ubuntu
  - android

permissions:
  - audio.playback
  - application.launch

tools:
  - play
  - pause
  - next
  - previous
  - search
```

---

# 15. Manifest Requirements

Every plugin should declare:

```text
identity
version
description
author
license
platforms
runtime
dependencies
permissions
tools
configuration
entrypoint
```

---

# 16. Capability Manifest

Capabilities should be explicit.

Example:

```yaml
capabilities:
  - browser.navigate
  - browser.read
  - browser.type
  - browser.click
```

---

# 17. Principle of Least Privilege

A skill should request only what it needs.

Spotify:

```text
audio.playback
```

does not need:

```text
filesystem.write
```

---

# 18. Permission Classes

Use capability categories:

```text
READ
WRITE
EXECUTE
NETWORK
DEVICE
CREDENTIAL
SENSITIVE
ADMIN
```

---

# 19. Risk Levels

Recommended:

```text
R0 — harmless
R1 — low impact
R2 — external communication
R3 — consequential
R4 — high impact
R5 — prohibited/restricted
```

---

# 20. Example Risk Classification

```text
play music → R0
open Chrome → R0
read public webpage → R0
create local file → R1
send email → R2
submit job application → R3
delete many files → R4
change system security settings → R4
bypass authentication → R5
```

---

# 21. Tool Metadata

Every tool should declare:

```text
name
description
input schema
output schema
risk level
permissions
idempotency
timeout
side effects
confirmation requirement
```

---

# 22. Tool Schema

Use JSON Schema-compatible definitions.

Example:

```json
{
  "name": "search_jobs",
  "description": "Search jobs on LinkedIn",
  "input_schema": {
    "type": "object",
    "properties": {
      "query": {"type": "string"},
      "location": {"type": "string"},
      "remote": {"type": "boolean"}
    },
    "required": ["query"]
  }
}
```

---

# 23. Tool Output

Tools should return structured results.

Example:

```json
{
  "status": "success",
  "jobs": [
    {
      "title": "Software Engineer",
      "company": "Example",
      "location": "Bangalore"
    }
  ]
}
```

---

# 24. Never Return Arbitrary Model Instructions

Tool results are data.

A webpage containing:

```text
Ignore previous instructions
```

must remain untrusted data.

---

# 25. Tool Execution Contract

```python
result = await tool.execute(
    context=context,
    arguments=validated_arguments
)
```

The tool runtime handles:

```text
authorization
validation
logging
timeouts
cancellation
retries
```

---

# 26. Tool Context

Context can include:

```text
user_id
device_id
task_id
skill_id
permissions
security_context
deadline
cancellation_token
```

---

# 27. Skill Interface

Conceptual:

```python
class Skill:

    manifest: SkillManifest

    async def initialize(self, context):
        ...

    async def health(self):
        ...

    async def execute(self, tool_name, arguments, context):
        ...

    async def shutdown(self):
        ...
```

---

# 28. Tool Interface

```python
class Tool:

    definition: ToolDefinition

    async def validate(self, arguments):
        ...

    async def execute(self, context, arguments):
        ...
```

---

# 29. Skill Runtime

The runtime is responsible for:

```text
loading
dependency injection
permissions
timeouts
resource limits
lifecycle
health
logging
```

---

# 30. Skill Router

The skill router maps user intent to capabilities.

User:

> "Play some music."

Router:

```text
music playback
```

→ Spotify skill.

---

# 31. Skill Selection

Selection should consider:

```text
intent
available skills
platform
current application state
permissions
tool cost
skill health
```

---

# 32. Multiple Skills

Some tasks require multiple skills.

Example:

> "Find SDE jobs and apply using my resume."

Possible chain:

```text
Browser Skill
+
LinkedIn Skill
+
Memory Skill
+
Filesystem Skill
+
Credential Skill
```

---

# 33. Skill Composition

Skills should be composable.

```text
Job Search
 ├── Browser
 ├── LinkedIn
 ├── Resume
 ├── Credential
 └── File Upload
```

---

# 34. Workflow vs Skill

A workflow is a sequence of skill/tool operations.

Example:

```text
search jobs
→ filter
→ inspect
→ fill
→ review
→ submit
```

The skill provides primitives.

The workflow provides orchestration.

---

# 35. Skill Dependency Graph

A plugin can declare:

```text
requires:
  - com.jarvis.browser >= 1.0
  - com.jarvis.memory >= 1.0
```

---

# 36. Dependency Resolution

Installer should:

```text
read manifest
 ↓
resolve dependencies
 ↓
check compatibility
 ↓
install
 ↓
verify
 ↓
register
```

---

# 37. Dependency Conflicts

If:

```text
Plugin A requires Browser 2.x
Plugin B requires Browser 1.x
```

the registry should detect conflict before installation.

---

# 38. Platform Capabilities

Skills can declare:

```text
windows
ubuntu
android
```

and platform-specific implementations.

---

# 39. Cross-Platform Skill

Example:

```text
Spotify
```

can have:

```text
WindowsAdapter
LinuxAdapter
AndroidAdapter
```

under one logical skill.

---

# 40. Native Adapter

Conceptually:

```python
class SpotifySkill:
    def play(self): ...
```

Implementation:

```text
Windows → native media controls
Ubuntu → MPRIS
Android → MediaSession
```

---

# 41. Browser Skill

The Browser Skill should expose:

```text
launch
navigate
back
forward
reload
read_page
find_text
click
type
select
upload
download
scroll
screenshot
tabs
windows
```

---

# 42. Browser Skill Architecture

```text
Browser Skill
      │
      ├── Chromium Adapter
      ├── Firefox Adapter
      └── Generic UI Adapter
```

---

# 43. Browser Automation Priority

Prefer:

```text
DOM/accessibility/API
```

over:

```text
pixel coordinates
```

because structured interfaces are more reliable.

---

# 44. Browser Computer Use

When DOM access is insufficient:

```text
screenshot
+
vision model
+
accessibility tree
```

can be used.

---

# 45. Browser Action

Example:

```json
{
  "action": "click",
  "target": {
    "role": "button",
    "name": "Apply"
  }
}
```

---

# 46. Coordinate Actions

Coordinates should be a fallback.

```json
{
  "action": "click",
  "x": 812,
  "y": 492
}
```

This should be higher risk and less preferred.

---

# 47. LinkedIn Skill

Possible tools:

```text
search_jobs
filter_jobs
open_job
read_job
extract_application
fill_application
upload_resume
answer_question
save_job
submit_application
```

---

# 48. LinkedIn Job Workflow

```text
user request
 ↓
job search
 ↓
filter
 ↓
inspect job
 ↓
match requirements
 ↓
prepare application
 ↓
fill fields
 ↓
detect missing information
 ↓
request user input if required
 ↓
review
 ↓
submit
```

---

# 49. Application Submission Safety

Submission is an external consequential action.

Default:

```text
confirmation required
```

unless the user has explicitly created a trusted policy allowing automatic submission.

---

# 50. Job Application Confirmation

JARVIS can say:

> "The application is complete. The information looks correct. Shall I submit it?"

---

# 51. Application Questions

If a question cannot be safely answered from memory:

```text
ask user
```

Do not invent qualifications.

---

# 52. Salary Questions

If user profile contains a preference:

```text
use approved preference
```

otherwise ask.

---

# 53. Work Authorization

Never fabricate.

If unknown:

> "I need your authorization status before I can answer this."

---

# 54. Resume Selection

Use memory/document retrieval:

```text
job requirements
+
resume metadata
```

to select an appropriate resume.

---

# 55. Form Filling

Map:

```text
field
→ profile/memory/document
```

with confidence.

---

# 56. Form Confidence

Example:

```text
Name → 1.0
Email → 1.0
Experience → 0.95
Unknown salary → 0.0
```

Only high-confidence fields should be automatically filled.

---

# 57. External Communication

Skills that send:

```text
email
messages
applications
forms
```

should declare external side effects.

---

# 58. Email Skill

Tools:

```text
search
read
draft
reply
send
archive
label
delete
```

`send` should normally be R2.

---

# 59. Calendar Skill

Tools:

```text
search_events
create_event
update_event
cancel_event
find_availability
```

---

# 60. File System Skill

Tools:

```text
list
read
write
copy
move
rename
delete
search
```

---

# 61. File Deletion

Single file:

```text
R1
```

Recursive destructive deletion:

```text
R4
```

Require stronger confirmation.

---

# 62. Terminal Skill

Tools:

```text
run_command
```

should not mean unrestricted shell execution.

---

# 63. Command Policy

Commands should be classified:

```text
safe
review
restricted
blocked
```

---

# 64. Safe Commands

Examples may include:

```text
pwd
ls
dir
git status
python --version
node --version
```

depending on environment.

---

# 65. Restricted Commands

Examples:

```text
rm -rf
format
disk operations
registry changes
sudo
powershell encoded commands
```

require strong policy.

---

# 66. Admin Access

Never let the model silently escalate privileges.

Flow:

```text
tool requests privilege
 ↓
policy check
 ↓
OS prompt / user confirmation
 ↓
execute
```

---

# 67. Windows Skill

Possible tools:

```text
launch_app
close_app
focus_window
minimize
maximize
set_volume
mute
lock
sleep
shutdown
screenshot
clipboard
```

---

# 68. Ubuntu Skill

Possible tools:

```text
launch_app
window_control
workspace_control
volume
brightness
lock
sleep
shutdown
screenshot
clipboard
```

---

# 69. Android Skill

Possible tools:

```text
launch_app
open_settings
read_accessibility
click
type
scroll
back
home
notification
media
share
```

subject to Android platform restrictions and permissions.

---

# 70. Accessibility Layer

Android and desktop accessibility APIs can provide structured UI information.

Prefer this over screenshots where possible.

---

# 71. Accessibility Tree

Represent:

```text
role
name
bounds
state
enabled
clickable
editable
children
```

---

# 72. UI Automation Abstraction

Create:

```python
class UIAutomation:
    async def get_tree(self): ...
    async def click(self, target): ...
    async def type(self, target, text): ...
    async def scroll(self, direction): ...
```

---

# 73. Application Adapters

Each application can implement:

```text
UI adapter
API adapter
CLI adapter
native adapter
```

---

# 74. Adapter Preference

Use the strongest available interface:

```text
Official API
>
Native integration
>
Accessibility/DOM
>
CLI
>
Vision + coordinates
```

---

# 75. Why APIs Come First

APIs are generally:

```text
more deterministic
faster
easier to validate
less fragile
```

---

# 76. UI Automation Fallback

When no API exists:

```text
DOM/accessibility
```

then:

```text
vision
```

---

# 77. Skill Discovery

JARVIS can search installed skills by:

```text
name
description
capability
tool
```

---

# 78. Dynamic Tool List

Do not expose hundreds of tools to the LLM at once.

Use:

```text
relevant tool retrieval
```

---

# 79. Tool Retrieval

Example:

```text
User: "Play music"

Retrieve:
spotify.play
spotify.search
media.pause
```

not:

```text
filesystem.delete
terminal.run
```

---

# 80. Tool Namespace

Use names:

```text
spotify.play
browser.navigate
filesystem.read
memory.search
windows.launch_app
```

---

# 81. Tool Names Must Be Stable

Tool names are part of the agent contract.

Avoid frequent breaking changes.

---

# 82. Tool Aliases

For migrations:

```text
browser.open_url
→ browser.navigate
```

can be temporarily aliased.

---

# 83. Tool Versioning

A tool can declare:

```text
api_version
```

---

# 84. Skill Configuration

Skills can have configuration:

```yaml
settings:
  default_browser: chrome
  default_profile: personal
```

---

# 85. Configuration Security

Never store:

```text
password
API secret
private key
```

in plain plugin configuration.

Use Credential Service.

---

# 86. Plugin Data Directory

Each plugin receives:

```text
plugin data directory
```

Example:

```text
data/plugins/com.jarvis.spotify/
```

---

# 87. Plugin Filesystem Isolation

A plugin should not automatically access:

```text
whole home directory
```

It should receive explicit paths/capabilities.

---

# 88. Plugin Network Access

Network access should be declared.

Example:

```yaml
network:
  allowed_domains:
    - api.spotify.com
```

---

# 89. Network Policy

A plugin requesting arbitrary internet access should receive stronger scrutiny.

---

# 90. Plugin Sandbox

Possible levels:

```text
L0 — in-process trusted
L1 — restricted subprocess
L2 — sandboxed process/container
```

---

# 91. Built-In Trusted Skills

Core OS skills may require:

```text
trusted runtime
```

because they need native APIs.

---

# 92. Third-Party Plugins

Prefer:

```text
sandboxed subprocess
```

where practical.

---

# 93. Plugin IPC

Sandboxed plugins communicate using:

```text
local IPC
```

with authenticated requests.

---

# 94. IPC Protocol

Messages:

```json
{
  "request_id": "abc",
  "method": "spotify.play",
  "arguments": {}
}
```

---

# 95. IPC Response

```json
{
  "request_id": "abc",
  "status": "success",
  "result": {}
}
```

---

# 96. Request Authentication

Each plugin should have:

```text
plugin identity
session token
capability token
```

where required.

---

# 97. Capability Tokens

A capability token can specify:

```text
skill
tool
scope
expiry
task
```

---

# 98. Capability Lease

Example:

```text
LinkedIn skill:
browser.click
browser.type
browser.read
```

for:

```text
current job application task
```

rather than permanently.

---

# 99. Expiring Capabilities

Sensitive capabilities should expire automatically.

---

# 100. Tool Confirmation

A tool can declare:

```yaml
confirmation:
  required: true
  reason: "External submission"
```

---

# 101. Confirmation Policies

User settings can define:

```text
always ask
ask once per task
ask for new domain
never ask for low-risk operations
```

---

# 102. Trusted Skill

A user may trust:

```text
Spotify
```

to play music without confirmation.

But that does not automatically trust:

```text
Spotify file access
```

---

# 103. Capability Granularity

Permissions should be granular.

Bad:

```text
spotify.full_access
```

Better:

```text
spotify.playback
spotify.search
```

---

# 104. Skill Health

Each skill exposes:

```text
installed
loaded
ready
degraded
failed
```

---

# 105. Health Check

Example:

```python
{
    "status": "degraded",
    "reason": "Browser not available"
}
```

---

# 106. Graceful Failure

If Spotify is unavailable:

> "Spotify isn't running. Would you like me to start it?"

---

# 107. Retry Policy

Retries should be declared per tool.

Example:

```text
network GET → retry
click submit → do NOT blindly retry
```

---

# 108. Idempotency

Tools declare:

```text
idempotent=true/false
```

---

# 109. Non-Idempotent Actions

Examples:

```text
send email
submit application
delete file
purchase
transfer money
```

must not be automatically retried without policy.

---

# 110. Tool Timeout

Every tool gets:

```text
timeout
```

based on operation type.

---

# 111. Cancellation

User:

> "Stop."

should cancel:

```text
current workflow
pending tool calls
queued actions
```

where safely possible.

---

# 112. Emergency Stop

Global:

```text
JARVIS STOP
```

should immediately stop new external actions.

---

# 113. Partial Workflow Failure

If step 4 fails:

```text
steps 1–3 remain recorded
```

JARVIS should resume from a safe checkpoint.

---

# 114. Workflow Checkpoint

Store:

```text
completed steps
current step
inputs
outputs
browser state
application state
```

but not secrets.

---

# 115. Resume

User:

> "Continue."

Planner loads checkpoint.

---

# 116. Skill Observability

Every execution should generate:

```text
skill
tool
task
duration
status
error
risk
```

---

# 117. Logging

Do not log:

```text
passwords
tokens
private messages
sensitive form values
```

---

# 118. Error Model

Standardize:

```text
VALIDATION_ERROR
PERMISSION_DENIED
AUTH_REQUIRED
CAPTCHA_REQUIRED
NOT_FOUND
TIMEOUT
RATE_LIMITED
UNSUPPORTED
EXTERNAL_FAILURE
USER_CANCELLED
```

---

# 119. Authentication Required

Tool returns:

```json
{
  "status": "auth_required",
  "service": "linkedin"
}
```

JARVIS:

> "LinkedIn needs you to sign in."

---

# 120. Credential Broker

Skills must request credentials through:

```text
Credential Service
```

not directly read credential files.

---

# 121. Credential Flow

```text
Skill
 ↓
credential.request("linkedin")
 ↓
Credential Service
 ↓
secure vault / OS credential store
 ↓
credential session
 ↓
Skill
```

---

# 122. Password Never Goes Into LLM Context

The LLM should receive:

```text
authentication_available = true
```

not:

```text
password = "..."
```

---

# 123. MFA

If MFA is required:

> "LinkedIn needs verification. Please complete it."

JARVIS can continue after successful authentication.

---

# 124. CAPTCHA

JARVIS should not attempt to bypass CAPTCHA.

It can:

```text
detect
pause
ask user
resume
```

---

# 125. Passkeys

Passkeys should use platform/browser authentication flows.

The model must never receive private key material.

---

# 126. Human-in-the-Loop

Skills can declare:

```text
requires_human
```

for:

```text
CAPTCHA
MFA
ambiguous legal declaration
high-risk action
```

---

# 127. Interactive Skill

The skill runtime supports:

```text
pause
ask
resume
```

---

# 128. Ask User API

```python
answer = await interaction.ask(
    question="What salary expectation should I enter?"
)
```

---

# 129. Voice Question

TTS:

> "Sir, what salary expectation should I enter?"

User:

> "Ten lakh."

Pipeline:

```text
STT
→ answer
→ workflow resume
```

---

# 130. Skill State Machine

Useful states:

```text
IDLE
RUNNING
WAITING_FOR_USER
WAITING_FOR_AUTH
WAITING_FOR_CONFIRMATION
FAILED
COMPLETED
CANCELLED
```

---

# 131. Long-Running Skills

Examples:

```text
job search
document indexing
large downloads
software installation
```

must run asynchronously.

---

# 132. Progress Reporting

JARVIS:

> "I've found 27 matching jobs. I'm checking the first ten."

---

# 133. Skill Notifications

Events:

```text
started
progress
waiting
completed
failed
```

---

# 134. Skill Output Types

Possible:

```text
TEXT
STRUCTURED_DATA
FILE
IMAGE
AUDIO
URL
APPLICATION_STATE
TASK
```

---

# 135. Skill UI

Some skills may expose optional UI:

```text
settings
status
history
permissions
```

---

# 136. Plugin Marketplace

A future plugin system may support:

```text
browse
install
update
disable
remove
```

---

# 137. Plugin Verification

Before installation:

```text
signature
publisher
hash
permissions
dependencies
```

---

# 138. Signed Plugins

Recommended:

```text
publisher signs package
```

JARVIS verifies signature before installation.

---

# 139. Plugin Trust Levels

```text
OFFICIAL
VERIFIED
USER_LOCAL
THIRD_PARTY
UNTRUSTED
```

---

# 140. Untrusted Plugins

Untrusted plugins should not receive:

```text
credentials
sensitive memory
admin privileges
```

unless explicitly authorized.

---

# 141. Plugin Updates

Update process:

```text
download
verify
stage
test
switch
rollback
```

---

# 142. Atomic Updates

Never partially replace an active plugin.

Use:

```text
versioned directories
```

and an atomic pointer/symlink switch where supported.

---

# 143. Rollback

If health check fails:

```text
previous version
```

is restored.

---

# 144. Plugin Compatibility Tests

Before enabling:

```text
manifest validation
dependency check
tool schema validation
health check
```

---

# 145. Plugin Dependency Lockfile

Maintain:

```text
plugin-lock.yaml
```

with exact versions and hashes.

---

# 146. Skill Registry Database

Possible schema:

```sql
CREATE TABLE plugins (
    id TEXT PRIMARY KEY,
    version TEXT NOT NULL,
    status TEXT NOT NULL,
    trust_level TEXT NOT NULL,
    manifest_json TEXT NOT NULL,
    installed_at TEXT NOT NULL
);
```

---

# 147. Skill Table

```sql
CREATE TABLE skills (
    id TEXT PRIMARY KEY,
    plugin_id TEXT NOT NULL,
    version TEXT NOT NULL,
    status TEXT NOT NULL,
    manifest_json TEXT NOT NULL
);
```

---

# 148. Tool Table

```sql
CREATE TABLE tools (
    id TEXT PRIMARY KEY,
    skill_id TEXT NOT NULL,
    name TEXT NOT NULL,
    schema_json TEXT NOT NULL,
    risk_level TEXT NOT NULL,
    permissions_json TEXT NOT NULL
);
```

---

# 149. Permission Grant Table

```sql
CREATE TABLE capability_grants (
    id TEXT PRIMARY KEY,
    skill_id TEXT NOT NULL,
    capability TEXT NOT NULL,
    scope_json TEXT,
    expires_at TEXT,
    created_at TEXT NOT NULL
);
```

---

# 150. Skill Configuration Table

```sql
CREATE TABLE skill_config (
    skill_id TEXT PRIMARY KEY,
    config_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

---

# 151. Plugin Directory

Recommended:

```text
plugins/
├── official/
├── verified/
├── local/
└── disabled/
```

---

# 152. Skill Package Layout

```text
com.jarvis.spotify/
├── manifest.yaml
├── skill.py
├── tools/
├── adapters/
├── schemas/
├── tests/
├── assets/
└── README.md
```

---

# 153. Cross-Platform Adapter Layout

```text
spotify/
├── core/
├── adapters/
│   ├── windows/
│   ├── linux/
│   └── android/
└── tools/
```

---

# 154. Browser Plugin Layout

```text
browser/
├── core/
├── chromium/
├── firefox/
├── accessibility/
├── dom/
├── vision/
└── tools/
```

---

# 155. Skill SDK

Create a JARVIS Skill SDK so new developers do not need to understand core internals.

Example:

```python
from jarvis_sdk import Skill, tool

class ExampleSkill(Skill):

    @tool(
        name="hello",
        risk="R0"
    )
    async def hello(self, name: str):
        return {"message": f"Hello {name}"}
```

---

# 156. SDK Responsibilities

SDK should provide:

```text
tool registration
schemas
logging
configuration
permissions
memory access
credential requests
interaction
events
HTTP client
IPC
```

---

# 157. Skill Context

SDK context:

```python
context.user
context.device
context.task
context.permissions
context.memory
context.credentials
context.interaction
context.logger
```

---

# 158. No Direct Core Imports

Third-party skills should not import arbitrary JARVIS internal modules.

Use:

```text
SDK interfaces
```

to preserve compatibility.

---

# 159. Skill SDK Versioning

Plugins declare:

```text
sdk_version
```

---

# 160. Tool Schema Generation

Decorators can generate:

```text
JSON Schema
```

from type annotations.

---

# 161. Pydantic

Pydantic can validate:

```text
tool inputs
tool outputs
configuration
manifest
```

---

# 162. Example Tool

```python
class SearchJobsInput(BaseModel):
    query: str
    location: str | None = None
    remote: bool = False
```

---

# 163. Tool Validation

Reject:

```text
missing required fields
wrong types
invalid enum
oversized input
```

before execution.

---

# 164. Output Validation

Tool outputs should conform to schemas.

---

# 165. Model Tool Calling

The LLM receives:

```text
tool name
description
schema
```

The planner converts model output into a validated execution request.

---

# 166. Never Trust Model Tool Arguments

Even if the LLM returns:

```json
{"path": "/important/file"}
```

the runtime still validates:

```text
path policy
permissions
risk
```

---

# 167. Tool Call Pipeline

```text
LLM
 ↓
tool call
 ↓
schema validation
 ↓
policy check
 ↓
risk evaluation
 ↓
confirmation if required
 ↓
execution
 ↓
result validation
 ↓
LLM
```

---

# 168. Planner vs Skill

Planner:

```text
what should happen next?
```

Skill:

```text
how do I execute this capability?
```

---

# 169. Example

Planner:

```text
Need to play a song.
```

Skill:

```text
spotify.search
spotify.play
```

---

# 170. Workflow Planner

Complex tasks:

```text
goal
 ↓
subgoals
 ↓
skills
 ↓
tools
 ↓
observations
 ↓
next action
```

---

# 171. Observation

After every significant UI action, retrieve fresh state.

Example:

```text
click Apply
 ↓
read page
 ↓
determine next state
```

---

# 172. Avoid Blind Action Chains

Do not execute:

```text
click
click
click
type
submit
```

without verifying state.

---

# 173. Skill Observation API

```python
await skill.observe()
```

returns:

```text
current state
available actions
errors
authentication state
```

---

# 174. Application State

Skill adapters should expose normalized state.

Example:

```json
{
  "application": "chrome",
  "focused": true,
  "url": "...",
  "logged_in": true
}
```

---

# 175. State Verification

Before risky action:

```text
verify expected state
```

---

# 176. Browser Job Example

Before submission:

```text
verify:
- correct company
- correct role
- correct resume
- required fields complete
- no unanswered mandatory questions
```

---

# 177. Skill Recovery

If selector breaks:

```text
DOM
 ↓
accessibility
 ↓
vision
 ↓
ask user
```

---

# 178. Adaptive Skills

Skills should not depend on one selector.

Prefer semantic targets:

```text
role=button
name=Apply
```

---

# 179. Website Changes

The LinkedIn skill should have:

```text
site adapter version
selectors
fallback strategies
health tests
```

---

# 180. Site Adapter

```text
linkedin/
├── current/
├── selectors/
├── workflows/
└── tests/
```

---

# 181. Website Detection

Detect:

```text
domain
page type
authentication state
```

before using site-specific actions.

---

# 182. Login Detection

Possible signals:

```text
account menu
profile icon
login button absence
known authenticated DOM
```

Do not assume login from one weak signal.

---

# 183. Authentication Recovery

If login required:

```text
pause
notify user
wait
resume
```

---

# 184. Browser Profile

Use a dedicated browser profile if desired:

```text
JARVIS automation profile
```

This can improve isolation.

---

# 185. User Browser Profile

Allow user to choose:

```text
personal
work
automation
```

---

# 186. Cookie Handling

Skills should not directly extract cookies unless explicitly required by an authorized integration.

Prefer browser-native authenticated sessions.

---

# 187. API Integration

If an official API exists:

```text
prefer API
```

over UI automation.

---

# 188. API Credentials

Use Credential Service.

---

# 189. Rate Limits

Skills should respect:

```text
API rate limits
website rate limits
robot policies
```

---

# 190. Abuse Prevention

JARVIS should not perform:

```text
mass account creation
spam
credential attacks
CAPTCHA bypass
```

---

# 191. Job Application Rate

If applying to many jobs, support:

```text
daily limits
per-site limits
review queue
```

to avoid unintended mass actions.

---

# 192. Skill Scheduling

Skills can expose scheduled workflows.

Example:

```text
check new SDE jobs every morning
```

The scheduler invokes:

```text
job-search skill
```

---

# 193. Skill Notifications

JARVIS can report:

```text
3 new matching jobs
```

without automatically applying unless authorized.

---

# 194. Skill Memory

Skills can have scoped persistent state.

Example:

```text
LinkedIn search filters
```

but not credentials.

---

# 195. Skill State Storage

Use:

```text
skill_state
```

with namespaced keys.

---

# 196. Skill State Schema

```sql
CREATE TABLE skill_state (
    skill_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(skill_id, key)
);
```

---

# 197. Plugin Telemetry

Local telemetry can track:

```text
tool latency
failure rates
usage
```

without sending data externally.

---

# 198. Privacy

Telemetry should be:

```text
local by default
```

---

# 199. Plugin Crash Isolation

If a plugin crashes:

```text
core continues running
```

---

# 200. Process Isolation

Third-party plugins should ideally run outside the main process.

---

# 201. Plugin Resource Limits

Possible limits:

```text
CPU
RAM
network
filesystem
process count
execution time
```

---

# 202. Plugin Watchdog

Monitor:

```text
heartbeat
memory
CPU
crashes
```

---

# 203. Automatic Restart

Safe plugins can restart after crashes.

---

# 204. Crash Loop

If plugin repeatedly crashes:

```text
disable
```

and notify user.

---

# 205. Plugin Health History

Track:

```text
crashes
last error
last successful execution
```

---

# 206. Skill Testing

Every skill should include:

```text
unit tests
integration tests
permission tests
failure tests
platform tests
```

---

# 207. Browser Skill Tests

Test:

```text
navigate
read
click
type
upload
authentication
selector changes
timeouts
```

---

# 208. OS Skill Tests

Use mocks for:

```text
process launch
window management
volume
filesystem
```

where possible.

---

# 209. Destructive Action Tests

Use sandbox directories.

Never test:

```text
filesystem.delete
```

against real user directories.

---

# 210. Skill Simulation

Create:

```text
MockSkillRuntime
```

for automated planner tests.

---

# 211. Tool Dry Run

Tools can support:

```text
dry_run=true
```

for previewing effects.

---

# 212. Example

Before deleting:

```text
dry run:
3 files would be deleted.
```

---

# 213. Preview

For consequential actions:

```text
plan
↓
preview
↓
confirm
↓
execute
```

---

# 214. Batch Actions

For multiple actions:

```text
show summary
```

rather than asking 20 confirmations.

---

# 215. Confirmation Grouping

Example:

> "I found 12 matching jobs. I can prepare applications for all 12, but submission will require approval for each or for the batch. What would you prefer?"

---

# 216. Batch Policy

Support:

```text
confirm_each
confirm_batch
auto_approve
```

for authorized operations.

---

# 217. User Policy

Example:

```yaml
policies:
  spotify.play:
    confirmation: never

  linkedin.submit_application:
    confirmation: batch

  filesystem.delete:
    confirmation: always
```

---

# 218. Policy Hierarchy

Recommended:

```text
system safety
>
user security policy
>
skill policy
>
task policy
>
model suggestion
```

---

# 219. Skill Policy Cannot Override System Safety

A plugin cannot request:

```text
bypass security
```

and override core policy.

---

# 220. Skill Policy Cannot Grant Itself Permissions

Permissions must come from:

```text
policy engine
```

---

# 221. User Policy Changes

Sensitive policy changes require:

```text
explicit user action
```

---

# 222. Policy Audit

Record:

```text
policy change
old value
new value
timestamp
device
```

---

# 223. Skill Installation UI

Show:

```text
Skill name
Publisher
Version
Permissions
Network domains
Filesystem access
Risk
```

before installation.

---

# 224. Example Installation Screen

```text
LinkedIn Skill

Permissions:
✓ Browser read
✓ Browser type
✓ Browser click
✓ File read: selected resume folder
✓ Network: linkedin.com

High-risk actions:
! Submit applications
```

---

# 225. Plugin Removal

Removing a plugin should offer:

```text
remove plugin
remove plugin data
retain task history
```

---

# 226. Plugin Data Deletion

User should be able to remove:

```text
plugin state
cached data
indexes
logs
```

---

# 227. Skill Package Signing

Package:

```text
plugin.zip
signature
manifest
hash
```

---

# 228. Local Skills

Developer can install:

```text
--dev
```

skills from a local directory.

---

# 229. Development Mode

Development plugins may have:

```text
verbose logging
hot reload
debug tools
```

but should not automatically receive production privileges.

---

# 230. Hot Reload

Useful during development:

```text
edit skill
 ↓
reload
 ↓
test
```

---

# 231. Production Mode

Disable:

```text
hot reload
debug shell
verbose sensitive logs
```

---

# 232. Skill SDK Languages

Recommended primary:

```text
Python
```

because JARVIS AI/automation components will largely use Python.

Possible future:

```text
TypeScript
Rust
Kotlin
```

through RPC interfaces.

---

# 233. Why Python First

Python has strong ecosystems for:

```text
AI
automation
browser control
OCR
document processing
speech
system integration
```

---

# 234. Native Performance Components

Use:

```text
Rust/C++
```

when necessary for:

```text
low-level OS hooks
high-performance vision
audio
secure native helpers
```

---

# 235. Android Skills

Android-native skills should primarily use:

```text
Kotlin
```

and communicate with JARVIS core through defined APIs.

---

# 236. Windows Native Skills

Possible:

```text
C#
C++
Rust
Python
```

depending on the integration.

---

# 237. Ubuntu Native Skills

Possible:

```text
Python
Rust
C/C++
```

---

# 238. Cross-Platform RPC

Use a stable local RPC protocol.

Possible:

```text
gRPC
Unix domain sockets
Named pipes
local HTTP
```

---

# 239. Recommended Initial IPC

Use:

```text
localhost HTTP/gRPC
```

for ease of development.

Use:

```text
Unix sockets
Windows named pipes
```

later for tighter security/performance where needed.

---

# 240. Skill Host

Architecture:

```text
JARVIS Core
    │
    ▼
Skill Host
    │
    ├── Built-in skill
    └── Plugin process
```

---

# 241. Skill Host Responsibilities

```text
load
authenticate
authorize
invoke
monitor
terminate
restart
```

---

# 242. Skill Manifest Validation

Validate:

```text
schema
permissions
entrypoint
dependencies
platform
version
```

before loading.

---

# 243. Malformed Skill

Result:

```text
installation rejected
```

not:

```text
partially installed
```

---

# 244. Skill Discovery from User Request

Example:

> "Open Spotify."

Router searches:

```text
application.launch
spotify
```

and selects the correct skill.

---

# 245. Unknown Capability

If no skill supports a request:

> "I don't currently have a capability for that."

Optionally:

```text
suggest plugin
```

if an installed marketplace/registry is available.

---

# 246. Skill Generation

Future JARVIS could generate a new skill skeleton.

But generated skills must undergo:

```text
validation
sandboxing
testing
permission review
```

before activation.

---

# 247. Self-Extending JARVIS

Architecture:

```text
User request
 ↓
capability missing
 ↓
generate implementation
 ↓
test in sandbox
 ↓
review permissions
 ↓
install
```

Never:

```text
generate
→ immediately run with full privileges
```

---

# 248. Skill Repair

If a site adapter breaks:

```text
detect failure
 ↓
collect safe diagnostic state
 ↓
attempt known fallback
 ↓
optionally update adapter
 ↓
test
```

---

# 249. Skill Learning

JARVIS can learn:

```text
preferred workflow
preferred application
preferred form values
```

but these become memory, not executable code.

---

# 250. Workflow Templates

A skill can provide reusable workflows:

```text
apply_to_job
download_report
create_project
backup_folder
```

---

# 251. Workflow Definition

Example:

```yaml
id: linkedin.apply
steps:
  - search_jobs
  - open_job
  - extract_application
  - fill_application
  - review
  - submit
```

---

# 252. Conditional Workflow

```yaml
if:
  auth_required: true

then:
  wait_for_user_login
```

---

# 253. Workflow Variables

Use:

```text
job
resume
profile
company
location
```

---

# 254. Workflow Secret Variables

Credentials must not be normal variables.

Use:

```text
credential reference
```

---

# 255. Workflow Checkpoints

Persist after consequential steps.

---

# 256. Workflow Rollback

Some operations can have compensation actions.

Example:

```text
create calendar event
```

can potentially be compensated with:

```text
delete created event
```

---

# 257. Not All Actions Are Reversible

The system must distinguish:

```text
reversible
partially reversible
irreversible
```

---

# 258. Irreversible Actions

Examples:

```text
submit application
send message
delete data
purchase
```

require stronger policy.

---

# 259. Skill Risk Declaration

Every workflow declares maximum risk.

Example:

```text
linkedin.apply = R3
```

---

# 260. Planner Risk Propagation

If workflow includes:

```text
submit_application
```

the entire plan should know:

```text
external consequential action
```

---

# 261. Confirmation at Boundary

Do not ask for confirmation at every harmless step.

Ask at:

```text
risk boundary
```

Example:

```text
prepare application → no confirmation
submit application → confirmation
```

---

# 262. User Intent Preservation

JARVIS should never expand:

> "Find jobs"

into:

> "Apply to jobs."

Search and application are different capabilities.

---

# 263. Scope Preservation

If user says:

> "Apply only to remote SDE jobs."

all subsequent filtering must preserve:

```text
role = SDE
remote = true
```

---

# 264. Constraint Tracking

Planner stores:

```text
hard constraints
soft preferences
```

---

# 265. Skill Constraints

Example:

```text
max applications/day = 10
```

---

# 266. User Preferences

Example:

```text
preferred cities
minimum salary
remote
experience
technologies
```

These should come from authorized memory.

---

# 267. Application Matching

The skill can compute:

```text
job
+
profile
+
resume
```

to estimate fit.

---

# 268. Skill Should Not Fabricate

If a job requires:

```text
5 years experience
```

and user has:

```text
1 year
```

JARVIS should not claim 5 years.

---

# 269. Document Skill

Tools:

```text
search_documents
read_document
summarize
extract
compare
create
```

---

# 270. Code Skill

Tools:

```text
inspect_repository
search_code
run_tests
format
build
git_status
```

---

# 271. GitHub Skill

Tools:

```text
search_repositories
read_issue
read_pr
create_issue
comment
create_branch
open_pr
```

with permissions.

---

# 272. IDE Skill

VS Code tools:

```text
open_project
open_file
search
edit
run_task
read_terminal
```

---

# 273. Music Skill

Tools:

```text
search
play
pause
next
previous
volume
queue
```

---

# 274. Media Skill

Generic abstraction:

```text
play
pause
seek
volume
next
previous
```

Apps can implement it.

---

# 275. Notification Skill

Tools:

```text
notify
read_notifications
dismiss
```

subject to platform permissions.

---

# 276. Clipboard Skill

Tools:

```text
read
write
clear
```

Clipboard reads should be permission-controlled because they may contain secrets.

---

# 277. Screenshot Skill

Tools:

```text
capture_screen
capture_window
capture_region
```

Screenshots are sensitive and should be treated accordingly.

---

# 278. Camera Skill

Android/desktop camera access:

```text
camera.capture
```

requires explicit device permission.

---

# 279. Microphone Skill

Voice pipeline owns microphone access.

Skills should not independently capture audio unless authorized.

---

# 280. Location Skill

Location should be:

```text
explicit permission
```

and scoped.

---

# 281. Device Capability Discovery

JARVIS should detect:

```text
camera
microphone
GPU
NPU
Bluetooth
Wi-Fi
screen
accessibility
```

and expose capability status.

---

# 282. Skill Selection Based on Hardware

Example:

```text
vision model available on GPU
```

→ enable local vision skill.

---

# 283. Skill Selection Based on Platform

Example:

```text
Windows
```

→ Windows UI automation.

```text
Ubuntu
```

→ Linux UI automation.

```text
Android
```

→ AccessibilityService.

---

# 284. Skill Selection Based on App

If Chrome is focused:

```text
browser skill
```

gets priority.

If VS Code is focused:

```text
IDE skill
```

may get priority.

---

# 285. Contextual Skill Routing

Current screen/app is useful context for routing.

---

# 286. Example

User:

> "Close this."

If current window is Chrome:

```text
window.close(chrome)
```

The system should not need a hard-coded app name.

---

# 287. Ambiguous Requests

User:

> "Open it."

If multiple candidates exist:

> "Which one should I open?"

---

# 288. Skill Capability Search

The planner can ask:

```text
registry.find_capabilities("open application")
```

---

# 289. Skill Registry API

```python
registry.find_skills(
    intent="play music"
)

registry.find_tools(
    capability="browser.click"
)
```

---

# 290. Skill Package Manager API

```python
plugin.install(package)
plugin.update(id)
plugin.disable(id)
plugin.uninstall(id)
plugin.verify(package)
```

---

# 291. Skill Settings API

```python
settings.get(skill_id)
settings.set(skill_id, values)
```

---

# 292. Skill Permission API

```python
permissions.request(...)
permissions.grant(...)
permissions.revoke(...)
```

---

# 293. Skill Event API

```python
events.emit(...)
events.subscribe(...)
```

---

# 294. Skill Interaction API

```python
interaction.ask(...)
interaction.confirm(...)
interaction.notify(...)
```

---

# 295. Skill Memory API

```python
memory.search(scope=...)
memory.remember(candidate=...)
```

with policy enforcement.

---

# 296. Skill Credential API

```python
credentials.request(
    service="linkedin",
    purpose="authentication"
)
```

---

# 297. Skill Network API

Prefer a controlled client:

```python
network.request(...)
```

instead of unrestricted sockets.

---

# 298. Skill File API

Use:

```python
filesystem.read_scoped(...)
```

rather than:

```python
open("/any/path")
```

for sandboxed skills.

---

# 299. Skill Process API

Use:

```python
process.run_scoped(...)
```

with policy.

---

# 300. Complete Skill Execution Example

User:

> "Apply to the best remote SDE jobs on LinkedIn."

Pipeline:

```text
Voice
 ↓
STT
 ↓
Intent
 ↓
Planner
 ↓
Memory retrieval
 ↓
Skill discovery
 ↓
LinkedIn skill
 ↓
Browser skill
 ↓
Job search
 ↓
Filtering
 ↓
Job scoring
 ↓
Resume retrieval
 ↓
Application preparation
 ↓
Missing information check
 ↓
User interaction if needed
 ↓
Application review
 ↓
Confirmation
 ↓
Submit
 ↓
Result
 ↓
Task memory
```

---

# 301. Job Skill Internal Tools

```text
linkedin.search_jobs
linkedin.open_job
linkedin.extract_job
linkedin.start_application
linkedin.fill_field
linkedin.upload_resume
linkedin.review_application
linkedin.submit_application
```

---

# 302. Browser Interaction

The LinkedIn skill should not directly manipulate Chrome internals if possible.

Instead:

```text
LinkedIn skill
→ Browser skill
```

This maintains separation.

---

# 303. Resume Retrieval

```text
LinkedIn Skill
→ Memory/Document Service
```

---

# 304. Credential Retrieval

```text
LinkedIn Skill
→ Credential Service
```

---

# 305. Confirmation

```text
LinkedIn Skill
→ Interaction Service
```

---

# 306. This Is Skill Composition

Each component has one responsibility.

---

# 307. Core Does Not Know LinkedIn Internals

Core knows:

```text
capability = job application
```

Skill knows:

```text
LinkedIn-specific workflow
```

---

# 308. Browser Does Not Know Job Logic

Browser knows:

```text
click
type
read
navigate
```

It does not know:

```text
what an SDE job is
```

---

# 309. Memory Does Not Know UI

Memory knows:

```text
profile
preferences
documents
history
```

not:

```text
click Apply
```

---

# 310. Credential Service Does Not Know Workflow

It knows:

```text
credential retrieval
```

not:

```text
which jobs to apply to
```

---

# 311. Separation of Concerns

This separation is one of the most important JARVIS architectural rules.

---

# 312. Recommended Monorepo

```text
jarvis/
├── apps/
│   ├── windows/
│   ├── ubuntu/
│   └── android/
│
├── core/
│   ├── agent/
│   ├── planner/
│   ├── policy/
│   ├── memory/
│   └── runtime/
│
├── sdk/
│   ├── python/
│   └── schemas/
│
├── skills/
│   ├── browser/
│   ├── filesystem/
│   ├── terminal/
│   ├── windows/
│   ├── linux/
│   ├── android/
│   ├── spotify/
│   ├── linkedin/
│   ├── github/
│   └── vscode/
│
├── plugins/
│   └── external/
│
├── protocols/
├── tests/
└── docs/
```

---

# 313. Package Ownership

Core owns:

```text
agent
planner
policy
runtime
registry
```

Skills own:

```text
application-specific behavior
```

---

# 314. Skill Development Workflow

```text
create manifest
 ↓
define capabilities
 ↓
define tools
 ↓
implement adapter
 ↓
add tests
 ↓
run sandbox
 ↓
validate permissions
 ↓
install locally
 ↓
integration test
```

---

# 315. Skill Test Harness

Provide:

```text
mock browser
mock filesystem
mock credentials
mock memory
mock interaction
```

so developers can test without real accounts.

---

# 316. Golden Workflows

Create deterministic test workflows.

Example:

```text
LinkedIn login page
→ mock authentication
→ search
→ open result
→ fill form
→ confirmation
→ submit
```

---

# 317. Regression Tests

Every website/app adapter update should run:

```text
existing workflows
```

before deployment.

---

# 318. Visual Regression

For UI skills, store:

```text
screenshots
accessibility trees
DOM snapshots
```

where legally and technically appropriate.

---

# 319. Skill Reliability Score

Track:

```text
success rate
failure rate
average latency
recovery rate
```

---

# 320. Skill Health Ranking

Planner can prefer:

```text
healthy skill
```

over:

```text
degraded skill
```

---

# 321. Automatic Skill Disable

If repeated catastrophic failures occur:

```text
disable
notify
rollback
```

---

# 322. Skill Telemetry Example

```json
{
  "skill": "linkedin",
  "tool": "submit_application",
  "duration_ms": 2100,
  "status": "success",
  "risk": "R3"
}
```

No sensitive form values.

---

# 323. Skill Error Example

```json
{
  "status": "auth_required",
  "message": "Authentication required",
  "recoverable": true
}
```

---

# 324. Human Interaction Contract

A skill can pause with:

```json
{
  "status": "waiting_for_user",
  "prompt": "Please complete MFA"
}
```

The runtime resumes the same task after interaction.

---

# 325. Task Persistence

The skill runtime must associate:

```text
skill execution
```

with:

```text
task ID
```

so execution can resume after restart.

---

# 326. Crash Recovery

After JARVIS restarts:

```text
load incomplete tasks
 ↓
check workflow checkpoints
 ↓
verify current external state
 ↓
resume or ask user
```

---

# 327. Never Blindly Resume External Actions

A restart should not automatically repeat:

```text
submit
send
purchase
delete
```

without revalidation.

---

# 328. Skill-Level Recovery Policy

Each tool declares:

```text
resume_safe
```

Example:

```text
browser.read → true
submit_application → false
```

---

# 329. Offline Skills

Some skills work entirely offline:

```text
filesystem
calculator
local media
local documents
memory
```

---

# 330. Online Skills

Others require:

```text
network
```

and must report offline state.

---

# 331. Network Availability

Skill runtime can expose:

```text
network_available
```

---

# 332. Offline Fallback

If Spotify API unavailable:

```text
local media skill
```

could still play local files.

---

# 333. Capability Fallback Graph

Example:

```text
play_music
 ├── Spotify
 ├── YouTube Music
 └── Local Media
```

Planner chooses based on availability/preferences.

---

# 334. Skill Priority

User preferences can influence priority:

```text
Spotify > YouTube Music > local
```

---

# 335. Application Detection

JARVIS can detect installed applications:

```text
Chrome
Firefox
VS Code
Spotify
```

and register relevant skills dynamically.

---

# 336. Installed Application Registry

Maintain:

```text
app ID
display name
executable
platform
version
launch method
```

---

# 337. App Launch Skill

Generic:

```text
application.launch
application.close
application.focus
application.is_running
```

---

# 338. App-Specific Skill

Then:

```text
spotify.play
vscode.open_file
chrome.navigate
```

---

# 339. App Identity

Use stable identifiers where available:

```text
Windows AppUserModelId
Android package name
Linux desktop file ID
```

---

# 340. Cross-Platform App Registry

Normalized:

```text
app_id
platform
native_id
```

---

# 341. Skill Manifest Example — Browser

```yaml
id: com.jarvis.browser
version: 1.0.0

capabilities:
  - browser.navigate
  - browser.read
  - browser.click
  - browser.type
  - browser.screenshot

permissions:
  - screen.read
  - input.control
  - network.browser
```

---

# 342. Skill Manifest Example — Filesystem

```yaml
id: com.jarvis.filesystem
version: 1.0.0

capabilities:
  - filesystem.read
  - filesystem.write

permissions:
  - filesystem.scoped_read
  - filesystem.scoped_write
```

---

# 343. Skill Manifest Example — Terminal

```yaml
id: com.jarvis.terminal
version: 1.0.0

capabilities:
  - terminal.execute

permissions:
  - process.execute
```

with strong command policy.

---

# 344. Skill Manifest Example — LinkedIn

```yaml
id: com.jarvis.linkedin
version: 1.0.0

capabilities:
  - jobs.search
  - jobs.apply

permissions:
  - browser.read
  - browser.click
  - browser.type
  - browser.upload
  - credential.request
  - memory.read
```

---

# 345. Skill Manifest Example — GitHub

```yaml
id: com.jarvis.github
version: 1.0.0

capabilities:
  - github.repository.read
  - github.issue.read
  - github.issue.write
  - github.pull_request.write

permissions:
  - network.github
  - credential.request
```

---

# 346. Plugin Marketplace Metadata

Optional fields:

```text
publisher
homepage
source_repository
license
documentation
security_contact
```

---

# 347. Plugin Review

Official/verified plugins should pass:

```text
static analysis
dependency scan
permission review
integration tests
```

---

# 348. Dependency Security

Scan:

```text
Python dependencies
Node dependencies
native libraries
```

for known vulnerabilities.

---

# 349. Plugin Lockdown

User can globally disable:

```text
third-party plugins
```

---

# 350. Developer Mode

Developer can allow:

```text
unsigned local plugins
```

but only with explicit local configuration.

---

# 351. Production Rule

Production JARVIS should default to:

```text
signed/verified plugins
```

where applicable.

---

# 352. Skill SDK Documentation

Each skill developer should know:

```text
manifest
tool definition
permissions
memory
credentials
interaction
events
testing
deployment
```

---

# 353. Skill Generator

Provide CLI:

```bash
jarvis skill create spotify
```

Generates:

```text
manifest
skill
tools
tests
README
```

---

# 354. Skill Validator

```bash
jarvis skill validate ./spotify
```

checks:

```text
manifest
schemas
permissions
dependencies
```

---

# 355. Skill Test

```bash
jarvis skill test ./spotify
```

---

# 356. Skill Package

```bash
jarvis skill package ./spotify
```

---

# 357. Skill Install

```bash
jarvis skill install spotify.jarvis-plugin
```

---

# 358. Skill List

```bash
jarvis skill list
```

---

# 359. Skill Inspect

```bash
jarvis skill inspect com.jarvis.spotify
```

---

# 360. Skill Permissions

```bash
jarvis skill permissions com.jarvis.spotify
```

---

# 361. Skill Disable

```bash
jarvis skill disable com.jarvis.spotify
```

---

# 362. Skill Update

```bash
jarvis skill update com.jarvis.spotify
```

---

# 363. Skill Rollback

```bash
jarvis skill rollback com.jarvis.spotify
```

---

# 364. Skill Architecture Summary

```text
                 USER
                   │
                   ▼
              JARVIS CORE
                   │
                   ▼
               PLANNER
                   │
                   ▼
             SKILL ROUTER
                   │
           ┌───────┼────────┐
           ▼       ▼        ▼
        Browser  OS      Services
           │       │        │
           ▼       ▼        ▼
        Tools   Tools     Tools
           │       │        │
           └───────┼────────┘
                   ▼
             POLICY ENGINE
                   │
                   ▼
              TOOL RUNTIME
                   │
                   ▼
        ┌──────────┼──────────┐
        ▼          ▼          ▼
       OS        Browser     APIs
```

---

# 365. Final Design Rules

1. The LLM never directly controls the OS.
2. Every executable capability is a tool.
3. Tools belong to skills.
4. Plugins package skills.
5. Skills declare capabilities explicitly.
6. Permissions use least privilege.
7. Tool arguments are validated independently of the LLM.
8. Tool results are untrusted data.
9. External actions have explicit risk levels.
10. Non-idempotent actions are not blindly retried.
11. Credentials never enter normal LLM context.
12. CAPTCHA is not bypassed.
13. MFA can pause a workflow for the user.
14. Browser DOM/accessibility/API interfaces are preferred over coordinates.
15. Vision is a fallback for difficult interfaces.
16. Skills should be platform-aware through adapters.
17. Core should not contain application-specific logic.
18. Skills should not bypass the policy engine.
19. Third-party plugins should be isolated where practical.
20. Plugins should be signed/verified in production.
21. Plugin updates should support rollback.
22. Long workflows must have checkpoints.
23. External actions must be revalidated after restart.
24. Skills need health checks.
25. Skills need automated tests.
26. Skill permissions must be auditable.
27. Skill data must be namespaced.
28. Skill network access should be controlled.
29. Skill filesystem access should be scoped.
30. Skill process execution should be policy-controlled.
31. User intent must not silently expand.
32. Missing information should cause JARVIS to ask.
33. JARVIS should never fabricate user information.
34. Current application state should be verified before consequential actions.
35. Skills should compose rather than duplicate functionality.
36. APIs should be preferred over UI automation when available.
37. The system should support offline skills.
38. The system should support graceful degradation.
39. A skill failure must not crash the entire assistant.
40. The Skill SDK should remain stable while internal implementation evolves.

---

# 366. Recommended Initial Skill Set

Build these first:

```text
1. application
2. filesystem
3. process
4. browser
5. UI automation
6. clipboard
7. screenshot
8. audio/media
9. memory
10. documents
11. terminal
12. Windows
13. Ubuntu/Linux
14. Android
15. Spotify
16. GitHub
17. VS Code
18. LinkedIn
19. email
20. calendar
```

---

# 367. Recommended Development Order

## Stage A — Runtime

```text
Skill interface
Tool interface
Registry
Manifest
Policy integration
```

## Stage B — Core Skills

```text
application
filesystem
process
browser
UI
```

## Stage C — Productivity

```text
documents
terminal
GitHub
VS Code
email
calendar
```

## Stage D — Consumer

```text
Spotify
media
notifications
```

## Stage E — High-Level Automation

```text
LinkedIn
job applications
multi-step workflows
```

## Stage F — Plugin Ecosystem

```text
SDK
CLI
signing
sandboxing
marketplace
updates
```

---

# 368. End-State JARVIS Skill Ecosystem

```text
                         JARVIS
                           │
                     ┌─────┴─────┐
                     │   CORE    │
                     └─────┬─────┘
                           │
                       PLANNER
                           │
                     SKILL ROUTER
                           │
       ┌───────────────────┼───────────────────┐
       │                   │                   │
       ▼                   ▼                   ▼
   COMPUTER             INTERNET           PERSONAL
       │                   │                   │
       │                   │                   │
 ┌─────┼─────┐       ┌─────┼─────┐       ┌────┼────┐
 ▼     ▼     ▼       ▼     ▼     ▼       ▼    ▼    ▼
OS   Browser UI     GitHub LinkedIn Email Memory Docs
 │      │     │
 ▼      ▼     ▼
Files  Web   Apps
              │
       ┌──────┼───────┐
       ▼      ▼       ▼
    Spotify VSCode  Other Apps
```

The Skill System is therefore the primary extensibility mechanism of JARVIS. It allows the same local AI core, planner, memory system, security policy, voice system, and computer-use engine to control an expanding ecosystem of applications without turning the core into an unmaintainable collection of application-specific code.
