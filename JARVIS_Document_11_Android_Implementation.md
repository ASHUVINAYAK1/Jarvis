# JARVIS — Document 11
# Android Implementation: Mobile Companion, Voice Interface, Device Control, Background Execution, Security, and PC Coordination

**Document status:** Detailed implementation specification  
**Target platform:** Android 10+ initially, with Android 13–16 as primary validation targets  
**Primary role:** Mobile companion and remote-control surface for the JARVIS system  
**Design principle:** Android is not merely a smaller desktop client. It is a first-class JARVIS node with its own voice interface, notifications, sensors, local capabilities, device controls, and secure connection to the primary PC-hosted JARVIS brain.

---

# 1. Purpose

The Android implementation provides JARVIS capabilities when the user is away from the PC.

It should support:

- voice activation,
- push-to-talk,
- spoken responses,
- text chat,
- notifications,
- task monitoring,
- PC control,
- local phone actions,
- application launching where Android permits it,
- media control,
- contacts and communication workflows with explicit permissions,
- camera input,
- screenshot/screen understanding where permitted,
- location-aware workflows where explicitly enabled,
- file access,
- document capture,
- device information,
- secure pairing with Windows/Linux JARVIS nodes,
- remote confirmations,
- task cancellation,
- local AI fallback,
- offline voice interaction,
- synchronization of memory and task state.

The Android app should feel like the same JARVIS identity rather than a separate assistant.

---

# 2. Important Android Constraint

Android is substantially more sandboxed than desktop operating systems.

A normal Android application cannot arbitrarily:

- inspect every other application's UI,
- inject arbitrary input into every app,
- read another app's private files,
- silently perform every system action,
- run unrestricted background processes indefinitely,
- access microphone/camera/location without permission,
- automate another application without an appropriate Android API/service.

Therefore the architecture must distinguish between:

```text
Android-native capabilities
```

and:

```text
PC execution capabilities
```

When the user asks for an operation that Android cannot safely or legally perform directly, JARVIS should route the task to the paired PC.

---

# 3. Recommended Android Stack

| Layer | Technology |
|---|---|
| Language | Kotlin |
| UI | Jetpack Compose |
| Architecture | Clean Architecture + MVVM/MVI |
| Dependency Injection | Hilt |
| Async | Kotlin Coroutines |
| State | StateFlow |
| Local DB | Room |
| Preferences | DataStore |
| Networking | Kotlin/OkHttp |
| Serialization | Kotlinx Serialization |
| RPC | gRPC where appropriate; HTTPS/WebSocket for selected flows |
| Secure storage | Android Keystore |
| Credentials | Credential Manager where applicable |
| Voice STT | whisper.cpp native integration / Android speech fallback |
| Wake word | openWakeWord or native wake-word engine |
| VAD | Silero VAD |
| TTS | Piper native integration / Android TTS fallback |
| Audio | Android AudioRecord/AudioTrack |
| Camera | CameraX |
| Notifications | NotificationManager |
| Background work | WorkManager |
| Long-running voice service | Foreground Service where permitted |
| App automation | AccessibilityService only when explicitly enabled |
| PC discovery | mDNS/NSD + explicit pairing |
| Secure transport | TLS |
| Cryptographic identity | Android Keystore |
| Local AI | llama.cpp/ExecuTorch/ONNX depending on model |
| Build | Gradle |
| Testing | JUnit + Compose UI + instrumented tests |

---

# 4. Android Application Modules

Recommended:

```text
android/
├── app/
├── core/
│   ├── model/
│   ├── networking/
│   ├── security/
│   ├── database/
│   ├── audio/
│   └── common/
├── feature/
│   ├── assistant/
│   ├── devices/
│   ├── tasks/
│   ├── memory/
│   ├── settings/
│   ├── permissions/
│   └── diagnostics/
└── native/
    ├── whisper/
    ├── piper/
    ├── vad/
    └── wakeword/
```

Keep platform-specific code isolated.

---

# 5. Android as a JARVIS Node

The overall architecture should be:

```text
                 JARVIS NETWORK
                       │
          ┌────────────┼────────────┐
          │            │            │
          ▼            ▼            ▼
       Windows       Ubuntu       Android
       Node           Node          Node
          │            │            │
          └────────────┼────────────┘
                       │
                 Shared Identity
                 Shared Task Model
                 Shared Memory
                 Shared Policies
```

The user should be able to start a task on one device and inspect it from another.

---

# 6. Primary vs Secondary AI

The PC should generally be the primary inference host.

Example:

```text
Android
   │
   ├── lightweight voice processing
   ├── local small model
   └── UI
          │
          ▼
      PC JARVIS
          │
     large local LLM
     VLM
     browser
     desktop automation
```

This provides significantly more capability than attempting to run the largest model entirely on a phone.

---

# 7. Local-Only Philosophy

The Android application should support a strict:

```text
LOCAL ONLY
```

mode.

In this mode:

- no cloud LLM,
- no cloud STT,
- no cloud TTS,
- no analytics,
- no remote telemetry.

PC communication can remain local:

```text
Android
   ↓
LAN
   ↓
PC
```

Internet access is used only for explicit tasks such as web browsing.

---

# 8. Android Assistant Screen

