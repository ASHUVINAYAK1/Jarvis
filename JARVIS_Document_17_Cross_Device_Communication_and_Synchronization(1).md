# JARVIS — Document 17
# Cross-Device Communication & Synchronization

**Status:** Detailed implementation specification  
**Platforms:** Windows, Ubuntu/Linux, Android  
**Architecture:** Local-first, distributed, privacy-preserving personal AI

---

## 1. Purpose

JARVIS is intended to operate as one assistant across Windows, Ubuntu, and Android rather than as three unrelated applications.

The user should be able to:

- start a task on Windows and continue it on Android;
- use Android as a microphone/remote control for the PC-hosted AI;
- use the PC as the primary inference machine when a large local model is available;
- receive task completion notifications on the phone;
- share selected clipboard/context/files between trusted devices;
- transfer an active conversation or workflow;
- route a task to the device that has the required capability;
- remain functional when devices temporarily lose network connectivity.

The architecture therefore needs a **device mesh**, a **secure transport layer**, and a **synchronization/state layer** above the platform-specific implementations.

---

# 2. Design Principles

## 2.1 Local-first

JARVIS must not require a cloud server for normal operation.

Preferred path:

```text
Windows JARVIS
      │
      ├── LAN ─────── Ubuntu JARVIS
      │
      └── LAN/VPN ─── Android JARVIS
```

Cloud services, if ever added, should be optional.

---

## 2.2 One logical assistant

Each installation is a JARVIS node.

```text
                 JARVIS Device Mesh
                         │
        ┌────────────────┼────────────────┐
        │                │                │
     Windows           Ubuntu          Android
        │                │                │
   desktop tools     Linux tools     mobile tools
        │                │                │
        └────────── shared identity ──────┘
                         │
                  shared task state
```

The user should perceive this as one assistant.

---

# 3. Device Roles

A device can have multiple roles.

## 3.1 Coordinator

Normally the most capable trusted PC.

Responsibilities:

- large-model inference;
- global task orchestration;
- model hosting;
- long-running workflows;
- centralized encrypted memory when configured;
- device registry;
- task coordination.

The coordinator is not necessarily permanently fixed.

---

## 3.2 Worker

A device capable of executing specific tools.

Examples:

- Windows desktop automation;
- Ubuntu terminal operations;
- Android notification operations;
- Android camera access;
- media control.

---

## 3.3 Voice endpoint

A device providing:

- microphone;
- wake-word detection;
- VAD;
- TTS;
- conversation UI.

Android can act as a voice endpoint while the PC performs inference.

---

## 3.4 UI endpoint

A device displaying:

- task status;
- confirmation requests;
- authentication requests;
- logs;
- notifications;
- conversation history.

---

# 4. Device Identity

Every installation gets a persistent cryptographic identity.

Example:

```text
device_id:
    jarvis-device-7f2c...

platform:
    windows

installation_id:
    ...

public_key:
    Ed25519 public key

capabilities:
    llm
    vision
    browser
    filesystem
    microphone
    speaker
```

The device ID must not be based only on hostname.

---

# 5. Cryptographic Identity

Recommended design:

```text
Identity key:
    Ed25519

Transport encryption:
    TLS 1.3

Symmetric encryption:
    AES-256-GCM or ChaCha20-Poly1305

Key derivation:
    HKDF-SHA-256
```

Private keys must be stored in platform-protected storage where possible.

### Windows

Use:

- Windows Credential Manager;
- DPAPI;
- TPM-backed protection where available.

### Ubuntu

Use:

- Secret Service/libsecret;
- encrypted local key store;
- TPM integration where available.

### Android

Use:

- Android Keystore;
- hardware-backed keys when available.

---

# 6. Device Pairing

A new device must not automatically become trusted.

Recommended pairing flow:

```text
PC:
    Settings → Devices → Add Device

Android:
    Settings → Pair with JARVIS

PC displays:
    6-digit code + QR

Android scans QR

Both devices perform:
    key exchange
    identity verification
    capability exchange

User confirms:

    "Trust Android phone?"

Pairing completed.
```

The QR code should contain a short-lived pairing payload rather than a permanent secret.

---

# 7. Trust Levels

Use explicit trust levels.

```text
UNPAIRED
    ↓
PAIRED
    ↓
TRUSTED
    ↓
PRIVILEGED
```

