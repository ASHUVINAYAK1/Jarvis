# JARVIS — Document 4
# Cross-Platform OS & Application Automation
## Windows + Ubuntu/Linux + Android

**Project:** Local-first JARVIS personal assistant  
**Purpose:** Define the execution layer that allows the JARVIS agent to control real devices and applications safely, reliably, and cross-platform.

---

# 1. Position in the Overall Architecture

The JARVIS architecture is:

```text
Voice / Text / UI
       ↓
Local AI Engine
       ↓
Agent Core
       ↓
Policy Engine
       ↓
Tool Registry
       ↓
Platform Automation Layer
       ↓
Windows / Linux / Android
       ↓
Applications / Browser / OS
```

Previous documents defined:

- Document 1 — overall JARVIS system architecture
- Document 2 — local AI engine
- Document 3 — agent, planning, memory, tools and autonomy

This document defines the layer that actually performs actions.

The most important design principle is:

> The agent should request capabilities. Platform adapters should execute them.

The LLM should never contain Windows-specific, Linux-specific, or Android-specific control logic in its core reasoning.

---

# 2. Goals

The automation layer must allow JARVIS to:

- Open applications.
- Close applications.
- Focus windows.
- Move and resize windows.
- Type text.
- Press keyboard shortcuts.
- Control the mouse.
- Read accessibility trees.
- Inspect application state.
- Take screenshots.
- Read clipboard contents.
- Set clipboard contents.
- Upload files.
- Download files.
- Navigate websites.
- Click buttons.
- Fill forms.
- Scroll.
- Drag and drop.
- Manage processes.
- Launch commands.
- Read files.
- Write files.
- Control media.
- Control system volume.
- Control display brightness where supported.
- Display notifications.
- Detect login screens.
- Detect application state.
- Control Android applications.
- Recover from UI changes.
- Fall back to computer vision when semantic automation fails.

---

# 3. Non-Goals

The platform automation layer should not:

- Make high-level decisions.
- Decide whether an action is appropriate.
- Store passwords in plain text.
- Bypass CAPTCHA.
- Bypass security controls.
- Disable antivirus/security software.
- Execute arbitrary LLM-generated commands without policy validation.
- Assume coordinate-based automation is reliable.
- Depend on a single UI automation technology.

High-level decisions belong to the agent.

Authorization belongs to the policy engine.

Execution belongs here.

---

# 4. Core Abstraction

Define common interfaces.

```text
Platform
 ├── DesktopProvider
 ├── WindowProvider
 ├── InputProvider
 ├── ScreenProvider
 ├── ClipboardProvider
 ├── ProcessProvider
 ├── ApplicationProvider
 ├── AccessibilityProvider
 ├── NotificationProvider
 ├── FileProvider
 └── CredentialProvider
```

Browser:

```text
BrowserProvider
```

Android:

```text
AndroidProvider
```

The agent sees these abstract capabilities.

---

# 5. Platform Provider Model

```text
                 Agent Tool
                     ↓
              Platform Service
                     ↓
              Provider Interface
            ┌────────┼────────┐
            ↓        ↓        ↓
         Windows    Linux    Android
         Adapter    Adapter   Adapter
```

Example:

```text
app.open("Chrome")
```

does not become:

```text
start chrome.exe
```

inside the agent.

Instead:

```text
ApplicationProvider.open("Chrome")
```

The Windows adapter resolves the actual executable.

---

# 6. Recommended Architecture

Use three levels:

```text
Level 1 — Common Capability API

Level 2 — Platform Adapter

Level 3 — Native OS API
```

Example:

```text
InputProvider.type()
        ↓
WindowsInputProvider
        ↓
Windows UI/input API
```

Linux:

```text
InputProvider.type()
        ↓
LinuxInputProvider
        ↓
Wayland/X11/desktop-specific mechanism
```

Android:

```text
AndroidProvider.type()
        ↓
AccessibilityService/UI automation
```

---

# 7. Windows Automation Stack

Windows should use a layered architecture.

Recommended priority:

```text
Native application API
        ↓
Windows UI Automation
        ↓
Win32
        ↓
PowerShell / COM
        ↓
Keyboard/mouse injection
        ↓
Screenshot + vision
```

The correct mechanism depends on the application.

---

# 8. Windows UI Automation

Windows UI Automation is the primary semantic desktop automation mechanism.

It can expose concepts such as:

```text
Window
Button
Edit
CheckBox
ComboBox
List
Tree
Menu
Tab
Text
```

JARVIS can query:

```text
control type
name
automation ID
class
state
children
patterns
```

This is substantially safer than blindly clicking coordinates.

---

# 9. Windows UI Tree

Example:

```text
Desktop
 └── Chrome
      ├── Window
      ├── Address bar
      ├── Tab bar
      └── Web content
```

For a native application:

```text
Notepad
 └── Window
      ├── Menu
      ├── Toolbar
      └── Edit control
```

The automation layer should convert native trees into a normalized representation.

---

# 10. Normalized UI Element

Example:

```json
{
  "id": "element_123",
  "role": "button",
  "name": "Submit",
  "enabled": true,
  "visible": true,
  "bounds": {
    "x": 100,
    "y": 200,
    "width": 120,
    "height": 40
  },
  "actions": [
    "click"
  ]
}
```

The agent does not need to understand the underlying Windows API.

---

# 11. Windows Application Launching

Application discovery should search:

```text
Start Menu
Installed application metadata
known executable locations
registered applications
PATH
AppUserModel IDs
```

Store an application profile:

```json
{
  "name": "Visual Studio Code",
  "platform": "windows",
  "launch": {
    "type": "executable",
    "target": "..."
  }
}
```

Do not hard-code paths when discovery can resolve them.

---

# 12. Process Management

Windows provider:

```text
process.list()
process.start()
process.terminate()
process.get_info()
process.is_running()
```

Dangerous operations require policy checks.

---

# 13. Window Management

Support:

```text
window.list()
window.focus()
window.minimize()
window.maximize()
window.restore()
window.move()
window.resize()
window.close()
```

Window identity should use stable identifiers where possible.

Avoid relying solely on title text because titles change.

---

# 14. Multi-Monitor Support

Expose:

```text
screen.list()
screen.get_primary()
screen.get_bounds()
```

Example:

```json
{
  "screen_id": 1,
  "x": 0,
  "y": 0,
  "width": 1920,
  "height": 1080,
  "scale": 1.0
}
```

The vision layer must know which monitor contains the target.

---

# 15. Windows Input

Common interface:

```text
keyboard.type()
keyboard.press()
keyboard.hotkey()
mouse.move()
mouse.click()
mouse.double_click()
mouse.right_click()
mouse.scroll()
mouse.drag()
```

Input should support:

```text
Unicode
keyboard layouts
modifier keys
mouse buttons
scroll direction
```

---

# 16. Keyboard Reliability

Typing should prefer:

```text
application-native input
```

when available.

Fallback:

```text
OS input injection
```

For sensitive fields:

```text
credential provider
```

rather than putting the secret into an ordinary text-generation pipeline.

---

# 17. Clipboard

Common interface:

```text
clipboard.get()
clipboard.set()
clipboard.clear()
```

The clipboard must be treated as sensitive.

Do not automatically retain clipboard history indefinitely.

---

# 18. Clipboard Security

If JARVIS copies a password:

```text
copy
 ↓
paste
 ↓
clear clipboard
```

The credential should not be written to:

```text
logs
memory
LLM context
analytics
```

---

# 19. Windows Shell

A controlled shell provider can support:

```text
shell.run()
shell.which()
shell.environment()
```

Commands should be validated.

Prefer predefined tools:

```text
git.status
python.run_tests
npm.install
docker.status
```

over arbitrary shell strings.

---

# 20. Linux Architecture

Linux is more fragmented than Windows.

JARVIS must detect:

```text
desktop environment
display server
session type
accessibility bus
window manager
```

Examples:

```text
GNOME
KDE
XFCE
X11
Wayland
```

---

# 21. Linux Automation Layers

Preferred order:

```text
Application API
        ↓
AT-SPI accessibility
        ↓
D-Bus
        ↓
desktop-specific APIs
        ↓
X11 mechanisms
        ↓
Wayland-compatible input mechanisms
        ↓
vision fallback
```

---

# 22. AT-SPI

AT-SPI is important for Linux accessibility automation.

It can expose:

```text
application
window
button
text field
menu
list
tree
```

JARVIS should normalize AT-SPI data into the common UI representation.

---

# 23. D-Bus

D-Bus is useful for:

```text
desktop services
notifications
power
media
application communication
system services
```

Where a D-Bus API exists, prefer it over simulated UI interaction.

---

# 24. X11 vs Wayland

This distinction is critical.

X11 historically permits broad desktop input/screen automation.

Wayland intentionally restricts arbitrary applications from:

- reading other application windows,
- injecting global input,
- capturing the whole desktop without permission.

Therefore:

> Do not design the Linux automation layer assuming universal global mouse/keyboard control.

The system must detect the environment and select supported mechanisms.

---

# 25. Wayland Strategy

On Wayland:

```text
native application API
        ↓
accessibility
        ↓
portal APIs
        ↓
desktop-specific automation
        ↓
user-approved screen/input mechanisms
        ↓
vision where permitted
```

The assistant should gracefully report unsupported capabilities instead of attempting security workarounds.

---

# 26. Linux Application Launch

Support:

```text
.desktop files
PATH
package metadata
known application IDs
desktop application database
```

Example:

```text
app.open("Firefox")
```

resolves to an application profile.

---

# 27. Linux Process Control

Common interface:

```text
process.list()
process.start()
process.stop()
process.get_info()
```

Native implementation can use OS process facilities.

---

# 28. Linux Window Management

Expose:

```text
list
focus
minimize
maximize
restore
move
resize
close
```

But capabilities vary between:

```text
X11
Wayland
GNOME
KDE
```

The provider must report capability availability.

---

# 29. Capability Discovery

Every provider should expose:

```json
{
  "keyboard_global": true,
  "mouse_global": false,
  "screen_capture": true,
  "accessibility_tree": true,
  "window_control": false
}
```