Main screen:

```text
┌───────────────────────────┐
│ JARVIS                    │
│ ● Ready                   │
│                           │
│       ◉                   │
│   Listening / Idle        │
│                           │
│ "How can I help?"         │
│                           │
│ [ Hold to speak ]         │
│                           │
│ Recent tasks              │
│ • Job search              │
│ • Music                   │
│ • PC diagnostics          │
└───────────────────────────┘
```

The UI should remain minimal.

Voice should be the primary interaction.

---

# 9. Voice Activation Modes

Android should support multiple modes.

### Mode A — Wake word

```text
"Jarvis"
```

### Mode B — Push to talk

User holds a button.

### Mode C — Headset button

When supported.

### Mode D — Notification action

Tap "Ask JARVIS."

### Mode E — Lock-screen interaction

Only within Android's allowed security boundaries.

---

# 10. Wake Word on Android

Wake-word processing should ideally remain local.

Pipeline:

```text
Microphone
 ↓
audio preprocessing
 ↓
wake word
 ↓
VAD
 ↓
record
 ↓
STT
```

Do not continuously send microphone audio to the PC merely to detect the wake word.

---

# 11. Android Background Restrictions

Android aggressively limits background execution.

Do not design the assistant around a permanently unrestricted background process.

Use:

- foreground service for approved long-running voice functionality,
- WorkManager for deferred work,
- notification actions,
- system-supported background APIs,
- exact scheduling only where justified and permitted.

The application should remain functional if Android stops a background component.

---

# 12. Foreground Service

A foreground service may be used for active assistant functions such as:

- ongoing voice session,
- active navigation,
- active task execution requiring continuous presence,
- selected device-control operations.

The user must see a persistent notification when required by Android.

The service must declare the appropriate foreground-service type(s) for the Android versions targeted.

---

# 13. Battery Strategy

Idle JARVIS should consume minimal battery.

Avoid:

```text
continuous heavy LLM
continuous VLM
continuous camera
continuous GPS
```

Instead:

```text
wake word
 ↓
short active window
 ↓
process
 ↓
return idle
```

Large model inference should normally be delegated to the PC.

---

# 14. Android Audio Pipeline

Recommended:

```text
AudioRecord
   ↓
noise suppression
   ↓
wake word
   ↓
VAD
   ↓
whisper.cpp
   ↓
JARVIS Core
   ↓
Piper / Android TTS
   ↓
AudioTrack
```

Audio should be streamed rather than buffering unnecessarily large recordings.

---

# 15. Android STT Options

Primary:

**whisper.cpp**

Advantages:

- local,
- predictable,
- cross-platform,
- compatible with the JARVIS speech architecture.

Fallback:

Android system speech recognition where the user explicitly allows it.

The app should clearly indicate when processing is not local.

---

# 16. Android TTS

Primary:

**Piper**, when an appropriate Android-native build is practical.

Fallback:

Android `TextToSpeech`.

Recommended behavior:

```text
Piper available
 → use Piper

otherwise
 → Android TTS
```

The user can choose the voice.

---

# 17. Streaming TTS

JARVIS should not wait for the complete answer.

Example:

```text
LLM generates:
"Your application..."

 ↓ sentence boundary

TTS starts

 ↓

LLM continues:
"has been submitted..."
```

The Android UI should show the current sentence while audio plays.

---

# 18. Barge-In

User:

> "Jarvis, stop."

While JARVIS speaks:

```text
audio input
 ↓
VAD
 ↓
speech detected
 ↓
stop TTS
 ↓
cancel playback
 ↓
process new command
```

The interruption path must have priority over normal assistant generation.

---

# 19. Bluetooth Headsets

Support:

- Bluetooth microphones,
- earbuds,
- car audio,
- headset buttons where available.

Handle:

```text
connected
disconnected
switched
```

Audio routing must recover gracefully.

---

# 20. Android Permissions

Permissions should be requested just-in-time.

Potential permissions:

```text
RECORD_AUDIO
CAMERA
ACCESS_FINE_LOCATION
ACCESS_COARSE_LOCATION
POST_NOTIFICATIONS
READ_MEDIA_IMAGES
READ_MEDIA_VIDEO
BLUETOOTH_CONNECT
BLUETOOTH_SCAN
```

Only request capabilities the user actually enables.

Do not request every permission on first launch.

---

# 21. Permission UX

Bad:

> "JARVIS needs 12 permissions."

Good:

> "To control your PC from your phone, JARVIS needs notification access."

Then:

```text
why
what it enables
what it does not enable
```

The user should be able to revoke access later.

---

# 22. Android AccessibilityService

An AccessibilityService can enable advanced UI automation, but it is a sensitive capability.

If enabled by the user, it may support:

- inspecting accessible UI,
- interacting with supported app controls,
- reading UI content within permitted boundaries,
- performing accessibility actions.

It must not be treated as a universal bypass of Android security.

---

# 23. Accessibility Automation Policy

The assistant should clearly tell the user:

> "Accessibility access allows JARVIS to interact with supported applications."

The app should provide a dedicated permission screen explaining:

- why it is required,
- what information can be accessed,
- how to disable it.

---

# 24. Android App Automation

For a supported application:

```text
AccessibilityService
 ↓
inspect tree
 ↓
find element
 ↓
perform action
 ↓
verify
```

Example:

```text
Find "Send"
 ↓
click
 ↓
verify message state
```

Do not assume every application exposes useful accessibility metadata.

---

# 25. Android Computer Use

When semantic UI information is unavailable:

```text
screenshot
 ↓
OCR
 ↓
VLM if required
 ↓
target localization
 ↓
supported action
 ↓
verify
```

The exact ability to capture/interact with another app depends on Android APIs, app permissions, device state, and platform restrictions.

---

# 26. Android Screen Capture

Use Android's supported screen-capture mechanisms, such as MediaProjection, when the user explicitly grants access.

The permission should be session-aware where appropriate.

Do not attempt hidden screen recording.

---

# 27. Screenshot Privacy

Screenshots can contain:

- passwords,
- OTPs,
- bank details,
- messages,
- private photos.

Therefore:

```text
capture
 ↓
process locally
 ↓
discard
```

unless the user explicitly asks to save or share the screenshot.

---

# 28. Camera

CameraX should be used.

Potential JARVIS commands:

> "Take a picture."

> "Read this document."

> "What is this component?"

Pipeline:

```text
CameraX
 ↓
frame
 ↓
OCR / VLM
 ↓
response
```

For continuous vision, enforce explicit activation.

---

# 29. Document Scanning

A useful Android skill:

```text
scan document
 ↓
detect edges
 ↓
correct perspective
 ↓
OCR
 ↓
save PDF/image
```

This can feed the local RAG system.

---

# 30. QR Codes

JARVIS can use the camera to:

- scan QR codes,
- inspect links,
- extract text.

Before opening a suspicious URL, the assistant should identify the domain and request confirmation if policy requires.

---

# 31. Location

Location should be opt-in.

Potential commands:

> "How far am I from home?"

> "Remind me when I reach the office."

The location subsystem should expose:

```text
location.current
location.geofence
location.permission_status
```

Location data should have explicit retention policies.

---

# 32. Contacts

If enabled, JARVIS may search contacts.

Example:

> "Call Rahul."

Flow:

```text
contacts.search
 ↓
resolve contact
 ↓
confirmation if ambiguous
 ↓
dial
```

Do not send messages or initiate calls merely because an LLM inferred a likely contact.

---

# 33. Phone Calls

Use Android-supported call APIs.

Possible policy:

```text
dial number
```

may be allowed.

But:

```text
place call
```

can have an external side effect and should follow confirmation policy.

---

# 34. Messaging

JARVIS should distinguish:

```text
draft message
```

from:

```text
send message
```

Example:

> "Write a message to Rahul saying I'll be late."

JARVIS:

> "Draft ready. Do you want me to send it?"

For trusted workflows the user can configure automatic sending, but the default should be conservative.

---

# 35. Notifications

Android is ideal as a JARVIS notification surface.

Examples:

```text
Task completed
Confirmation required
PC offline
Job application needs information
Download finished
Reminder
Security alert
```

Notification actions:

```text
Approve
Reject
Open
Cancel
Snooze
```

---

# 36. Remote Confirmation

This is a major feature.

Suppose the Linux PC asks:

> "Submit the job application?"

Android notification:

```text
JARVIS

Application for Software Engineer
at Example Corp

[Review] [Approve] [Reject]
```

Approval must be authenticated and bound to the task.

---

# 37. Confirmation Security

Do not send:

```text
approve=true
```

as an unauthenticated network message.

Use:

```text
task_id
confirmation_id
device_identity
nonce
timestamp
signature/authenticated channel
```

The PC verifies that the response belongs to the current task.

---

# 38. Device Pairing

Recommended first-time flow:

```text
PC:
"Add Android device"

       ↓

QR code displayed

       ↓

Android scans QR

       ↓

cryptographic handshake

       ↓

user confirms pairing

       ↓

device registered
```

Do not use only an IP address as identity.

---

# 39. QR Pairing Payload

The QR code can contain a short-lived bootstrap token:

```json
{
  "device": "linux-pc",
  "endpoint": "...",
  "pairing_token": "...",
  "expires": "..."
}
```

It must not contain permanent credentials.

---

# 40. Device Identity

Generate an Android device key using Android Keystore.

Conceptually:

```text
Android
 ├── private key
 └── public identity

PC
 └── trusted device record
```

The private key should remain non-exportable where the Android Keystore implementation permits.

---

# 41. Transport

Potential transport layers:

### Local LAN

```text
TLS + WebSocket/gRPC
```

### Remote access

Prefer a secure overlay/network architecture rather than exposing a raw JARVIS port to the internet.

Possible future integrations can include secure VPN/mesh networking.

Do not expose unauthenticated ports.

---

# 42. LAN Discovery

Use Android Network Service Discovery (NSD)/mDNS.

The PC advertises:

```text
_jarvis._tcp
```

Android discovers it.

Discovery does not equal trust.

Pairing is still required.

---

# 43. Connection States

Android should show:

```text
Connected
Connecting
PC unavailable
Paired but offline
Remote connection
```

The user should always know whether commands are being executed:

```text
On phone
On PC
On another device
```

---