A trusted device may communicate with the mesh.

A privileged device may:

- execute high-risk tools;
- access selected shared memory;
- approve remote actions;
- manage other devices.

---

# 8. Capability Discovery

Every device publishes capabilities.

Example:

```json
{
  "device_id": "desktop-01",
  "platform": "windows",
  "capabilities": [
    "browser",
    "desktop_automation",
    "filesystem",
    "terminal",
    "microphone",
    "speaker",
    "llm",
    "vision"
  ]
}
```

Android:

```json
{
  "device_id": "phone-01",
  "platform": "android",
  "capabilities": [
    "microphone",
    "camera",
    "notifications",
    "location",
    "contacts",
    "speaker",
    "voice_ui"
  ]
}
```

Capabilities should include versions and permission states.

---

# 9. Capability Routing

When a task requires a capability, the coordinator selects a suitable device.

Example:

> "Take a photo and analyze it."

Routing:

```text
Camera capability
      ↓
Android
      ↓
Capture image
      ↓
Transfer encrypted image
      ↓
PC vision model
      ↓
Analysis
      ↓
Android TTS
```

---

# 10. Transport Architecture

The recommended hierarchy is:

```text
Application protocol
        ↓
gRPC / WebSocket
        ↓
TLS 1.3
        ↓
TCP or QUIC
        ↓
LAN / trusted VPN
```

### gRPC

Use for:

- request/response APIs;
- typed RPC;
- capability APIs;
- device management.

### WebSocket

Use for:

- realtime events;
- voice streaming;
- task progress;
- notifications;
- interactive sessions.

### QUIC

Consider later for:

- high-latency networks;
- mobile connections;
- multiplexed streaming;
- connection migration.

Do not implement all three initially.

Recommended first implementation:

```text
gRPC + WebSocket + TLS
```

---

# 11. Device Gateway

Each JARVIS installation contains a Device Gateway.

```text
┌─────────────────────────────┐
│        JARVIS Node          │
│                             │
│  AI Engine                  │
│  Agent Runtime              │
│  Skill Runtime              │
│  Memory                     │
│                             │
│  Device Gateway             │
│       │                     │
│  ┌────┴────┐                │
│  │ RPC API │                │
│  │ Events  │                │
│  │ Auth    │                │
│  └─────────┘                │
└─────────────────────────────┘
```

The gateway should be the only network-facing component.

---

# 12. Gateway Responsibilities

The gateway handles:

1. authentication;
2. authorization;
3. device discovery;
4. capability discovery;
5. connection management;
6. RPC;
7. event streaming;
8. task routing;
9. synchronization;
10. rate limiting;
11. replay protection;
12. connection health.

Platform-specific tools should not expose arbitrary network servers.

---

# 13. Protocol Layers

Use four logical layers.

```text
Layer 4 — Application
    commands
    tasks
    memory
    workflows

Layer 3 — Synchronization
    replication
    conflict resolution
    checkpoints

Layer 2 — Device Mesh
    discovery
    pairing
    routing

Layer 1 — Secure Transport
    TLS
    authentication
```

---

# 14. Message Envelope

Every inter-device message should have a standard envelope.

```json
{
  "message_id": "uuid",
  "message_type": "task.progress",
  "sender_device": "desktop-01",
  "recipient_device": "phone-01",
  "timestamp": "2026-08-17T12:00:00Z",
  "correlation_id": "task-123",
  "sequence": 42,
  "payload": {}
}
```

Required properties:

- unique message ID;
- sender;
- recipient;
- timestamp;
- message type;
- correlation ID;
- sequence number.

---

# 15. Event Types

Core events:

```text
device.online
device.offline
device.capabilities_changed

task.created
task.started
task.progress
task.waiting
task.completed
task.failed
task.cancelled

confirmation.requested
confirmation.approved
confirmation.denied

auth.required
auth.completed

memory.created
memory.updated
memory.deleted

conversation.started
conversation.message
conversation.handoff

notification.created

clipboard.updated

file.transfer.started
file.transfer.progress
file.transfer.completed
file.transfer.failed
```

---

# 16. Task Synchronization

Tasks require persistent IDs.

Example:

```json
{
  "task_id": "task-01",
  "parent_task_id": null,
  "status": "WAITING_FOR_USER",
  "owner_device": "desktop-01",
  "executor_device": "desktop-01",
  "created_at": "...",
  "updated_at": "...",
  "checkpoint": {
    "step": 7
  }
}
```