The agent can then select another strategy.

---

# 30. Android Architecture

Android should not be treated as a miniature desktop.

Use:

```text
Android app
+
AccessibilityService
+
Intents
+
Notifications
+
optional ADB/developer bridge
```

The Android JARVIS component acts as an execution node.

---

# 31. Android AccessibilityService

AccessibilityService can provide:

```text
UI hierarchy
focused element
text
content descriptions
click actions
scroll actions
text input
```

This is the primary mechanism for controlling arbitrary supported applications.

---

# 32. Android UI Tree

Example:

```text
Root
 └── Activity
      ├── Toolbar
      ├── Search field
      ├── RecyclerView
      │    ├── Item
      │    └── Item
      └── Bottom navigation
```

JARVIS should normalize this into its common element model.

---

# 33. Android Intents

Use native intents whenever possible.

Examples:

```text
open browser
open maps
open dialer
open settings
share content
open file
open media
```

This is preferable to simulating taps.

---

# 34. Android App Launching

Support:

```text
package name
application label
launch intent
deep link
```

Example:

```text
app.open("YouTube")
```

resolves through package metadata.

---

# 35. Android Permissions

The JARVIS Android application may need carefully justified permissions for:

```text
microphone
notifications
accessibility
foreground service
screen capture where needed
```

Do not request all permissions at installation.

Request them when their capability is first needed.

---

# 36. Android Screen Capture

Where supported and authorized, use Android screen capture mechanisms.

The user must explicitly approve screen-capture access when Android requires it.

Never attempt to bypass this consent.

---

# 37. Android ADB

ADB is useful during development and controlled-device scenarios.

It can assist with:

```text
installing builds
debugging
log collection
test automation
```

Production JARVIS should not depend on ADB being enabled for ordinary users.

---

# 38. UIAutomator

UIAutomator is particularly useful for:

```text
automated testing
development
controlled Android workflows
```

It should complement AccessibilityService rather than become the sole production mechanism.

---

# 39. Android Background Execution

Android heavily restricts background work.

Use:

```text
Foreground Service
WorkManager
AlarmManager
notifications
```

depending on the task.

Long-running voice functionality must comply with Android background execution and microphone rules.

---

# 40. Browser Automation

Browser automation should have its own subsystem.

Recommended technology:

```text
Playwright
```

with Chromium/Chrome/Firefox support as appropriate.

The browser provider exposes:

```text
navigate
back
forward
reload
tabs
click
type
select
scroll
extract
upload
download
screenshot
wait
```

---

# 41. Browser Priority

Use:

```text
DOM
 ↓
Accessibility snapshot
 ↓
browser semantics
 ↓
OCR
 ↓
vision
 ↓
coordinates
```

For web pages, DOM interaction should be the default.

---

# 42. Browser Context

Maintain:

```text
browser instance
browser context
page
tab
URL
cookies/session
downloads
uploads
```

The credential system should remain separate from browser automation.

---

# 43. Existing User Session

JARVIS should support detecting whether the user is already logged in.

Possible indicators:

```text
known authenticated DOM state
account UI
redirect behavior
session state
```

Do not attempt to extract session cookies merely to determine login status.

---

# 44. Login Flow

```text
Open site
 ↓
Check authenticated state
 ├── logged in → continue
 └── logged out
       ↓
   credential available?
       ├── yes → secure login mechanism
       └── no → ask user to log in
```

---

# 45. Browser Profiles

Use a dedicated controlled browser profile where possible.

Possible modes:

```text
JARVIS profile
Existing user profile
Temporary task profile
```

A dedicated profile improves reproducibility and reduces interference.

---

# 46. Browser Downloads

Downloads should be tracked:

```json
{
  "download_id": "d1",
  "filename": "resume.pdf",
  "path": "...",
  "source_url": "...",
  "task_id": "..."
}
```

The agent should know when a download has completed.

---

# 47. Browser Uploads

For file upload:

```text
agent identifies file
 ↓
policy check
 ↓
browser.set_input_files()
 ↓
observe
 ↓
verify uploaded filename
```

Do not upload arbitrary files without knowing what they are.

---

# 48. File Dialogs

Native file dialogs may not be part of the webpage DOM.

Priority:

```text
browser-native upload API
 ↓
OS accessibility
 ↓
keyboard
 ↓
vision
```

---

# 49. Drag and Drop

Drag-and-drop is inherently fragile.

Use:

```text
native application API
```

where possible.

Otherwise:

```text
semantic target
 ↓
calculate bounds
 ↓
mouse drag
 ↓
verify
```

---

# 50. Vision Fallback

Computer vision should be the fallback when semantic automation fails.

Pipeline:

```text
Screenshot
 ↓
preprocess
 ↓
vision model
 ↓
element detection
 ↓
target grounding
 ↓
action
 ↓
screenshot
 ↓
verification
```

---

# 51. Vision Action Representation

Do not let the vision model directly execute actions.

It returns:

```json
{
  "target": {
    "description": "blue Submit button",
    "bounds": [800, 620, 980, 680]
  },
  "confidence": 0.91
}
```