# 44. Task Routing

Every command is classified.

Example:

> "Play music."

Could execute locally.

> "Open VS Code."

Must route to PC.

> "Search LinkedIn for SDE jobs."

Can route to PC browser.

> "Take a photo."

Execute on phone.

> "What is on my screen?"

Could execute on phone or PC depending on active device.

---

# 45. Capability Advertisement

Each device publishes capabilities:

```json
{
  "device": "android",
  "capabilities": [
    "camera",
    "microphone",
    "tts",
    "location",
    "notifications",
    "contacts"
  ]
}
```

PC:

```json
{
  "device": "linux-pc",
  "capabilities": [
    "browser",
    "desktop",
    "terminal",
    "large_llm",
    "vlm",
    "filesystem"
  ]
}
```

The planner chooses an appropriate node.

---

# 46. Shared Task Model

A task created on Android should appear on the PC.

Example:

```json
{
  "task_id": "task_123",
  "origin_device": "android",
  "execution_device": "linux-pc",
  "state": "WAITING_CONFIRMATION"
}
```

---

# 47. Task Cancellation

User:

> "Cancel that job application."

Android sends:

```text
task.cancel(task_123)
```

PC:

```text
cancel pending actions
stop browser worker if safe
save state
```

External operations already completed cannot necessarily be undone.

---

# 48. Task Progress

Android should show:

```text
Searching jobs...
3/20 jobs evaluated
Application form open
Waiting for your confirmation
```

Progress should be event-based, not simulated by the UI.

---

# 49. Offline Android Mode

When PC is unavailable:

Android should still support:

- wake word where permitted,
- local STT,
- local TTS,
- small local LLM,
- notes,
- reminders,
- local files,
- camera,
- basic device controls,
- cached memory,
- local task creation.

The app should queue tasks that require the PC.

---

# 50. Offline Task Queue

Example:

```text
User:
"Jarvis, when my PC comes online, open VS Code and run the tests."

Android:
task created
 ↓
PC unavailable
 ↓
queue
 ↓
PC reconnects
 ↓
send task
 ↓
execute
```

The user can cancel queued tasks.

---

# 51. Local Android Model

Do not assume a large model can run efficiently on every phone.

Use hardware profiles:

```text
Low
Medium
High
```

The model manager chooses an appropriate quantized model.

Possible local runtimes:

- llama.cpp,
- ExecuTorch,
- ONNX Runtime,
- vendor-specific acceleration where justified.

The final runtime should be benchmark-driven.

---

# 52. Android Model Manager

Capabilities:

```text
download
install
remove
benchmark
select
```

Model metadata:

```text
RAM required
storage required
context length
quantization
runtime
CPU/GPU/NPU support
```

---

# 53. Model Downloading

Models may be hundreds of MB or multiple GB.

Requirements:

- resumable download,
- checksum verification,
- free-space check,
- Wi-Fi preference,
- charging preference,
- background restrictions handling.

Never leave a partially downloaded model marked as usable.

---

# 54. Storage Strategy

Use app-specific storage.

Do not scatter models across arbitrary shared directories.

Recommended:

```text
Android app storage/
└── models/
```

The user should be able to see model storage usage.

---

# 55. Model Cache Policy

When storage is low:

```text
unused model
 ↓
candidate for removal
```

Never automatically delete the user's preferred model without warning.

Temporary VLM/model caches should have quotas.

---

# 56. Android Database

Use Room for:

- tasks,
- device records,
- cached messages,
- model metadata,
- preferences that require structured storage,
- local memory indexes where appropriate.

Example:

```text
TaskEntity
DeviceEntity
ConversationEntity
ModelEntity
PermissionEntity
```

---

# 57. DataStore

Use DataStore for preferences such as:

```text
wake word enabled
voice selected
TTS speed
default execution device
confirmation mode
battery mode
```

Do not store secrets in DataStore.

---

# 58. Android Keystore

Use Keystore for:

- device identity keys,
- encryption keys,
- secure local key material.

Do not store private keys in plain files.

---

# 59. Local Database Encryption

If JARVIS stores highly sensitive data locally, use an appropriate encryption strategy.

At minimum:

```text
encrypted sensitive fields
+
Keystore-protected key
```

The architecture should allow stronger encrypted database support later.

---

# 60. Memory Synchronization

Memory can exist:

```text
Android local memory
PC primary memory
```

Use explicit categories:

```text
device-local
user-global
task-local
temporary
```

Do not sync every piece of raw conversation blindly.

---

# 61. Privacy-Sensitive Memory

Examples:

- credentials,
- financial data,
- private documents,
- precise location,
- private communications.

These should have stricter storage/synchronization policies.

The model should receive only what it needs for the current task.

---

# 62. Android File Access

Use the Storage Access Framework.

Support:

```text
open document
create document
open directory
```

Do not rely on broad filesystem access.

---

# 63. File Sharing with PC

User:

> "Send this PDF to my PC."

Flow:

```text
Android file picker
 ↓
select PDF
 ↓
encrypted transfer
 ↓
PC receives
 ↓
verify checksum
 ↓
save to selected location
```

The user should choose the destination unless a trusted rule exists.

---

# 64. Clipboard