The task state must survive:

- application restart;
- device sleep;
- network loss;
- temporary process failure.

---

# 17. Task Handoff

Example:

> "Continue the job search on my phone."

The PC creates a handoff package.

```text
Task
 ├── objective
 ├── current state
 ├── completed steps
 ├── pending steps
 ├── relevant memory
 ├── browser context
 ├── screenshots
 └── required capabilities
```

The phone receives the package and either:

1. continues locally;
2. requests the PC to execute unavailable operations.

---

# 18. Workflow Checkpoints

Long-running workflows must checkpoint.

Example LinkedIn workflow:

```text
CHECKPOINT 1
Logged into LinkedIn

CHECKPOINT 2
Search completed

CHECKPOINT 3
Candidate jobs selected

CHECKPOINT 4
Application opened

CHECKPOINT 5
Form partially filled

CHECKPOINT 6
User confirmation requested

CHECKPOINT 7
Application submitted
```

If the browser crashes after checkpoint 5, JARVIS should resume from checkpoint 5 rather than restarting.

---

# 19. Offline Operation

Devices must continue operating independently.

Example:

```text
PC offline from phone
        ↓
Android continues:
    wake word
    voice conversation
    local small model
    notifications
    local tools
        ↓
PC reconnects
        ↓
synchronization
```

---

# 20. Offline Queue

Each device should maintain an outbound queue.

```text
SQLite

outbox
------
message_id
type
payload
created_at
retry_count
status
```

When connectivity returns:

```text
queued messages
      ↓
authentication
      ↓
sequence validation
      ↓
delivery
      ↓
acknowledgement
      ↓
delete/compact
```

---

# 21. Idempotency

Every action capable of changing state needs an idempotency key.

Example:

```text
operation_id = 8e7...
```

If a network timeout occurs after an operation executes, JARVIS must not execute it twice blindly.

Especially important for:

- sending messages;
- submitting applications;
- purchases;
- deleting files;
- changing settings;
- sending email.

---

# 22. Conflict Resolution

Use different policies depending on data type.

### Immutable events

Append-only.

### Task state

Last valid state according to server/coordinator sequence.

### User preferences

Versioned updates.

### Memory

Merge based on memory IDs and timestamps.

### Clipboard

Newest version unless explicitly pinned.

Do not use blind "last write wins" for high-risk operations.

---

# 23. Conversation Synchronization

Conversation messages should have:

```text
conversation_id
message_id
sender
timestamp
role
content
attachments
device_id
```

Example:

```text
User → Android:
    "Find me some SDE jobs."

Android → PC:
    conversation.message

PC:
    creates task

PC → Android:
    task.started

Android:
    TTS:
    "I'm searching now, sir."
```

---

# 24. Voice Architecture Across Devices

A microphone can be attached to any device.

Example:

```text
Android microphone
       ↓
VAD
       ↓
Wake word
       ↓
audio stream
       ↓
PC STT
       ↓
LLM
       ↓
PC TTS
       ↓
Android speaker
```

However, for low latency, Android should optionally perform:

- wake word;
- VAD;
- noise suppression;
- streaming audio encoding.

---

# 25. Voice Endpoint Arbitration

Multiple devices may hear the same wake word.

Example:

```text
PC hears "Jarvis"
Phone hears "Jarvis"
Tablet hears "Jarvis"
```

Only one should become active.

Use:

```text
device proximity
+
wake confidence
+
audio quality
+
current interaction state
+
user-selected priority
```

Example priority:

```text
Headset > Phone > PC > Tablet
```

---

# 26. Active Conversation Device

Maintain:

```text
active_voice_device
active_ui_device
```

Example:

```json
{
  "active_voice_device": "phone-01",
  "active_ui_device": "desktop-01"
}
```

This prevents multiple devices from speaking simultaneously.

---

# 27. Notification Routing

Notifications should be intelligent.

Example:

```text
PC:
    task completed

Phone:
    push notification

PC:
    desktop toast

Headset:
    optional TTS
```

User preferences determine which channels are used.

---

# 28. Remote Confirmation

High-risk actions can request confirmation on the nearest active device.

Example:

```text
PC:
    "This will submit the job application.
     Shall I proceed?"

Phone:
    [Approve] [Reject]
```

Approval must be cryptographically bound to:

- task ID;
- action ID;
- exact action parameters;
- expiration time.

A generic "yes" must never approve a different action.

---

# 29. Authentication Requests

If the PC needs a password:

```text
PC:
    auth.required

Phone:
    secure notification

User:
    authenticates using device biometrics

Credential:
    remains in secure storage

PC:
    receives only authorized result
```

Do not synchronize plaintext passwords between devices.

---

# 30. Credential Architecture

Credentials should normally remain on the device that owns them.

Example:

```text
LinkedIn password
        ↓
Windows credential store
```

Android should not receive the password.

If remote authentication is necessary:

```text
Android authorization
       ↓
short-lived capability token
       ↓
PC performs operation
```

---

# 31. Clipboard Synchronization

Clipboard sync must be opt-in.

Support:

- text;
- URLs;
- images;
- files.

Sensitive clipboard data should have:

- expiration;
- encryption;
- no persistent storage by default.

Example:

```text
PC:
    Ctrl+C

JARVIS:
    detects clipboard change

Policy:
    "Sync normal text"

Android:
    clipboard updated
```

---

# 32. File Transfer

Use a resumable transfer protocol.

Flow:

```text
metadata
   ↓
permission check
   ↓
encrypted transfer
   ↓
chunk verification
   ↓
resume support
   ↓
SHA-256 verification
   ↓
atomic rename
```

Large files should not be loaded entirely into memory.

---

# 33. File Transfer Security

Before accepting a remote file:

- verify sender;
- verify transfer authorization;
- verify hash;
- enforce destination policy;
- enforce size limits;
- optionally scan files;
- prevent path traversal.

Never allow:

```text
../../Windows/System32/...
```

or equivalent paths.

---

# 34. LAN Discovery

Preferred initial discovery mechanisms:

### mDNS

Useful for:

```text
jarvis-desktop.local
jarvis-phone.local
```

### UDP discovery

Can be used as a fallback.

Discovery packets must not contain sensitive data.

Example:

```text
JARVIS_DISCOVER
```

Response:

```text
device_id
service_port
protocol_version
public_key_fingerprint
```

---

# 35. Pairing Over LAN

Discovery does not equal trust.

Correct flow:

```text
discover
   ↓
display candidate
   ↓
user initiates pairing
   ↓
authenticated key exchange
   ↓
user verifies code
   ↓
trust relationship stored
```

---

# 36. Remote Connectivity

For access outside the home LAN, do not expose JARVIS ports directly to the Internet.

Preferred architecture:

```text
Device
   ↓
encrypted private overlay
   ↓
other trusted device
```

A private VPN/mesh overlay can be used.

Examples of technologies worth evaluating:

- Tailscale;
- WireGuard;
- Headscale.

The system should abstract the transport so it is not hard-coded to one provider.

---

# 37. Internet Exposure

Never do this:

```text
Internet
   ↓
port 50051
   ↓
JARVIS
```

Instead:

```text
Internet
   ↓
private encrypted overlay
   ↓
authenticated device
   ↓
JARVIS gateway
```

---

# 38. Network State Machine

Each device maintains:

```text
OFFLINE
CONNECTING
AUTHENTICATING
CONNECTED
DEGRADED
RECONNECTING
```

Example:

```text
CONNECTED
    ↓ network loss
DEGRADED
    ↓
RECONNECTING
    ↓ success
CONNECTED
```

---

# 39. Heartbeats

Use heartbeat messages.

Example:

```text
heartbeat interval:
    10–30 seconds

timeout:
    2–3 missed heartbeats
```

Mobile devices should use longer intervals or OS-aware background mechanisms to conserve battery.

---

# 40. Android Background Constraints

Android is fundamentally different from Windows/Linux.

Do not assume a permanent background process can run indefinitely.

Use:

- foreground service where appropriate;
- Android notification system;
- WorkManager for deferred work;
- push mechanisms when required;
- OS-approved background execution.

The Android node should remain lightweight.

---

# 41. PC as Primary AI Host

A practical default configuration:

```text
Android
   ↓ voice/request
Windows PC
   ↓
local LLM
   ↓
agent
   ↓
Windows/browser tools
```

Android becomes the mobile interface and capability endpoint.