The executor validates it.

---

# 52. Coordinate Safety

Before clicking coordinates:

```text
screen unchanged?
target still visible?
window still focused?
confidence sufficient?
```

If not:

```text
re-screenshot
re-ground
```

---

# 53. UI Change Detection

Compute a lightweight UI state signature:

```text
DOM hash
accessibility hash
screenshot perceptual hash
```

If the environment changes unexpectedly:

```text
invalidate cached element references
```

---

# 54. Element Lifetime

Never assume an element ID remains valid after:

```text
navigation
DOM update
application refresh
modal change
```

Re-query after significant state changes.

---

# 55. Waiting Strategy

Never rely on:

```text
sleep(5)
```

Prefer:

```text
wait_for_element
wait_for_url
wait_for_network_idle
wait_for_text
wait_for_process
wait_for_window
```

Timeouts remain mandatory.

---

# 56. Application State Detection

Applications can report:

```text
running
foreground
authenticated
busy
ready
dialog_open
error
```

State detection should use native APIs where possible.

---

# 57. Dialog Handling

Common dialogs:

```text
permission
save
open
error
confirmation
update
cookie
notification
```

The agent should classify unexpected dialogs.

Example:

> "Chrome is asking whether to allow notifications. Do you want me to allow it?"

---

# 58. Popup Handling

Browser automation should detect:

```text
new tab
new window
modal
popup
permission dialog
download prompt
```

Do not automatically dismiss all popups.

Some may contain important information.

---

# 59. Notification Handling

JARVIS should expose:

```text
notify.send()
notify.list()
```

Platform adapters implement native notifications.

---

# 60. Media Control

Common interface:

```text
media.play
media.pause
media.next
media.previous
media.volume
media.get_state
```

Prefer system media APIs over UI automation.

---

# 61. System Volume

Common API:

```text
audio.get_volume()
audio.set_volume()
audio.mute()
audio.unmute()
```

The platform implementation handles:

```text
Windows
PulseAudio/PipeWire
Android audio manager
```

---

# 62. Microphone State

Expose:

```text
mic.list()
mic.default()
mic.mute()
mic.unmute()
```

The voice subsystem consumes the selected microphone.

---

# 63. Camera

Camera capabilities should be isolated.

Possible tools:

```text
camera.list
camera.capture
```

Camera access should require explicit user authorization.

---

# 64. File Operations

The platform layer should expose:

```text
read
write
copy
move
rename
delete
search
watch
```

File tools should use sandboxed allowed roots.

---

# 65. File Sandbox

Example:

```text
Allowed:
~/Documents
~/Downloads
~/Projects

Denied by default:
system directories
credential stores
browser profile internals
private application data
```

The policy engine can grant temporary access.

---

# 66. File Watcher

JARVIS can subscribe to:

```text
file.created
file.modified
file.deleted
```

This enables workflows such as:

> "Whenever I download a PDF, organize it into Documents."

---

# 67. Cross-Device Architecture

Windows and Ubuntu can run a JARVIS node.

Android can run another node.

```text
                 JARVIS Core
                     │
            secure local network
          ┌──────────┼──────────┐
          ↓          ↓          ↓
       Windows     Ubuntu     Android
        Node        Node       Node
```

---

# 68. Device Registry

Each node reports:

```json
{
  "device_id": "...",
  "platform": "windows",
  "capabilities": [
    "desktop",
    "browser",
    "filesystem",
    "microphone"
  ],
  "online": true
}
```

---

# 69. Task Handoff

User could say:

> "Start this download on my PC and notify me on my phone when it finishes."

Architecture:

```text
Task created on PC
 ↓
background worker
 ↓
download complete
 ↓
event bus
 ↓
Android notification
```

---

# 70. Secure Device Communication

Use authenticated encrypted communication.

Possible architecture:

```text
TLS
+
device identity
+
pairing
+
short-lived tokens
```

Never expose an unauthenticated automation API to the local network.

---

# 71. Local Network Threat Model

Do not assume:

```text
LAN = trusted
```

A compromised device on the same network should not be able to:

```text
control JARVIS
read memory
execute tools
```

without authentication.

---

# 72. Node Authentication

Use:

```text
device key pair
certificate/public key
pairing approval
```

A newly discovered device should require explicit pairing.

---

# 73. Remote Tool Calls

Example:

```text
Agent
 ↓
device.select("windows-pc")
 ↓
tool request
 ↓
authenticated RPC
 ↓
Windows provider
 ↓
result
```

---

# 74. RPC

Use one of:

```text
gRPC
WebSocket
local HTTP
```

A practical architecture:

```text
gRPC/WebSocket
```

for persistent node communication.

---

# 75. Localhost Security

For desktop services:

```text
bind localhost by default
```

Do not bind automation APIs to:

```text
0.0.0.0
```

unless secure remote operation is explicitly configured.

---

# 76. Startup Architecture

Windows:

```text
JARVIS service
+
tray/UI process
+
agent runtime
```

Linux:

```text
systemd user service
+
tray/UI
+
agent runtime
```