Android clipboard access is restricted and privacy-sensitive on modern Android.

JARVIS should use clipboard APIs only when appropriate.

Avoid continuous clipboard monitoring.

---

# 65. Media Control

Android can control local media where supported.

Potential commands:

```text
play
pause
next
previous
volume
```

For remote PC media:

```text
Android
 ↓
JARVIS protocol
 ↓
PC MPRIS/media adapter
```

---

# 66. Smart Home Future Integration

Do not hard-code smart-home vendors into the Android app.

Instead:

```text
JARVIS Skill
 ↓
Home integration adapter
```

Future protocols can include:

- Matter,
- Home Assistant,
- vendor APIs.

The Android app simply provides the interaction surface.

---

# 67. Wearables

Future support:

```text
Wear OS
smartwatch
Bluetooth headset
car systems
```

The Android architecture should expose assistant APIs that these clients can consume later.

---

# 68. Android Widget

A home-screen widget can provide:

```text
Ask JARVIS
Run recent task
PC status
Microphone
```

Avoid making the widget the only assistant interface.

---

# 69. Quick Settings Tile

A JARVIS Quick Settings tile can provide:

```text
Ask JARVIS
```

or:

```text
JARVIS listening
```

where supported.

This is particularly useful when the user does not want to open the application.

---

# 70. Lock Screen

The assistant should be conservative on the lock screen.

Allowed examples:

```text
Start a voice session
Read non-sensitive task status
```

Restricted:

```text
send message
unlock PC
approve financial action
expose private memory
```

The device's authentication state should influence policy.

---

# 71. Biometric Confirmation

For high-impact remote actions:

```text
Android confirmation
 ↓
biometric authentication
 ↓
signed approval
 ↓
PC execution
```

This can provide a strong human-presence signal.

---

# 72. Android Security Model

The Android app should enforce:

```text
least privilege
```

Every capability has:

```text
requested
granted
denied
temporarily unavailable
```

The core planner should not assume a capability exists.

---

# 73. Dangerous Action Policy

Examples requiring strong confirmation:

```text
send money
delete large sets of files
send sensitive message
submit legal/financial form
approve privileged PC command
share private document
```

Android can be the confirmation device even when the operation runs on the PC.

---

# 74. Remote PC Password Request

If PC automation reaches a login prompt:

```text
PC:
"Password required."
```

JARVIS can speak:

> "Your password is required to continue."

Do not ask the user to speak a password aloud in a public environment.

Preferred:

```text
open secure password input
```

or:

```text
use credential manager
```

---

# 75. OTP Handling

If a login requires an OTP:

```text
JARVIS:
"An OTP is required. Please enter it."
```

The system should not automatically infer or expose OTPs unless the user has explicitly configured a secure workflow and the source is permitted.

---

# 76. Browser on Android

Android can use:

- Chrome,
- Firefox,
- custom tabs,
- embedded browser only where appropriate.

For serious PC web automation, route to the PC browser worker.

The Android app is primarily the command/control surface.

---

# 77. Android Job Application Example

User:

> "Jarvis, apply for SDE jobs."

Android:

```text
wake word
 ↓
STT
 ↓
classify task
 ↓
PC required
 ↓
send to paired PC
```

PC executes:

```text
browser
 ↓
LinkedIn
 ↓
job search
 ↓
application
```

Android receives:

```text
3 suitable jobs found
1 application ready for confirmation
```

User approves via voice or biometric UI.

---

# 78. Phone-as-Remote Example

User:

> "Jarvis, what's happening on my PC?"

Android:

```text
task/status request
 ↓
PC
 ↓
desktop/task state
 ↓
Android
```

JARVIS responds:

> "VS Code is open. Your test suite is running and 38 of 42 tests have completed."

---

# 79. PC Wake/Availability

The Android app can display:

```text
PC online
PC asleep
PC offline
```

Future versions may support a configured Wake-on-LAN workflow.

Wake-on-LAN should require explicit user setup.

---

# 80. Cross-Device Routing Algorithm

Conceptually:

```text
1. Parse user intent.
2. Identify required capabilities.
3. Enumerate trusted devices.
4. Check device availability.
5. Check permissions.
6. Choose execution device.
7. Create task.
8. Execute.
9. Stream progress.
10. Request confirmation if required.
11. Verify result.
12. Synchronize task state.
```

---

# 81. Android UI State

The UI should expose the task state:

```text
Listening
Thinking
Executing on PC
Waiting for you
Speaking
Completed
Failed
Offline
```

Do not show "thinking" forever.

If the PC is working, say:

> "Working on your PC."

---

# 82. Conversation Synchronization

Conversation history can be synchronized selectively.

Recommended:

```text
conversation metadata
task context
assistant result
```

rather than automatically syncing every audio recording.

Raw audio should be discarded after transcription unless the user explicitly enables storage.

---

# 83. Voice Recording Retention

Default:

```text
audio
 ↓
STT
 ↓
discard
```

Optional:

```text
save recording
```

only when explicitly enabled.

---

# 84. Android Diagnostics

Provide:

```text
JARVIS Diagnostics
```

Checks:

```text
microphone
speaker
wake word
VAD
STT
TTS
local model
PC connection
notifications
camera
accessibility
battery optimization
storage
network
Keystore
```