---

# 42. Android as Temporary AI Host

If the PC is unavailable:

```text
Android
   ↓
small local model
   ↓
limited agent
```

It should handle:

- basic conversation;
- reminders;
- local notes;
- voice commands;
- simple device operations.

Complex workflows can wait until the PC returns.

---

# 43. Model Routing Across Devices

The coordinator should consider:

```text
model quality
latency
VRAM
battery
network latency
privacy
task complexity
```

Example:

```text
Simple command:
    Android local model

Complex reasoning:
    PC large model

Image analysis:
    PC vision model

No network:
    Android fallback model
```

---

# 44. Device Resource Advertisement

Each node can publish:

```json
{
  "cpu_threads": 16,
  "ram_gb": 32,
  "gpu": "NVIDIA",
  "vram_gb": 8,
  "battery": null,
  "charging": true,
  "models": [
    "qwen-local"
  ]
}
```

Do not expose unnecessary hardware details to untrusted peers.

---

# 45. Task Placement

Example scoring function:

```text
score =
    capability_match
    + model_quality
    + available_resources
    + low_latency
    + privacy_requirement
    - battery_cost
```

The highest-scoring trusted device executes the task.

---

# 46. Synchronization Database

Each node should have a local database.

Recommended:

```text
SQLite
```

Core tables:

```text
devices
device_capabilities
tasks
task_events
conversations
conversation_messages
sync_outbox
sync_inbox
memory_metadata
file_transfers
confirmations
audit_events
```

---

# 47. Event Log

Important state changes should be represented as events.

Example:

```text
TaskCreated
TaskStarted
ToolCalled
ToolCompleted
ConfirmationRequested
ConfirmationApproved
TaskCompleted
```

This creates a reliable audit trail.

---

# 48. Sequence Numbers

Every synchronized stream should have monotonically increasing sequence numbers.

Example:

```text
desktop stream:

1
2
3
4
5
```

If Android receives:

```text
1
2
4
5
```

it knows event 3 is missing and can request replay.

---

# 49. Acknowledgement

Use acknowledgements.

```text
Android:
    event #42

PC:
    ACK #42
```

For critical operations, acknowledgement means receipt, not successful execution.

Use separate execution result messages.

---

# 50. Synchronization Algorithm

Recommended initial approach:

```text
1. Connect
2. Authenticate
3. Exchange device metadata
4. Exchange last-known sequence numbers
5. Request missing events
6. Replay events
7. Resolve conflicts
8. Flush outbound queue
9. Mark synchronized
```

---

# 51. Memory Synchronization

Not every memory should replicate everywhere.

Classify memory:

```text
GLOBAL
PRIVATE_DEVICE
TEMPORARY
SENSITIVE
TASK_CONTEXT
```

Example:

```text
User preference:
    GLOBAL

Browser session:
    PRIVATE_DEVICE

Current screenshot:
    TEMPORARY

Password:
    SENSITIVE
```

Sensitive data should never replicate by default.

---

# 52. Conversation Synchronization Policy

Possible modes:

```text
NONE
METADATA_ONLY
TEXT
TEXT_AND_ATTACHMENTS
FULL
```

Default should be privacy-preserving.

---

# 53. Camera Data

Android camera captures should normally be:

```text
temporary
encrypted
task-scoped
deleted after completion
```

unless the user explicitly saves them.

---

# 54. Presence

Presence information:

```text
device
online
last_seen
active
idle
locked
charging
network
```

Presence helps route notifications and tasks.

Example:

> "Tell me when my PC finishes."

If PC is locked but phone is active:

```text
phone receives notification
```

---

# 55. Cross-Device Locks

Some resources require distributed locks.

Example:

```text
Only one device should control:
    active browser session
```

Use short-lived leases:

```text
browser_control_lease
owner = desktop-01
expires = ...
```

Never use permanent locks.

---

# 56. Browser Session Handoff

A browser workflow can include:

```text
URL
cookies/session reference
tab metadata
workflow state
page snapshot
```

Actual cookies should remain protected on the owning device.

Prefer transferring a task representation rather than raw session credentials.

---

# 57. Example: Job Application Workflow

User:

> "Jarvis, continue the job applications from my phone."

Phone:

```text
send task.handoff
```

PC:

```text
load task
load checkpoint
verify LinkedIn session
continue browser automation
```