Android:

```text
application
+
foreground service when appropriate
```

---

# 77. Startup Sequence

```text
OS starts
 ↓
JARVIS runtime
 ↓
hardware detection
 ↓
load configuration
 ↓
start local inference
 ↓
start voice service
 ↓
start agent supervisor
 ↓
register tools
 ↓
register device
 ↓
ready
```

The system should expose:

> "JARVIS online."

only after essential subsystems are healthy.

---

# 78. Health Checks

Each subsystem reports:

```text
READY
DEGRADED
FAILED
```

Example:

```text
LLM: READY
Whisper: READY
TTS: READY
Browser: READY
Vision: DEGRADED
Android node: OFFLINE
```

---

# 79. Graceful Degradation

If vision fails:

```text
DOM/accessibility still works
```

If local GPU inference fails:

```text
CPU model
```

If browser automation fails:

```text
semantic desktop automation
```

If Android is offline:

```text
PC functionality continues
```

JARVIS should not collapse because one component fails.

---

# 80. Capability Matrix

The agent should query capabilities dynamically.

```text
Windows:
desktop = yes
browser = yes
global_input = yes
accessibility = yes

Ubuntu Wayland:
desktop = partial
browser = yes
global_input = restricted
accessibility = yes

Android:
desktop = no
app_control = yes
browser = yes
accessibility = yes
```

---

# 81. Common Tool API

Example:

```python
class ApplicationProvider:
    async def list(self): ...
    async def open(self, app): ...
    async def close(self, app): ...
    async def focus(self, app): ...
```

The agent uses this interface.

---

# 82. Browser Provider

```python
class BrowserProvider:
    async def open(self, url): ...
    async def tabs(self): ...
    async def click(self, target): ...
    async def type(self, target, text): ...
    async def extract(self, query): ...
    async def screenshot(self): ...
```

---

# 83. Input Provider

```python
class InputProvider:
    async def type(self, text): ...
    async def press(self, key): ...
    async def hotkey(self, keys): ...
    async def click(self, x, y): ...
    async def scroll(self, amount): ...
```

---

# 84. Accessibility Provider

```python
class AccessibilityProvider:
    async def get_tree(self, root=None): ...
    async def find(self, role=None, name=None): ...
    async def click(self, element): ...
    async def set_text(self, element, text): ...
```

---

# 85. Screen Provider

```python
class ScreenProvider:
    async def list(self): ...
    async def capture(self, screen=None): ...
    async def get_cursor(self): ...
```

---

# 86. Process Provider

```python
class ProcessProvider:
    async def list(self): ...
    async def start(self, executable, args=None): ...
    async def terminate(self, pid): ...
```

---

# 87. Android Provider

```python
class AndroidProvider:
    async def list_apps(self): ...
    async def open_app(self, package): ...
    async def get_ui_tree(self): ...
    async def click(self, element): ...
    async def type(self, element, text): ...
    async def swipe(self, start, end): ...
    async def screenshot(self): ...
```

---

# 88. Browser vs Desktop Separation

Chrome is both:

```text
desktop application
```

and:

```text
browser
```

JARVIS should not use desktop clicking for webpage elements if Playwright can access the DOM.

Use:

```text
BrowserProvider
```

for web content.

Use:

```text
ApplicationProvider
WindowProvider
```

for the Chrome application itself.

---

# 89. Example: "Open LinkedIn"

```text
Intent
 ↓
browser.open
 ↓
BrowserProvider
 ↓
launch browser if needed
 ↓
navigate
 ↓
observe page
```

---

# 90. Example: "Apply for Jobs"

```text
Agent
 ↓
BrowserProvider
 ↓
search
 ↓
extract jobs
 ↓
Agent ranks
 ↓
BrowserProvider opens job
 ↓
Accessibility/DOM inspection
 ↓
fill form
 ↓
Policy
 ↓
confirmation if required
 ↓
submit
 ↓
verify
```

---

# 91. Example: Native Application

User:

> "Open VS Code and my project."

Flow:

```text
ApplicationProvider.open(VS Code)
 ↓
ProcessProvider.verify
 ↓
ApplicationProvider.focus
 ↓
FileProvider.resolve(project)
 ↓
VS Code native/CLI interface
 ↓
verify workspace
```

---

# 92. Example: Linux Terminal

User:

> "Open a terminal and run the project's tests."

Flow:

```text
app.open("Terminal")
 ↓
project.resolve()
 ↓
developer.test_project()
 ↓
policy
 ↓
execute
 ↓
capture stdout/stderr
 ↓
agent summarizes
```

A predefined developer tool is preferable to unconstrained shell execution.

---

# 93. Example: Android

User:

> "Open WhatsApp."

```text
AndroidProvider
 ↓
resolve package
 ↓
launch intent
 ↓
verify foreground app
```

---

# 94. Example: Android Form

User:

> "Fill this form with my profile."

```text
AccessibilityService
 ↓
UI tree
 ↓
field mapping
 ↓
profile lookup
 ↓
policy
 ↓
set text
 ↓
verify fields
```

---

# 95. Vision Escalation

Use vision only after:

```text
native API failed
accessibility unavailable
DOM unavailable
```

Escalation:

```text
API
→ accessibility
→ DOM
→ OCR
→ vision
→ coordinate input
```

---

# 96. Confidence Thresholds

Each automation strategy should provide confidence.

Example:

```text
DOM target: 0.99
Accessibility target: 0.96
OCR target: 0.87
Vision target: 0.81
Coordinate guess: 0.55
```

Below the action threshold:

```text
do not act
```

Ask the user or gather more observations.

---

# 97. Verification

Every meaningful UI action should have:

```text
action
 ↓
expected state
 ↓
observation
 ↓
verification
```

Example:

```text
click "Send"
 ↓
expected: message appears in sent list
 ↓
observe
 ↓
verified
```

---

# 98. Idempotency

Platform tools must declare:

```text
safe_to_retry
```

Examples:

```text
focus_window = yes
open_app = mostly
send_message = no
delete_file = no
submit_application = no
```

---

# 99. Timeouts

Every operation needs a timeout.

Examples:

```text
launch app: 15s
page navigation: 30s
UI element wait: 10s
file operation: task-dependent
vision inference: model-dependent
```

No infinite waits.

---

# 100. Error Taxonomy

Normalize errors:

```text
NOT_FOUND
PERMISSION_DENIED
TIMEOUT
ALREADY_EXISTS
NOT_SUPPORTED
AUTH_REQUIRED
UI_CHANGED
NETWORK_ERROR
DEVICE_OFFLINE
POLICY_DENIED
USER_CANCELLED
```

This lets the agent recover intelligently.

---

# 101. Platform Capability Errors

Example:

```json
{
  "error": "NOT_SUPPORTED",
  "capability": "global_mouse_control",
  "platform": "linux-wayland"
}
```

The planner can select another method.

---

# 102. Testing Architecture

Use:

```text
unit tests
provider tests
integration tests
end-to-end tests
UI fixtures
virtual machines
Android emulator
```

---

# 103. Windows Testing

Test:

```text
Windows 10/11 where supported
native apps
Chrome
Edge
VS Code
Explorer
Terminal
Office applications where applicable
```

---

# 104. Linux Testing

Test at minimum:

```text
Ubuntu GNOME
X11 session where available
Wayland session
Chrome/Chromium
Firefox
VS Code
Terminal
Files
```

The provider must explicitly report differences.

---

# 105. Android Testing

Use:

```text
Android emulator
physical device
different screen sizes
different Android versions
```

Test:

```text
Accessibility
notifications
foreground service
app launch
UI interaction
screen changes
```

---

# 106. Browser Testing

Maintain deterministic test pages:

```text
login page
form page
dynamic page
modal page
infinite scroll page
file upload page
download page
error page
```

This is much safer than testing only against production websites.

---

# 107. Failure Injection

Simulate:

```text
network disconnected
application crashed
element renamed
window moved
login expired
device disconnected
model unavailable
screen changed
```

The automation layer must recover or report accurately.

---

# 108. Performance

Measure:

```text
application launch latency
UI tree extraction latency
screenshot latency
DOM extraction latency
input latency
tool round-trip latency
Android RPC latency
```

Avoid unnecessary screenshots.

---

# 109. Screenshot Policy

Screenshots are expensive and sensitive.

Use:

```text
DOM/accessibility first
```

and capture screenshots only when:

```text
visual grounding needed
debugging
user explicitly requests
verification requires it
```

---

# 110. Privacy

The automation layer can see:

```text
screen contents
documents
browser pages
notifications
clipboard
```

Therefore it must have strict data boundaries.

Default:

```text
local only
no telemetry
no remote upload
```

---

# 111. Logging

Log:

```text
action type
target identifier
success/failure
duration
task ID
```

Do not log:

```text
password
access token
private message body unnecessarily
clipboard secrets
full screenshots by default
```

---

# 112. Repository Structure

Recommended:

```text
platform/
│
├── common/
│   ├── interfaces/
│   ├── models/
│   ├── capabilities/
│   └── errors/
│
├── windows/
│   ├── applications/
│   ├── windows/
│   ├── input/
│   ├── accessibility/
│   ├── screen/
│   ├── processes/
│   └── notifications/
│
├── linux/
│   ├── applications/
│   ├── accessibility/
│   ├── dbus/
│   ├── x11/
│   ├── wayland/
│   ├── input/
│   └── screen/
│
├── android/
│   ├── accessibility/
│   ├── intents/
│   ├── input/
│   ├── screen/
│   ├── notifications/
│   └── apps/
│
└── browser/
    ├── playwright/
    ├── profiles/
    ├── downloads/
    ├── uploads/
    └── grounding/
```

---

# 113. Recommended Technology Choices

## Windows

Use:

```text
Python agent layer
Windows UI Automation
Win32 where required
PowerShell for controlled system tasks
COM where appropriate
native Windows APIs
```

## Linux

Use:

```text
Python
AT-SPI
D-Bus
X11-compatible mechanisms where available
Wayland/portal-compatible mechanisms
desktop-specific APIs
```

## Android