---

# 85. Battery Optimization UX

If reliable background voice requires an exemption or special system configuration on a particular Android version/device, explain the tradeoff.

Do not tell users to disable all battery protections without reason.

Provide:

```text
Recommended
Balanced
Battery Saver
```

profiles.

---

# 86. Manufacturer Differences

Android vendors may impose additional background restrictions.

Test major classes of devices:

- Pixel
- Samsung
- OnePlus
- Xiaomi
- Motorola

Do not assume identical behavior across manufacturers.

---

# 87. Android Crash Recovery

If the assistant service crashes:

```text
system
 ↓
restart when allowed
 ↓
restore state
 ↓
reconnect PC
```

Never repeatedly crash-loop.

Use exponential backoff.

---

# 88. Network Recovery

When Wi-Fi changes:

```text
connection lost
 ↓
discover PC again
 ↓
authenticate existing pairing
 ↓
resume
```

Do not require manual pairing every time the local IP changes.

---

# 89. Remote Security

For a future internet-accessible JARVIS network:

Never expose:

```text
raw HTTP assistant API
```

without strong authentication.

Use:

```text
mutual authentication
encrypted transport
device identity
authorization
rate limiting
task-level confirmation
```

---

# 90. Android App Architecture

Recommended dependency flow:

```text
UI
 ↓
ViewModel
 ↓
Use Case
 ↓
Repository
 ↓
Service / local DB / network
```

Do not let Compose UI directly call networking or native inference code.

---

# 91. Feature Modules

Example:

```text
feature-assistant
feature-devices
feature-tasks
feature-memory
feature-camera
feature-settings
feature-diagnostics
```

This keeps the project maintainable as functionality grows.

---

# 92. Native AI Integration

Native libraries such as whisper.cpp/Piper should be isolated behind Kotlin interfaces.

Example:

```text
SpeechRecognizer
    └── WhisperCppRecognizer

SpeechSynthesizer
    └── PiperSynthesizer
```

The rest of the app does not know the native implementation.

---

# 93. JNI Boundary

Keep JNI calls small and stable.

Do not expose raw C++/C structures throughout Kotlin.

Preferred:

```text
Kotlin
 ↓
Native wrapper
 ↓
C/C++/Rust
```

with a small API:

```text
initialize()
loadModel()
transcribe()
unload()
```

---

# 94. Android Performance

Targets should be measured by device class.

Example targets:

| Operation | Target |
|---|---:|
| Wake detection | near real-time |
| VAD response | <200 ms |
| short STT | ~1–3 s depending on model/device |
| TTS start | <1 s |
| PC command dispatch | <500 ms on LAN |
| notification delivery | platform dependent |
| app cold start | <2–3 s target |

These are engineering targets, not guarantees.

---

# 95. Thermal Management

Large local models can heat phones quickly.

Use:

```text
temperature/load detection
 ↓
reduce model
 ↓
reduce generation length
 ↓
route inference to PC
```

Avoid prolonged high-load local VLM operation on battery-powered devices.

---

# 96. Charging Mode

When charging and on Wi-Fi, the app may permit:

- model downloads,
- indexing,
- local embeddings,
- maintenance,
- cache cleanup.

The user should control these policies.

---

# 97. Android Model Routing

Example:

```text
Short command
 → phone model

Complex reasoning
 → PC model

Image understanding
 → PC VLM if available

Offline emergency command
 → phone small model
```

The router should consider:

```text
task complexity
device availability
battery
thermal state
privacy
latency
model availability
```

---

# 98. Local Fallback

If PC goes offline during a conversation:

```text
PC unavailable
 ↓
Android local model
 ↓
continue basic conversation
```

For unsupported tasks:

> "Your PC is offline, so I can't perform that desktop operation yet."

---

# 99. Android Logging

Use structured logs.

Never log:

- passwords,
- OTPs,
- tokens,
- raw private documents,
- unrestricted conversation content.

Production logging should be less verbose than development logging.

---

# 100. Crash Reports

If crash reporting is added, it should be:

- opt-in where appropriate,
- privacy-preserving,
- free of secrets,
- disabled in strict local-only mode.

---

# 101. Testing Strategy

### Unit tests

- task routing,
- permission state,
- device selection,
- policy,
- serialization,
- repositories.

### Instrumented tests

- audio,
- database,
- services,
- networking.

### Compose tests

- assistant UI,
- confirmation UI,
- device management.

### End-to-end

```text
Android
 ↓
LAN
 ↓
Ubuntu
 ↓
task
 ↓
Android notification
 ↓
confirmation
 ↓
Ubuntu completion
```

---

# 102. Android Automation Testing

Create controlled test applications exposing known accessibility trees.

Example test app:

```text
Button: Login
TextField: Email
TextField: Password
Button: Submit
```

JARVIS should automate this deterministic environment before attempting real applications.

---

# 103. Voice Testing

Test:

- accents,
- Hindi,
- English,
- code-switching,
- background noise,
- fan noise,
- music,
- Bluetooth microphones,
- whispered commands,
- long commands,
- interruptions.

Measure:

```text
wake false positives
wake false negatives
WER
latency
barge-in latency
```