If authentication is required:

```text
PC → phone:
    auth.required

Phone:
    "LinkedIn needs you to log in."

User:
    logs in on PC

PC:
    continue task
```

---

# 58. Example: PC-to-Phone Notification

User:

> "Let me know on my phone when the build finishes."

PC:

```text
create task
run build
```

Build completes:

```text
task.completed
```

Notification router:

```text
active phone
    ↓
notification
```

Phone:

> "Your build has completed successfully."

---

# 59. Example: Phone-to-PC Voice

User speaks:

> "Jarvis, open VS Code and run the project."

Android:

```text
wake
VAD
STT
```

Request:

```text
execute desktop action
```

PC:

```text
open VS Code
run project
```

PC returns:

```text
success
```

Android TTS:

> "Done, sir."

---

# 60. Cross-Device Permission Model

Permissions should be capability-based.

Example:

```text
phone:
    can.request desktop.browser

phone:
    cannot directly execute arbitrary shell

phone:
    can request:
        open_application
        screenshot
        run_approved_workflow
```

The PC still makes the final authorization decision.

---

# 61. Remote Tool Calls

Example:

```json
{
  "tool": "desktop.open_application",
  "device": "desktop-01",
  "arguments": {
    "application": "code"
  },
  "authorization": {
    "task_id": "task-123"
  }
}
```

The receiving device validates:

1. sender identity;
2. trust;
3. capability;
4. permission;
5. task authorization;
6. argument schema.

---

# 62. Never Trust Remote Tool Arguments

Remote calls must pass the same validation pipeline as local calls.

```text
remote request
      ↓
schema validation
      ↓
permission check
      ↓
risk classification
      ↓
confirmation if needed
      ↓
tool execution
```

---

# 63. Security Threat Model

Threats include:

- stolen phone;
- compromised PC;
- rogue LAN device;
- MITM attack;
- replay attack;
- malicious plugin;
- compromised VPN;
- leaked pairing code;
- malicious file transfer;
- remote command injection;
- credential theft.

Mitigations must be implemented at the protocol layer.

---

# 64. Replay Protection

Every message should contain:

```text
message_id
timestamp
sequence
nonce
```

Reject:

- expired messages;
- duplicate messages;
- invalid sequence numbers;
- invalid signatures/authentication.

---

# 65. Rate Limiting

Apply rate limits to:

- pairing attempts;
- authentication;
- remote tool requests;
- file transfers;
- task creation;
- notifications.

This prevents a compromised device from flooding the mesh.

---

# 66. Device Revocation

The user must be able to revoke a device immediately.

Example:

```text
Settings
→ Devices
→ Ashutosh Phone
→ Revoke
```

Revocation should invalidate:

- trust relationship;
- access tokens;
- session keys;
- capability leases.

---

# 67. Lost Device

If the phone is lost:

```text
PC
→ Devices
→ Phone
→ Revoke
```

The phone should no longer access:

- tasks;
- memory;
- credentials;
- PC tools.

---

# 68. Audit Log

Record security-sensitive operations:

```text
device paired
device revoked
remote tool executed
confirmation approved
credential access requested
file transferred
permission changed
```

Do not log passwords or raw secrets.

---

# 69. Privacy Rules

Default:

```text
No cloud
No telemetry
No unnecessary replication
No plaintext credentials
No unrestricted remote shell
No persistent screenshots
```

The user should be able to inspect synchronization settings.

---

# 70. Suggested Technology Stack

## Shared protocol

```text
Protocol Buffers
gRPC
WebSocket
TLS 1.3
```

## Local database

```text
SQLite
```

## Cryptography

```text
libsodium
or
well-maintained platform crypto APIs
```

## Networking

```text
mDNS
TCP/TLS
optional QUIC later
```

## Remote private networking

```text
WireGuard
Tailscale
or Headscale
```

---

# 71. Shared Rust Networking Core

For a cross-platform system, Rust is a strong choice for the device-mesh layer.

Suggested crate:

```text
jarvis-mesh
```

Responsibilities:

```text
identity
pairing
discovery
transport
authentication
message envelopes
routing
sync
queues
```

Expose bindings to:

```text
Windows
Ubuntu
Android
```

through platform-specific interfaces.

---

# 72. Proposed Monorepo Structure