Use:

```text
Kotlin
Android SDK
AccessibilityService
Intents
WorkManager
Foreground Service where required
UIAutomator for controlled automation/testing
```

## Browser

Use:

```text
Playwright
Chromium/Chrome
```

---

# 114. Why Kotlin for Android

The Android execution node should be native Kotlin.

Reasons:

- Best Android API access.
- AccessibilityService integration.
- Lifecycle management.
- Permissions.
- Notifications.
- Background execution.
- Battery management.
- Platform compatibility.

The Android node communicates with the shared JARVIS backend/core.

---

# 115. Why Not Build Android Automation Entirely in Python

Python can be useful for experimentation, but Android platform control needs native APIs.

Use:

```text
Kotlin Android node
```

and communicate with:

```text
Python JARVIS core
```

through a secure protocol.

---

# 116. Cross-Platform Core Language

Recommended initial architecture:

```text
Python
```

for:

```text
agent
AI
orchestration
tools
policy
memory
task system
```

Native components:

```text
Kotlin → Android
C/C++/Rust → performance-critical components only if needed
```

---

# 117. Browser Runtime

Browser automation can be isolated:

```text
Playwright worker
```

This avoids coupling the agent directly to browser internals.

---

# 118. Tool Execution Service

The platform layer should expose a local service:

```text
jarvis-platform-service
```

Responsibilities:

```text
provider discovery
tool execution
capability reporting
native API access
security enforcement
```

---

# 119. Example Request

```json
{
  "request_id": "req_123",
  "tool": "application.open",
  "arguments": {
    "name": "Chrome"
  }
}
```

Response:

```json
{
  "request_id": "req_123",
  "status": "success",
  "application": "Chrome",
  "pid": 1234
}
```

---

# 120. Example Browser Request

```json
{
  "tool": "browser.click",
  "arguments": {
    "element": {
      "role": "button",
      "name": "Apply"
    }
  }
}
```

The browser provider resolves the actual DOM element.

---

# 121. Example Vision Request

```json
{
  "tool": "screen.find",
  "arguments": {
    "description": "Apply button"
  }
}
```

Response:

```json
{
  "status": "success",
  "confidence": 0.91,
  "bounds": [820, 600, 960, 650]
}
```

The executor then validates and performs the click.

---

# 122. Security Boundary

The architecture should be:

```text
LLM
 ↓
Agent
 ↓
Policy
 ↓
Tool
 ↓
Platform Service
 ↓
OS
```

Never:

```text
LLM
 ↓
OS shell
```

---

# 123. Startup Security

At startup:

```text
load signed/known configuration
 ↓
initialize local services
 ↓
verify model files
 ↓
verify tool registry
 ↓
start automation service
```

Unexpected binaries or plugins should not automatically become trusted tools.

---

# 124. Application Plugins

Future JARVIS plugins can define:

```text
application profile
native API
UI selectors
workflow definitions
```

Example:

```text
plugins/
  linkedin/
  github/
  spotify/
  vscode/
```

But plugins must be permission-scoped.

---

# 125. Workflow Definitions

Some repetitive tasks should be encoded as deterministic workflows.

Example:

```yaml
name: open-development-environment

steps:
  - app.open: "VS Code"
  - app.open: "Terminal"
  - browser.open: "GitHub"
```

The LLM can invoke this workflow rather than rediscovering it every time.

---

# 126. Hybrid Automation

The ideal JARVIS system combines:

```text
Deterministic workflows
+
Native APIs
+
Semantic UI automation
+
Browser DOM automation
+
Accessibility
+
Vision
```

This gives substantially better reliability than a pure computer-use agent.

---

# 127. Computer Use Should Be the Last Layer

Do not build JARVIS around:

```text
screenshot → LLM → click coordinate
```

That approach is expensive and fragile.

Instead:

```text
API → accessibility/DOM → structured UI → vision
```

---

# 128. End-to-End Example

User:

> "JARVIS, open Chrome and apply to suitable SDE jobs."

## Step 1

Speech subsystem produces:

```text
open Chrome and apply to suitable SDE jobs
```

## Step 2

Agent creates task.

## Step 3

Planner requests:

```text
application.open
browser.navigate
browser.search
browser.extract
browser.fill
browser.submit
```

## Step 4

Policy validates.

## Step 5

Windows provider opens Chrome.

## Step 6

Browser provider navigates.

## Step 7

DOM/accessibility state is inspected.

## Step 8

Login state is determined.

## Step 9

If login required:

> "LinkedIn requires you to log in, sir."

## Step 10

User logs in.

## Step 11

Agent resumes.

## Step 12

Jobs are searched.

## Step 13

Agent evaluates jobs.

## Step 14

Forms are filled.

## Step 15

Policy determines whether confirmation is required.

## Step 16

User confirms.

## Step 17

Application is submitted.

## Step 18

Submission is verified.

## Step 19

Task is persisted.

## Step 20

JARVIS says:

> "The application was submitted successfully."

---

# 129. Failure Example

Suppose the website changes its button.