---

# 104. Security Testing

Test:

- malicious webpage instructions,
- malicious terminal output,
- fake device pairing,
- replayed confirmation,
- expired confirmation,
- stolen local network access,
- unauthorized device,
- notification spoofing,
- accessibility misuse,
- screenshot leakage.

---

# 105. Android Build Variants

Recommended:

```text
debug
internal
beta
release
```

Potential release flavors:

```text
localOnly
standard
developer
```

The `localOnly` build should disable cloud integrations entirely.

---

# 106. Play Store Considerations

If distributing through Google Play, review current platform policies concerning:

- AccessibilityService use,
- foreground services,
- background microphone,
- screen capture,
- SMS/call-related functionality,
- package visibility,
- permissions.

Do not design the product assuming that every technically possible API use is automatically permitted for Play distribution.

A sideloaded/private enterprise build can have a different deployment model, but should still respect Android security boundaries.

---

# 107. Sideloaded Developer Build

For development:

```text
adb install
```

Use this to test capabilities before production distribution.

Do not bypass Android security through rooting or undocumented exploits.

---

# 108. Android Startup

Unlike desktop startup, do not expect a conventional "launch everything at boot" model.

Use Android-supported mechanisms such as:

- boot-related receivers where permitted,
- scheduled work,
- notification actions,
- foreground services where justified.

The assistant must remain useful even if Android delays background execution.

---

# 109. Android Boot Flow

Conceptually:

```text
device boot
 ↓
Android
 ↓
JARVIS state restoration when permitted
 ↓
device discovery
 ↓
PC connection attempt
 ↓
notification/service as appropriate
```

Do not automatically start heavy AI models at boot.

---

# 110. Deep Link Architecture

Support links such as:

```text
jarvis://task/<id>
jarvis://device/<id>
jarvis://confirm/<id>
jarvis://assistant
```

This allows notifications and other applications to open specific JARVIS screens.

Validate all deep-link parameters.

---

# 111. Android Assistant Integration

Where Android's current APIs permit, investigate integration with:

- default digital assistant mechanisms,
- assistant intents,
- voice interaction APIs.

Do not make the entire product dependent on a specific Android assistant API.

---

# 112. Home Screen Integration

Provide:

- widget,
- shortcut,
- quick settings tile,
- notification action.

The fastest path to JARVIS should be:

```text
wake word
or
single tap
```

---

# 113. Security Architecture

The Android node should have:

```text
Device Identity
      ↓
Secure Transport
      ↓
Authentication
      ↓
Authorization
      ↓
Task Policy
      ↓
Execution
```

Never:

```text
socket
 ↓
command
 ↓
execute
```

---

# 114. Remote Command Authorization

Each remote command should include:

```text
device_id
task_id
request_id
capability
arguments
timestamp
```

The receiving node validates:

```text
device trusted?
capability allowed?
request fresh?
task valid?
policy allows?
confirmation required?
```

---

# 115. Replay Protection

Confirmation messages should expire.

Example:

```text
confirmation expires in 60 seconds
```

or use one-time nonce-based authorization.

An old "Approve" message must never approve a new task.

---

# 116. Android Lock State

When the phone is locked:

```text
low-risk voice responses
```

may be allowed.

High-risk actions should require:

```text
device unlock
or biometric authentication
```

depending on policy.

---

# 117. User Profiles

The Android app can support one JARVIS identity with multiple paired devices.

Future support:

```text
personal device
work device
tablet
car
watch
```

Each has separate capabilities.

---

# 118. Multi-Device Conflict Resolution

If Android and PC both modify a task:

Use task event IDs and timestamps.

Example:

```text
task event
event_id
device_id
sequence
timestamp
```

The core task engine resolves conflicts according to explicit state transitions.

---

# 119. Android-Specific Skills

Initial skills:

```text
phone
camera
contacts
calls
notifications
files
media
location
clipboard
PC remote control
device status
```

Future:

```text
calendar
email
smart home
car
wearable
```

Each skill follows:

```text
detect
validate permissions
execute
verify
report
```

---

# 120. Example: "Take a Picture"

User:

> "Jarvis, take a picture."

Flow:

```text
wake
 ↓
STT
 ↓
intent
 ↓
camera permission
 ↓
CameraX
 ↓
capture
 ↓
save
 ↓
speak confirmation
```

No PC required.

---

# 121. Example: "Show Me What's on My PC"

```text
Android
 ↓
task request
 ↓
PC screenshot capability
 ↓
screen capture
 ↓
OCR/VLM
 ↓
structured result
 ↓
Android
```

If the user explicitly asks to see the image, transfer the image through the authenticated channel.

---

# 122. Example: "Run Tests on My PC"

```text
Android voice
 ↓
PC selection
 ↓
task creation
 ↓
policy
 ↓
PC terminal
 ↓
execution
 ↓
progress events
 ↓
result
 ↓
Android TTS
```

---

# 123. Example: "Stop JARVIS on My PC"

This should be treated as a privileged device-control command.

Possible policy:

```text
Android authenticated
+
biometric confirmation
```

Then:

```text
PC service shutdown
```

The user should be informed that remote assistant services are stopping.

---

# 124. Android Dashboard

Recommended sections:

```text
Assistant
Devices
Tasks
Memory
Models
Permissions
Automation
Settings
Diagnostics
```

The device page should show:

```text
This phone
Ubuntu PC
Windows PC
```

with status.

---

# 125. Device Detail

Example:

```text
Ubuntu PC
● Online

Capabilities:
✓ Browser
✓ Desktop
✓ Terminal
✓ Local LLM
✓ Vision
✓ Voice

Actions:
[Ask]
[Diagnostics]
[Disconnect]
[Manage permissions]
```

---

# 126. User Experience Principle

JARVIS should speak naturally.

Instead of:

> "RPC call task.submit returned HTTP 200."

Say:

> "Done, sir."

Technical details belong in diagnostics.

---

# 127. Error UX

Instead of:

> "UNAVAILABLE: gRPC status 14."

Say:

> "Your PC is currently offline."

Then optionally:

> "I can queue the task and run it when the PC reconnects."

---

# 128. Android Notification Example

```text
JARVIS

Your PC needs approval.

"Submit Software Engineer application
to Example Corp?"

[Review] [Approve] [Reject]
```

The approval action is authenticated and task-bound.

---

# 129. Android Implementation Order

### Step 1
Create Kotlin/Compose application.

### Step 2
Create shared task/device contracts.

### Step 3
Implement local database.

### Step 4
Implement Android Keystore identity.

### Step 5
Implement PC pairing.

### Step 6
Implement secure LAN connection.

### Step 7
Implement assistant UI.

### Step 8
Implement local TTS.

### Step 9
Implement local STT.

### Step 10
Implement VAD.

### Step 11
Implement wake word.

### Step 12
Implement PC task routing.

### Step 13
Implement notifications.

### Step 14
Implement remote confirmations.

### Step 15
Implement camera.

### Step 16
Implement files.

### Step 17
Implement AccessibilityService integration if enabled.

### Step 18
Implement local small-model fallback.

### Step 19
Implement model manager.

### Step 20
Implement diagnostics.

### Step 21
Implement widgets/Quick Settings.

### Step 22
Implement release hardening.

---

# 130. Recommended Android Milestones

## Milestone A — Companion

- chat
- voice
- TTS
- PC pairing
- task status
- notifications

## Milestone B — Remote Controller

- PC commands
- task cancellation
- confirmation
- device management

## Milestone C — Mobile Assistant

- camera
- files
- contacts
- media
- local voice

## Milestone D — Local AI

- local STT
- local TTS
- local small LLM
- local VLM where hardware allows

## Milestone E — Advanced Automation

- AccessibilityService
- screen understanding
- mobile workflows
- application skills

---

# 131. Production Readiness Criteria

Android implementation is ready when:

- voice works reliably,
- wake-word operation is battery-conscious,
- PC pairing is secure,
- remote commands are authenticated,
- confirmations cannot be replayed,
- notifications work reliably,
- PC offline mode is handled,
- local STT/TTS work,
- Android permissions are clear,
- sensitive data is protected,
- app survives network changes,
- tasks synchronize correctly,
- camera/file operations work,
- local-only mode genuinely avoids cloud services,
- accessibility automation is explicitly user-enabled,
- release builds are signed and hardened.

---

# 132. Final Android Architecture

```text
                         USER
                           │
                  Voice / Touch / Widget
                           │
                           ▼
                 ┌───────────────────┐
                 │ Android JARVIS    │
                 │ Kotlin + Compose  │
                 └─────────┬─────────┘
                           │
              ┌────────────┼─────────────┐
              │            │             │
              ▼            ▼             ▼
          Voice Node    Local AI      Phone Skills
              │            │             │
        Wake/VAD/STT      Small LLM    Camera
              │            │            Files
              ▼            ▼            Media
             TTS          Fallback      Contacts
              │                         Location
              └────────────┬────────────┘
                           │
                           ▼
                  Device Gateway
                           │
                    TLS + Pairing
                           │
            ┌──────────────┴──────────────┐
            ▼                             ▼
       Ubuntu JARVIS                 Windows JARVIS
            │                             │
       Large Local LLM                Large Local LLM
       Browser                        Browser
       Desktop                       Desktop
       Terminal                      Native APIs
       VLM                           VLM
```

---

# 133. Final Architectural Rule

The Android application should **not** become a second unrelated JARVIS brain.

The intended architecture is:

```text
                  JARVIS
                    │
          Shared identity + protocols
                    │
       ┌────────────┼────────────┐
       │            │            │
     Linux       Windows      Android
       │            │            │
   execution     execution    mobile
   platform      platform     companion
```

The same user should be able to say:

> "Jarvis, run my tests."

from Android, Windows, or Ubuntu and receive the same conceptual behavior.

Only the available capabilities and execution device should change.

---

# 134. Relationship to the Next Documents

This document defines the Android execution/client layer.

It intentionally relies on later/previous documents for:

```text
AI model architecture
task planning
security policy
memory
skills
browser automation
cross-device protocol
testing
packaging
```

The next major subsystem should therefore address the **Browser + Computer-Use Engine**, because browser automation and general desktop/mobile UI interaction are what turn the JARVIS model from a conversational assistant into an agent capable of actually operating software.