```text
jarvis/
│
├── apps/
│   ├── windows/
│   ├── ubuntu/
│   └── android/
│
├── core/
│   ├── agent/
│   ├── ai/
│   ├── memory/
│   ├── skills/
│   ├── security/
│   └── workflows/
│
├── mesh/
│   ├── identity/
│   ├── pairing/
│   ├── discovery/
│   ├── transport/
│   ├── protocol/
│   ├── sync/
│   └── routing/
│
├── protocols/
│   ├── device.proto
│   ├── task.proto
│   ├── event.proto
│   ├── memory.proto
│   └── file_transfer.proto
│
├── sdk/
│   ├── rust/
│   ├── typescript/
│   └── kotlin/
│
└── tests/
    ├── integration/
    ├── network/
    ├── security/
    └── sync/
```

---

# 73. Protocol Versioning

Every protocol must have a version.

Example:

```text
jarvis.mesh.v1
```

Use backward-compatible schema evolution.

Never reuse removed field numbers in Protocol Buffers.

---

# 74. API Compatibility

A device should advertise:

```json
{
  "protocol_version": "1.2",
  "supported_features": [
    "task_handoff",
    "file_transfer_v2",
    "voice_streaming"
  ]
}
```

Feature negotiation prevents newer devices from assuming unsupported functionality.

---

# 75. Connection Manager

Each node should contain:

```text
ConnectionManager
```

Responsibilities:

- establish connection;
- reconnect;
- maintain heartbeat;
- select transport;
- authenticate;
- detect degraded state;
- close stale connections.

---

# 76. Router

The mesh router determines:

```text
Which device?
Which capability?
Which transport?
Which authorization?
```

Example:

```text
Task:
    analyze image

Candidates:
    Android — camera only
    Windows — vision model
    Ubuntu — vision model

Router:
    Android captures
    Windows analyzes
```

---

# 77. Workflow Coordinator

The coordinator should not execute every operation itself.

Instead:

```text
Planner
   ↓
Task Graph
   ↓
Capability Router
   ↓
Device
   ↓
Skill
   ↓
Tool
```

This keeps platform-specific execution isolated.

---

# 78. Task Graph Example

```text
CapturePhoto
      ↓
TransferImage
      ↓
AnalyzeImage
      ↓
GenerateResponse
      ↓
SpeakResponse
```

Potential placement:

```text
CapturePhoto → Android
TransferImage → Mesh
AnalyzeImage → Windows
GenerateResponse → Windows
SpeakResponse → Android
```

---

# 79. Failure Recovery

Suppose Android loses connection during upload.

```text
transfer:
    chunk 1
    chunk 2
    chunk 3
    X connection lost
```

On reconnect:

```text
PC:
    resume from chunk 4
```

Do not restart the entire transfer.

---

# 80. Device Restart

After reboot:

```text
start JARVIS
      ↓
load device identity
      ↓
load local database
      ↓
restore pending tasks
      ↓
connect to mesh
      ↓
synchronize
      ↓
resume eligible workflows
```

---

# 81. Startup Ordering

On Windows/Linux:

```text
OS startup
    ↓
JARVIS service
    ↓
identity
    ↓
database
    ↓
security
    ↓
mesh
    ↓
AI engine
    ↓
skills
    ↓
voice
    ↓
UI
```

The Android startup model follows Android lifecycle constraints rather than desktop service semantics.

---

# 82. Graceful Shutdown

Before shutdown:

```text
stop accepting new tasks
        ↓
checkpoint workflows
        ↓
flush event queue
        ↓
close transfers
        ↓
close connections
        ↓
persist state
```

---

# 83. Testing Strategy

## Unit tests

Test:

- message encoding;
- authentication;
- routing;
- state transitions;
- sequence handling.

## Integration tests

Test:

```text
Windows ↔ Ubuntu
Windows ↔ Android
Ubuntu ↔ Android
```

## Failure tests

Simulate:

- packet loss;
- delayed packets;
- duplicate packets;
- network disconnect;
- device restart;
- clock skew;
- corrupted files.

---

# 84. Network Chaos Testing

Automate:

```text
10% packet loss
500 ms latency
random disconnects
bandwidth throttling
duplicate messages
out-of-order delivery
```

JARVIS should continue safely.

---

# 85. Security Testing

Test:

- invalid certificates;
- revoked device;
- replayed message;
- expired token;
- malformed RPC;
- unauthorized tool;
- path traversal;
- oversized transfer;
- forged capability;
- pairing brute force.

---

# 86. Observability

Provide a local diagnostics panel:

```text
Devices
Connections
Latency
Messages
Task sync
Failed messages
Pending transfers
Protocol version
```

Never require uploading logs to a cloud service.

---

# 87. Developer Diagnostics

CLI:

```bash
jarvis devices list
jarvis devices pair
jarvis devices revoke
jarvis mesh status
jarvis mesh ping
jarvis mesh logs
jarvis sync status
jarvis tasks list
```

Example:

```bash
jarvis mesh ping phone-01
```

Output:

```text
Connected
Latency: 18ms
TLS: 1.3
Protocol: 1.2
Trust: privileged
```

---

# 88. Performance Targets

On a normal LAN:

```text
device discovery:
    < 2 seconds

pairing:
    < 10 seconds

RPC:
    < 50 ms typical

event propagation:
    < 100 ms typical

voice streaming:
    near realtime

small task handoff:
    < 1 second
```

Large model inference is excluded from network latency targets.

---

# 89. Battery Targets for Android

The mesh should minimize background activity.

Prefer:

```text
event-driven
rather than
continuous polling
```

Voice mode should activate only when appropriate.

---

# 90. Data Retention

Default:

```text
events:
    configurable retention

screenshots:
    task-scoped

audio:
    transient

transferred files:
    user-controlled

clipboard:
    short-lived

credentials:
    platform secure storage
```

---

# 91. Recommended Initial Implementation

Do not implement every advanced feature simultaneously.

Build this first:

```text
1. Device identity
2. Pairing
3. LAN discovery
4. TLS
5. gRPC
6. WebSocket events
7. Device registry
8. Capability discovery
9. Task synchronization
10. Remote tool calls
11. Offline queue
12. Reconnection
13. Android ↔ PC voice requests
14. File transfer
15. Device revocation
```

Then add:

```text
QUIC
remote VPN
advanced replication
distributed coordination
automatic task placement
```

---

# 92. Minimal End-to-End Architecture

```text
                 ┌─────────────────────┐
                 │   JARVIS Coordinator│
                 │                     │
                 │ Agent / Planner     │
                 │ AI Engine           │
                 │ Memory              │
                 └──────────┬──────────┘
                            │
                    ┌───────┴───────┐
                    │ Device Router │
                    └───────┬───────┘
                            │
                 Secure Device Mesh
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
   ┌────▼─────┐       ┌─────▼────┐       ┌─────▼─────┐
   │ Windows  │       │  Ubuntu  │       │  Android  │
   │          │       │          │       │           │
   │ Browser  │       │ Terminal │       │ Camera    │
   │ Desktop  │       │ Linux    │       │ Mic       │
   │ Apps     │       │ Apps     │       │ Voice     │
   └──────────┘       └──────────┘       └───────────┘
```

---

# 93. Final Architecture Principle

The most important architectural decision is:

> **JARVIS is one logical agent with multiple trusted execution endpoints.**

The LLM/planner should reason about the user's objective.

The mesh should determine where an operation can safely execute.

The platform layer should perform the actual operation.

Conceptually:

```text
USER
 ↓
VOICE / TEXT
 ↓
JARVIS AGENT
 ↓
TASK GRAPH
 ↓
CAPABILITY ROUTER
 ↓
DEVICE MESH
 ↓
PLATFORM SKILL
 ↓
TOOL
 ↓
RESULT
 ↓
AGENT
 ↓
VOICE / UI
```

This separation allows the project to scale from a single Windows PC into a complete Windows + Ubuntu + Android personal-assistant ecosystem without rewriting the core agent.

---

# 94. Implementation Outcome

After implementing this document, JARVIS should be able to:

- recognize trusted devices;
- communicate securely;
- discover capabilities;
- route tasks;
- synchronize conversations;
- synchronize task state;
- perform remote tool calls;
- transfer files;
- transfer selected context;
- recover from network failures;
- resume workflows;
- notify the user on another device;
- request remote confirmation;
- use a PC as the primary AI engine;
- use Android as a mobile voice endpoint;
- operate independently when disconnected;
- synchronize safely after reconnection.

This establishes the distributed foundation required by the remaining JARVIS subsystems.