```text
DOM selector fails
 ↓
accessibility lookup
 ↓
fails
 ↓
screenshot
 ↓
vision identifies button
 ↓
policy
 ↓
click
 ↓
verify
```

If vision confidence is low:

> "The page layout changed and I can't safely identify the Submit button. Please point me to it."

---

# 130. Android Example

User:

> "JARVIS, open Maps and find the nearest office."

```text
AndroidProvider
 ↓
launch Maps
 ↓
intent/deep link
 ↓
location/search UI
 ↓
accessibility
 ↓
enter query
 ↓
observe results
 ↓
voice response
```

---

# 131. Linux Example

User:

> "Open my project in VS Code and run the tests."

```text
Linux ApplicationProvider
 ↓
open VS Code
 ↓
workspace detection
 ↓
developer tool
 ↓
controlled process execution
 ↓
collect output
 ↓
agent analyzes
 ↓
voice response
```

---

# 132. Windows Example

User:

> "Open Downloads and move all PDFs into Documents."

```text
FileProvider.list
 ↓
filter PDFs
 ↓
preview affected files
 ↓
policy
 ↓
move
 ↓
verify
 ↓
report
```

For mass changes, confirmation should normally be required.

---

# 133. Implementation Order

Recommended implementation order:

```text
1. Common provider interfaces
2. Windows application provider
3. Windows input
4. Windows screen capture
5. Windows accessibility
6. Browser provider
7. Linux application provider
8. Linux accessibility
9. Linux input/capability detection
10. Android node
11. Cross-device RPC
12. Vision fallback
13. Advanced application plugins
```

---

# 134. First Working Prototype

The first prototype should support only:

```text
open app
close app
focus app
type
hotkeys
screenshot
browser open
browser click
browser type
browser extract
```

Run it on Windows first.

Then Linux.

Then Android.

---

# 135. Why Windows First

For a desktop-first JARVIS prototype, Windows provides a relatively straightforward environment for:

- desktop automation,
- application launching,
- UI Automation,
- keyboard/mouse control,
- browser automation,
- process management.

Once the common interface is stable, Linux can implement the same contract.

---

# 136. Linux Development Strategy

Build and test separately for:

```text
Ubuntu X11
Ubuntu Wayland
```

Do not assume that an automation technique working on X11 works on Wayland.

Capability detection is mandatory.

---

# 137. Android Development Strategy

Android should be developed as an independent native client:

```text
android/
    app/
    accessibility/
    communication/
    notifications/
```

It communicates with the shared JARVIS system.

---

# 138. Cross-Platform Interface Stability

The common API should remain stable.

Example:

```text
application.open()
```

may have completely different implementations.

That is acceptable.

The agent should not care.

---

# 139. Final Architecture

```text
                        JARVIS AGENT
                              │
                              ▼
                        POLICY ENGINE
                              │
                              ▼
                        TOOL REGISTRY
                              │
                              ▼
                     PLATFORM SERVICE
                              │
          ┌───────────────────┼───────────────────┐
          │                   │                   │
          ▼                   ▼                   ▼
       WINDOWS              LINUX              ANDROID
          │                   │                   │
   ┌──────┼──────┐     ┌──────┼──────┐      ┌─────┼─────┐
   │      │      │     │      │      │      │     │     │
 WinUI   Win32  APIs  AT-SPI D-Bus  APIs  A11y  Intent  SDK
   │      │      │     │      │      │      │     │
   └──────┴──────┘     └──────┴──────┘      └─────┴─────┘
          │                   │                   │
          └───────────────────┼───────────────────┘
                              ▼
                       REAL APPLICATIONS
```

---

# 140. Final Engineering Principles

1. Build common interfaces first.
2. Keep platform code isolated.
3. Prefer native APIs.
4. Prefer semantic UI access.
5. Prefer DOM for websites.
6. Use accessibility before vision.
7. Use vision before coordinate guessing.
8. Verify every meaningful action.
9. Detect capabilities dynamically.
10. Treat Wayland as a distinct environment.
11. Treat Android as a native execution node.
12. Keep secrets outside the LLM.
13. Keep automation APIs authenticated.
14. Make every operation cancellable.
15. Add timeouts everywhere.
16. Make side effects idempotent where possible.
17. Maintain an audit trail.
18. Never bypass platform security controls.
19. Fail safely when automation confidence is low.
20. Keep the agent independent of platform implementation details.

---

# 141. Relationship to the Other Documents

```text
DOCUMENT 1
Overall JARVIS architecture
        ↓
DOCUMENT 2
Local AI Engine
        ↓
DOCUMENT 3
Agent Core
        ↓
DOCUMENT 4
OS + Application Automation
        ↓
Actual Device Control
```

The resulting architecture is:

```text
LOCAL MODELS
     ↓
REASONING
     ↓
PLANNING
     ↓
POLICY
     ↓
TOOLS
     ↓
PLATFORM ADAPTERS
     ↓
OPERATING SYSTEM
     ↓
APPLICATION
     ↓
OBSERVATION
     ↓
AGENT
```

This is the execution foundation required for the JARVIS vision: a local AI companion capable of interacting with the user's computer and phone rather than merely answering questions.
