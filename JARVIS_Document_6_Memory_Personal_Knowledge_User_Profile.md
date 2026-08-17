# JARVIS — Document 6
# Memory, Personal Knowledge & User Profile System
## Local-First Personal Memory, RAG, User Profile, Knowledge Graphs, Retrieval, Privacy & Cross-Device Synchronization

**Project:** Local-first JARVIS personal assistant  
**Document:** 6 — Memory, Personal Knowledge & User Profile System  
**Depends on:** Documents 1–5

---

# 1. Purpose

A convincing personal assistant cannot rely only on the current conversation.

JARVIS needs a memory system that can remember useful information about the user, tasks, projects, documents, preferences, previous decisions, browser workflows, applications, routines, and interactions.

However, simply storing every conversation is a poor architecture.

The JARVIS memory system must determine:

- what should be remembered;
- what should remain temporary;
- how long information should remain available;
- how information should be retrieved;
- how memories should be ranked;
- how conflicting memories should be handled;
- how memories should be corrected;
- how memories should be deleted;
- how sensitive information should be protected;
- how memory should work offline;
- how memory should synchronize between Windows, Ubuntu and Android.

The target architecture is:

```text
                USER
                  │
                  ▼
             JARVIS AGENT
                  │
       ┌──────────┴──────────┐
       ▼                     ▼
  Memory Writer          Memory Reader
       │                     │
       ▼                     ▼
 Memory Store            Retrieval
       │                     │
       └──────────┬──────────┘
                  ▼
             Context Builder
                  │
                  ▼
              Local LLM
```

---

# 2. Core Principle

JARVIS should not have one giant "memory database."

Instead, memory should be divided by purpose.

Recommended categories:

```text
Working Memory
Short-Term Memory
Episodic Memory
Semantic Memory
Procedural Memory
User Profile
Project Memory
Task Memory
Document Knowledge
Application History
Preference Memory
Device Memory
Relationship/Entity Memory
```

Each category has different retention and retrieval rules.

---

# 3. Memory Hierarchy

The overall hierarchy:

```text
                    JARVIS MEMORY
                          │
       ┌──────────────────┼──────────────────┐
       │                  │                  │
       ▼                  ▼                  ▼
    Working            Long-Term          Knowledge
    Memory              Memory             Base
       │                  │                  │
       │        ┌─────────┼─────────┐        │
       │        ▼         ▼         ▼        │
       │     Episodic  Semantic  Procedural  │
       │                                      │
       └──────────────────────────────────────┘
```

---

# 4. Working Memory

Working memory contains information needed for the current task.

Example:

User:

> "Open LinkedIn and apply to the first two jobs."

Working memory:

```text
current task
current browser
selected jobs
current page
current form
temporary variables
pending confirmation
```

Working memory should expire quickly.

---

# 5. Working Memory Example

```json
{
  "task_id": "task_123",
  "goal": "Apply to first two matching SDE jobs",
  "current_site": "linkedin.com",
  "selected_jobs": [
    "job_1",
    "job_2"
  ],
  "current_job": "job_1",
  "pending_action": "submit"
}
```

---

# 6. Short-Term Memory

Short-term memory preserves recent context.

Examples:

```text
last few user instructions
recent conversation
recent browser page
recent corrections
current project discussion
recent entities
```

Retention can be:

```text
minutes
hours
days
```

depending on the memory type.

---

# 7. Episodic Memory

Episodic memory represents events.

Example:

```text
On August 17:
User asked JARVIS to search for SDE jobs.
JARVIS found 15 jobs.
User approved 3.
2 applications were submitted.
```

This is different from a permanent fact.

---

# 8. Semantic Memory

Semantic memory stores facts.

Examples:

```text
User's preferred programming languages
User's preferred job roles
User's preferred IDE
User's preferred browser
```

Semantic memory should be structured and searchable.

---

# 9. Procedural Memory

Procedural memory represents how the user prefers tasks to be performed.

Examples:

```text
When applying for jobs:
use the latest software-engineering resume.

When downloading files:
save them in Documents/Downloads.

When opening music:
use Spotify.

When sending messages:
ask before sending.
```

---

# 10. User Profile

The user profile is authoritative structured information.

Example:

```json
{
  "identity": {},
  "contact": {},
  "education": [],
  "experience": [],
  "skills": [],
  "career": {},
  "preferences": {},
  "documents": {},
  "devices": {}
}
```

The profile should not be implemented as arbitrary LLM-generated prose.

---

# 11. Profile vs Memory

Important distinction:

```text
Profile = structured current truth

Memory = contextual historical information
```

Example:

Profile:

```text
preferred_role = Software Engineer
```

Memory:

```text
User previously considered DevOps roles but decided to prioritize software engineering.
```

---

# 12. Source of Truth

Different data types need different sources of truth.

```text
Identity → User Profile
Passwords → Secure Credential Store
Documents → Document Store
Applications → Application Database
Preferences → Preference Store
Conversation events → Memory Store
Task state → Task Store
```

Do not store everything in the vector database.

---

# 13. Memory Types

Recommended enum:

```text
WORKING
SHORT_TERM
EPISODIC
SEMANTIC
PROCEDURAL
PREFERENCE
PROFILE
PROJECT
TASK
DOCUMENT
APPLICATION
ENTITY
```

---

# 14. Memory Record

A generic memory record:

```json
{
  "id": "mem_123",
  "type": "semantic",
  "content": "User prefers software engineering roles.",
  "source": "user_statement",
  "confidence": 0.99,
  "created_at": "...",
  "updated_at": "...",
  "expires_at": null,
  "importance": 0.85
}
```

---

# 15. Memory Metadata

Each memory should have:

```text
ID
type
content
source
confidence
importance
created_at
updated_at
last_accessed
access_count
expiration
sensitivity
scope
entities
tags
embedding
```

---

# 16. Source Types

Possible sources:

```text
explicit_user_statement
user_correction
user_profile
conversation
task_result
browser_observation
document
application
system_event
device
derived_inference
```

---

# 17. Explicit vs Inferred Memory

This distinction is critical.

Explicit:

> "Remember that I prefer remote jobs."

Inference:

```text
User has applied to several remote jobs.
```

The second should not automatically become:

```text
User always prefers remote jobs.
```

Inferences need lower confidence.

---

# 18. Confidence

Use confidence:

```text
1.0 = explicitly confirmed
0.9 = strong evidence
0.7 = likely
0.5 = uncertain
```

The exact numeric values are implementation details.

---

# 19. Importance

Memory importance should determine retention.

Examples:

```text
Name → very high
Preferred role → high
Temporary webpage → low
One-time search result → low
Long-term project decision → high
```

---

# 20. Sensitivity

Classify information:

```text
PUBLIC
PERSONAL
SENSITIVE
HIGHLY_SENSITIVE
SECRET
```

Examples:

```text
favorite IDE → PERSONAL

phone number → SENSITIVE

password → SECRET
```

Passwords should not be stored as ordinary memories at all.

---

# 21. Never Put Passwords in Memory

This is a hard rule.

Do not store:

```text
password
OTP
private key
API token
session cookie
credit card CVV
```

inside the general memory database.

Use the secure credential system.

---

# 22. Memory Creation

The LLM should not freely write permanent memories.

Instead:

```text
conversation
 ↓
Memory Candidate Extractor
 ↓
classification
 ↓
policy
 ↓
deduplication
 ↓
user confirmation if necessary
 ↓
memory store
```

---

# 23. Memory Candidate

Example:

User:

> "From now on, use my software engineering resume for developer jobs."

Candidate:

```json
{
  "type": "preference",
  "content": "Use software engineering resume for developer jobs.",
  "scope": "job_applications",
  "confidence": 1.0
}
```

---

# 24. Automatic Memory Rules

Good candidates:

```text
explicit long-term preferences
project decisions
stable workflow preferences
repeated corrections
application history
device preferences
```

Poor candidates:

```text
every sentence
temporary mood
random web facts
temporary UI state
one-off search queries
```

---

# 25. Memory Confirmation

For important memory, JARVIS can say:

> "Should I remember that for future job applications?"

This is preferable to silently creating potentially unwanted memories.

---

# 26. User Commands

Support:

```text
"Remember that I prefer remote jobs."

"Forget that."

"What do you remember about my job preferences?"

"Change my preferred role."

"Delete everything you remember about LinkedIn."

"Don't remember this conversation."
```

---

# 27. Memory Query

Example:

> "What do you remember about my job search?"

Retrieve:

```text
preferred roles
salary preference
locations
past applications
resume preference
recent decisions
```

---

# 28. Memory Retrieval

Retrieval should combine multiple strategies.

Recommended:

```text
keyword search
semantic search
metadata filtering
recency
importance
entity matching
relationship traversal
```

Not semantic search alone.

---

# 29. Hybrid Retrieval

Architecture:

```text
Query
 │
 ├── keyword search
 │
 ├── vector search
 │
 ├── metadata filter
 │
 └── graph/entity lookup
 │
 ▼
Candidate memories
 │
 ▼
Reranker
 │
 ▼
Top memories
```

---

# 30. Embeddings

Semantic retrieval needs embeddings.

A local embedding model can encode:

```text
memory
document chunk
conversation summary
task description
```

into vectors.

---

# 31. Local Embedding Models

Suitable families include:

```text
BGE
E5
Nomic Embed
Qwen embedding models
```

The exact model should be selected based on:

```text
embedding dimension
language support
latency
memory
license
quality
```

---

# 32. Vector Database

For the first version, use:

```text
SQLite + vector extension
```

or:

```text
PostgreSQL + pgvector
```

For a fully local desktop-first system, SQLite is attractive.

For a multi-device server-style deployment, PostgreSQL + pgvector becomes attractive.

---

# 33. Recommended Initial Storage

```text
SQLite
 ├── profile
 ├── memories
 ├── tasks
 ├── applications
 ├── preferences
 ├── entities
 └── metadata

Vector index
 └── embeddings
```

---

# 34. Why Not Use Only a Vector Database?

Vectors are bad at:

```text
exact equality
transactions
unique IDs
timestamps
state transitions
foreign keys
```

Use relational storage for authoritative structured data.

---

# 35. RAG

RAG means:

```text
Retrieval-Augmented Generation
```

JARVIS should retrieve relevant personal knowledge before asking the local LLM to answer.

---

# 36. Personal RAG Pipeline

```text
User request
     ↓
Query understanding
     ↓
Memory retrieval
     ↓
Document retrieval
     ↓
Task retrieval
     ↓
Reranking
     ↓
Context construction
     ↓
Local LLM
```

---

# 37. Context Builder

The context builder should produce:

```text
CURRENT TASK
USER PROFILE RELEVANT FIELDS
RELEVANT MEMORIES
RELEVANT DOCUMENTS
RECENT EVENTS
AVAILABLE TOOLS
POLICY
```

---

# 38. Context Budget

Do not inject hundreds of memories.

Use a budget.

Example:

```text
profile: 1–2 KB
memories: 4–8 KB
documents: task-dependent
task state: 2–4 KB
```

Exact values should be tuned to the local model's context window.

---

# 39. Memory Ranking

A useful score can combine:

```text
semantic similarity
+
importance
+
recency
+
confidence
+
entity relevance
+
task relevance
-
redundancy
```

Conceptually:

```text
score =
semantic
+ importance
+ recency
+ confidence
+ relevance
- redundancy
```

---

# 40. Recency

Recent information is often more relevant.

Use a decay function rather than a binary cutoff.

Example:

```text
recent event → high
old but important preference → still high
old irrelevant event → low
```

---

# 41. Importance Override

Important memories should not disappear merely because they are old.

Example:

```text
preferred job role
```

may be years old but still relevant.

---

# 42. Contradictory Memories

Suppose:

```text
Memory A:
User prefers remote work.

Memory B:
User now prefers hybrid work.
```

The latest explicitly confirmed preference should supersede the older preference.

Do not blindly provide both to the LLM.

---

# 43. Versioned Memory

Store:

```text
old value
new value
changed_at
reason/source
```

This allows auditability.

---

# 44. Preference State

Instead of:

```text
memory A
memory B
memory C
```

use:

```text
preference:
preferred_work_mode = hybrid
```

with history behind it.

---

# 45. Memory Consolidation

Periodically:

```text
recent memories
 ↓
cluster
 ↓
deduplicate
 ↓
summarize
 ↓
promote important information
 ↓
expire low-value information
```

---

# 46. Example Consolidation

Raw:

```text
User likes React.
User uses React.
User is working on React.
User prefers React for frontend.
```

Consolidate into:

```text
User frequently uses and prefers React for frontend development.
```

But preserve source references.

---

# 47. Memory Promotion

Possible flow:

```text
conversation
 ↓
short-term
 ↓
repeated evidence
 ↓
semantic/procedural memory
```

Repeated behavior can increase confidence but should not automatically become a permanent preference when ambiguity exists.

---

# 48. Forgetting

Memory needs forgetting.

Reasons:

```text
expiration
user request
superseded information
low importance
storage policy
privacy policy
```

---

# 49. User-Controlled Forgetting

Commands:

```text
"Forget this."

"Forget everything about this project."

"Forget my job applications."

"Delete all LinkedIn-related memories."

"Clear today's conversation memory."
```

The deletion mechanism must be actual deletion, not simply hiding records.

---

# 50. Tombstones

For synchronized systems, deletion may require a tombstone:

```json
{
  "memory_id": "mem123",
  "deleted_at": "...",
  "tombstone": true
}
```

This prevents an old device from resurrecting deleted data during sync.

---

# 51. Retention Policies

Different memory classes:

```text
working → minutes/hours
short-term → days
temporary observations → hours/days
episodic → configurable
semantic → long-lived
procedural → long-lived
application history → long-lived
```

---

# 52. Project Memory

Every major project should have its own memory namespace.

Example:

```text
project: JARVIS
```

could contain:

```text
architecture decisions
documents
TODOs
technology choices
known bugs
milestones
decisions
```

---

# 53. Project Namespace

```text
memory.scope = "project"
memory.scope_id = "jarvis"
```

This prevents unrelated memories from contaminating project reasoning.

---

# 54. Task Memory

Task memory is temporary but persistent across crashes.

Example:

```text
task:
apply to 3 jobs

completed:
job 1

pending:
job 2
job 3
```

---

# 55. Application Memory

Application history should be structured.

Fields:

```text
application_id
company
role
URL
job_id
source
date
status
resume_version
cover_letter_version
notes
```

---

# 56. Application Status

Possible states:

```text
DISCOVERED
SHORTLISTED
PREPARED
AWAITING_CONFIRMATION
SUBMITTED
REJECTED
INTERVIEW
OFFER
WITHDRAWN
```

---

# 57. Job Search Memory

Store:

```text
search criteria
previous searches
excluded companies
applied jobs
shortlisted jobs
saved jobs
```

---

# 58. Document Memory

The document system can expose:

```text
document metadata
document chunks
document embeddings
document summaries
```

The memory system should reference documents instead of duplicating them.

---

# 59. Resume Knowledge

Resume information should be structured where possible:

```text
education
experience
projects
skills
achievements
links
```

This allows reliable form filling.

---

# 60. Document Chunking

For long documents:

```text
document
 ↓
semantic sections
 ↓
chunks
 ↓
embeddings
```

Do not use arbitrary fixed-length chunks only.

Prefer section-aware chunking.

---

# 61. Document Metadata

Example:

```json
{
  "document_id": "doc123",
  "type": "resume",
  "version": "v3",
  "created_at": "...",
  "tags": [
    "software-engineering"
  ]
}
```

---

# 62. Knowledge Graph

A knowledge graph can represent relationships:

```text
User
 ├── works_on → JARVIS
 ├── uses → React
 ├── applied_to → Company A
 ├── prefers → Remote
 └── owns → Resume V3
```

---

# 63. Do We Need a Graph Database?

Not initially.

Start with relational tables:

```text
entities
relationships
```

Add a dedicated graph engine only if graph queries become a major requirement.

---

# 64. Entity Model

Entities can represent:

```text
Person
Company
Project
Job
Website
Application
Document
Device
Application
Software
Place
```

---

# 65. Relationship Model

```json
{
  "from": "user",
  "relation": "applied_to",
  "to": "job_123",
  "confidence": 1.0
}
```

---

# 66. Entity Resolution

Different text can refer to the same entity.

Example:

```text
OpenAI
openai.com
OpenAI company
```

Resolve them to one canonical entity.

---

# 67. Memory Scope

Every memory should have a scope where useful:

```text
global
project
task
website
device
application
conversation
```

---

# 68. Example

A LinkedIn field mapping:

```text
scope = linkedin
```

A JARVIS architecture decision:

```text
scope = project:jarvis
```

A temporary browser observation:

```text
scope = task:123
```

---

# 69. Device Memory

JARVIS should remember device capabilities.

Example:

```text
Windows PC
GPU available
microphone available
Chrome installed

Android phone
camera available
GPS available
battery state
```

Device memory must avoid unnecessarily storing precise location history.

---

# 70. Cross-Device Architecture

```text
             JARVIS Identity
                    │
          ┌─────────┼─────────┐
          ▼         ▼         ▼
       Windows    Ubuntu    Android
          │         │         │
          └─────────┼─────────┘
                    │
             Sync Service
```

---

# 71. Local-First Sync

The system should continue working if:

```text
internet unavailable
server unavailable
other device unavailable
```

Each device maintains a local copy of the information it needs.

---

# 72. Synchronization

Use:

```text
event-based synchronization
```

Example:

```text
memory.created
memory.updated
memory.deleted
preference.changed
application.created
```

---

# 73. Event ID

Every synchronization event should have:

```text
event_id
device_id
timestamp
entity_id
version
operation
```

---

# 74. Conflict Resolution

For simple preferences:

```text
latest confirmed update wins
```

For complex records:

```text
version comparison
field-level merge
manual conflict
```

---

# 75. Conflict Example

Windows:

```text
preferred_work_mode = remote
```

Android:

```text
preferred_work_mode = hybrid
```

If both were changed offline:

```text
detect conflict
 ↓
compare timestamps/version
 ↓
if ambiguous → ask user
```

---

# 76. Synchronization Security

Devices must authenticate.

Recommended:

```text
device identity
public/private key
mutual authentication
encrypted transport
```

Never synchronize memory over unauthenticated local network traffic.

---

# 77. Local Network

A future setup may allow:

```text
Windows JARVIS host
       ↕
Android companion
       ↕
Ubuntu JARVIS node
```

Communication can use a secure local RPC protocol.

---

# 78. Sensitive Memory Synchronization

Sensitive information should use stronger rules.

Example:

```text
normal preference → sync
phone → encrypted sync
password → secure credential system only
session token → device-specific
```

---

# 79. Memory Encryption

At rest:

```text
database encryption
```

At transport:

```text
TLS or mutually authenticated encrypted RPC
```

On Android:

```text
Android Keystore
```

On Windows:

```text
Windows credential/security facilities
```

On Linux:

```text
OS keyring/secret service
```

---

# 80. Backup

Memory backup should support:

```text
manual backup
scheduled backup
encrypted backup
export
restore
```

---

# 81. Backup Format

A logical backup can contain:

```text
profile
preferences
memories
entities
relationships
application history
project metadata
```

Exclude:

```text
raw passwords
session cookies
temporary credentials
```

unless handled by the dedicated secure credential backup system.

---

# 82. Restore

Restore should be transactional:

```text
validate backup
 ↓
decrypt
 ↓
verify integrity
 ↓
stage
 ↓
restore
 ↓
rebuild indexes
 ↓
verify
```

---

# 83. Memory API

High-level API:

```python
class MemoryService:

    async def remember(self, memory): ...
    async def retrieve(self, query): ...
    async def forget(self, query): ...
    async def update(self, memory_id, value): ...
    async def list(self, filters): ...
```

---

# 84. Profile API

```python
class ProfileService:

    async def get(self, field): ...
    async def set(self, field, value): ...
    async def update(self, fields): ...
    async def history(self, field): ...
```

---

# 85. Preference API

```python
class PreferenceService:

    async def get(self, key): ...
    async def set(self, key, value): ...
    async def reset(self, key): ...
```

---

# 86. Knowledge API

```python
class KnowledgeService:

    async def search(self, query): ...
    async def get_document(self, document_id): ...
    async def retrieve_chunks(self, query): ...
    async def related_entities(self, entity_id): ...
```

---

# 87. Memory Retrieval API

Example:

```python
results = await memory.retrieve(
    query="What resume should I use for SDE applications?",
    scope="job_applications",
    limit=5
)
```

Expected:

```text
software engineering resume v3
```

---

# 88. Memory Writer

The writer should classify candidates:

```text
fact
preference
procedure
event
project decision
task result
```

Then apply policy.

---

# 89. Memory Writer Pipeline

```text
input
 ↓
extract candidates
 ↓
classify
 ↓
sensitivity
 ↓
importance
 ↓
deduplicate
 ↓
conflict detection
 ↓
policy
 ↓
store
```

---

# 90. Memory Reader Pipeline

```text
query
 ↓
intent
 ↓
scope detection
 ↓
keyword retrieval
 ↓
vector retrieval
 ↓
entity retrieval
 ↓
merge
 ↓
rerank
 ↓
deduplicate
 ↓
context builder
```

---

# 91. Reranker

The reranker should consider:

```text
semantic relevance
scope
recency
importance
confidence
source reliability
```

---

# 92. Source Reliability

Suggested priority:

```text
explicit user statement
>
user profile
>
confirmed correction
>
task result
>
document
>
repeated behavior
>
LLM inference
```

---

# 93. Memory Hallucination Prevention

The LLM must never assume:

```text
missing memory = fact
```

If memory retrieval finds nothing:

> "I don't have that saved."

rather than inventing.

---

# 94. Memory Citations Internally

For important answers, context entries should preserve provenance:

```text
memory_id
source
created_at
```

This lets JARVIS explain:

> "You told me this last month."

without fabricating provenance.

---

# 95. Personal Knowledge RAG

Personal RAG sources:

```text
memory
documents
projects
applications
tasks
notes
calendar data
emails if connected
browser history if authorized
```

Every connector should have an explicit permission scope.

---

# 96. Email Memory

If an email connector is later added:

Do not ingest the entire mailbox into permanent memory.

Instead:

```text
email index
 ↓
search
 ↓
retrieve relevant messages
 ↓
temporary context
```

---

# 97. Calendar Memory

Calendar events should normally remain in the calendar system.

JARVIS can retrieve:

```text
today's schedule
meeting context
upcoming deadlines
```

rather than copying every event into memory.

---

# 98. Application Connectors

Same principle for:

```text
GitHub
Google Drive
Slack
Gmail
Calendar
Notion
```

Use external system as source of truth.

Memory stores:

```text
relevant summaries
decisions
references
```

---

# 99. Memory as Index, Not Garbage Dump

The memory system should answer:

> "What does JARVIS need to know right now?"

not:

> "How much information can we store?"

---

# 100. Conversation Memory

Conversation history should be divided:

```text
raw recent messages
conversation summary
important extracted memories
```

---

# 101. Conversation Summaries

Long conversations should periodically be summarized.

Example:

```text
Topic:
JARVIS architecture

Decisions:
- local-first
- Windows/Ubuntu/Android
- browser automation via Playwright
- Python core
```

---

# 102. Summary Updates

Do not regenerate an entire lifetime summary every turn.

Use incremental summaries.

---

# 103. Conversation IDs

Each conversation:

```text
conversation_id
```

Each message:

```text
message_id
conversation_id
timestamp
role
content
```

---

# 104. Conversation Retention

User should control:

```text
keep
delete
auto-delete after X days
don't retain
```

---

# 105. Private Conversation Mode

Support:

```text
"Private mode."
```

In private mode:

```text
do not create long-term memories
```

Temporary working memory can still operate for the task.

---

# 106. Memory Exclusion

User can say:

> "Don't remember this."

The system should tag the current content as:

```text
NO_LONG_TERM_MEMORY
```

---

# 107. Memory Editing UI

Desktop app should provide:

```text
Memory
├── Facts
├── Preferences
├── Projects
├── Applications
├── Procedures
└── Recently added
```

Each item:

```text
edit
forget
inspect source
```

---

# 108. Memory Search UI

User can search:

```text
"LinkedIn"
"resume"
"JARVIS"
"job preferences"
```

Results show:

```text
memory
source
date
confidence
```

---

# 109. Memory Audit

For important information:

```text
Where did you learn this?
```

JARVIS can respond:

> "You told me this during our previous discussion."

Only when the provenance actually exists.

---

# 110. Memory Security Boundary

The local LLM should not have unrestricted database access.

Instead:

```text
LLM
 ↓
Memory API
 ↓
Policy
 ↓
retrieval
```

---

# 111. Query Filtering

A memory query should include:

```text
scope
sensitivity ceiling
user authorization
task relevance
```

---

# 112. Tool Isolation

The browser agent should not be able to read:

```text
all personal memories
```

It should request only what it needs.

Example:

```text
browser.form_fill
requires:
profile.contact
profile.education
profile.experience
```

not:

```text
all memories
```

---

# 113. Least Privilege

Every subsystem gets minimal memory access.

Example:

```text
Music agent:
music preferences only

Job agent:
career profile + resume + application history

Browser:
task-specific context

System agent:
device information
```

---

# 114. Memory Permissions

Possible scopes:

```text
memory.read
memory.write
memory.update
memory.delete
profile.read
profile.write
document.read
```

---

# 115. High-Risk Memory Operations

Require explicit user confirmation:

```text
delete all memory
export memory
change identity data
change financial profile
share personal information
```

---

# 116. Memory Database Schema

Initial SQLite schema:

```sql
CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    content TEXT NOT NULL,
    source TEXT NOT NULL,
    scope TEXT,
    scope_id TEXT,
    confidence REAL,
    importance REAL,
    sensitivity TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    expires_at TEXT,
    last_accessed_at TEXT,
    access_count INTEGER DEFAULT 0
);
```

---

# 117. Memory Embeddings

Conceptually:

```sql
CREATE TABLE memory_embeddings (
    memory_id TEXT PRIMARY KEY,
    embedding BLOB NOT NULL,
    model TEXT NOT NULL,
    dimensions INTEGER NOT NULL,
    created_at TEXT NOT NULL
);
```

A real vector extension can replace the BLOB implementation.

---

# 118. Entities

```sql
CREATE TABLE entities (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    canonical_name TEXT NOT NULL,
    metadata_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

---

# 119. Relationships

```sql
CREATE TABLE relationships (
    id TEXT PRIMARY KEY,
    from_entity TEXT NOT NULL,
    relation TEXT NOT NULL,
    to_entity TEXT NOT NULL,
    confidence REAL,
    created_at TEXT NOT NULL
);
```

---

# 120. Preferences

```sql
CREATE TABLE preferences (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    source TEXT NOT NULL,
    confidence REAL NOT NULL,
    updated_at TEXT NOT NULL
);
```

---

# 121. Profile

Profile tables should be more structured.

Example:

```sql
CREATE TABLE profile_fields (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    sensitivity TEXT NOT NULL,
    source TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

For highly structured production data, dedicated tables can be introduced.

---

# 122. Application Table

```sql
CREATE TABLE applications (
    id TEXT PRIMARY KEY,
    company TEXT,
    role TEXT,
    source TEXT,
    url TEXT,
    job_id TEXT,
    status TEXT,
    resume_id TEXT,
    submitted_at TEXT,
    metadata_json TEXT
);
```

---

# 123. Tasks

```sql
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    goal TEXT NOT NULL,
    status TEXT NOT NULL,
    checkpoint_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

---

# 124. Memory Events

For audit and sync:

```sql
CREATE TABLE memory_events (
    event_id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    device_id TEXT NOT NULL,
    version INTEGER,
    created_at TEXT NOT NULL
);
```

---

# 125. Memory Indexes

Index:

```text
type
scope
scope_id
created_at
updated_at
importance
sensitivity
```

This improves structured retrieval.

---

# 126. Vector Search

If using SQLite vector support:

```text
memory embedding index
document embedding index
entity embedding index
```

Separate indexes allow different retrieval policies.

---

# 127. Embedding Versioning

Every vector should store:

```text
model name
model version
dimensions
```

If the embedding model changes:

```text
re-embed
```

---

# 128. Model Migration

Embedding migration:

```text
old index
 ↓
batch re-embedding
 ↓
new index
 ↓
validation
 ↓
switch
```

Do not mix incompatible vector spaces.

---

# 129. Memory Cache

Frequently used memories can be cached.

Examples:

```text
preferred resume
preferred browser
current project
preferred TTS voice
```

Use an in-memory cache with invalidation.

---

# 130. Cache Invalidation

When profile changes:

```text
profile update
 ↓
invalidate relevant caches
```

Never allow stale profile information to silently override the source of truth.

---

# 131. Memory API Security

Requests should carry:

```text
device
user
task
scope
requested sensitivity
```

Policy evaluates them.

---

# 132. Example Secure Request

```json
{
  "task_id": "job_123",
  "scope": "job_application",
  "requested_fields": [
    "name",
    "email",
    "phone",
    "education"
  ]
}
```

---

# 133. Memory Context Object

The context builder can generate:

```json
{
  "profile": {
    "preferred_role": "software_engineer"
  },
  "preferences": {
    "resume": "software_engineering_v3"
  },
  "recent_events": [],
  "relevant_memories": []
}
```

---

# 134. Avoid Prompt Pollution

Do not inject:

```text
unrelated memories
```

For example, while applying for a job, JARVIS does not need:

```text
favorite music
old weather query
unrelated project
```

---

# 135. Context Categories

Use:

```text
REQUIRED
RELEVANT
OPTIONAL
```

Only required/relevant context enters the model.

---

# 136. Memory Compression

When many memories are relevant:

```text
retrieve
 ↓
cluster
 ↓
summarize
 ↓
retain citations
```

---

# 137. Memory Retrieval Example

User:

> "Which resume should I upload for this SDE application?"

Retrieval:

```text
job role = software engineering
profile = software engineering
preference = latest software engineering resume
documents = resume v3
```

Answer:

> "Use your latest software-engineering resume, version 3."

---

# 138. Memory Retrieval Example 2

User:

> "Have I already applied here?"

Retrieval:

```text
company entity
+
job URL
+
application history
```

Result:

```text
submitted August 12
```

---

# 139. Memory Retrieval Example 3

User:

> "Why did we choose Playwright?"

Retrieval:

```text
project memory
architecture decision
Document 5
```

JARVIS can explain the actual stored decision.

---

# 140. Memory Learning

JARVIS should learn from corrections.

User:

> "No, I don't want Java jobs. I'm targeting JavaScript/TypeScript roles."

Candidate preference:

```text
exclude = Java-only roles
prefer = JavaScript/TypeScript
```

Because this changes future behavior, it is a strong memory candidate.

---

# 141. Learning Threshold

Repeated behavior can create candidates, but stable preferences should ideally be confirmed.

Example:

```text
Observed:
user rejects 5 remote jobs

Candidate:
user may prefer onsite/hybrid
```

Ask before permanently changing profile.

---

# 142. Behavioral Memory

Behavioral observations can be stored as:

```text
observed pattern
```

not necessarily:

```text
permanent preference
```

---

# 143. Memory Confidence Update

Evidence can update confidence:

```text
explicit confirmation → strong
single observation → weak
repeated consistent observations → stronger
contradiction → lower
```

---

# 144. Memory Decay

For uncertain observations:

```text
confidence decreases over time
```

For explicit preferences:

```text
no automatic decay
```

unless superseded.

---

# 145. Memory Consolidation Job

A scheduled local process can:

```text
find duplicates
find contradictions
find expired memories
summarize conversations
rebuild embeddings
```

This can run during idle periods.

---

# 146. Idle Processing

Do expensive tasks when:

```text
CPU idle
GPU idle
device charging
```

especially on Android.

---

# 147. Android Memory

Android should not necessarily maintain the entire memory database.

Recommended:

```text
local cache
+
synchronized subset
```

The primary JARVIS host can maintain the full local knowledge base.

---

# 148. Android Offline Mode

If disconnected:

```text
local profile subset
recent conversations
recent tasks
device-specific memory
```

remain available.

Changes synchronize later.

---

# 149. Windows Primary Host

A Windows PC can serve as:

```text
primary JARVIS memory node
```

when it is the main machine.

Ubuntu can similarly become primary.

The architecture should not hard-code Windows as the permanent master.

---

# 150. Primary Node

A primary node can provide:

```text
full memory
LLM
vector search
document RAG
task persistence
```

Other devices act as:

```text
clients
workers
specialized nodes
```

---

# 151. Distributed JARVIS

Eventually:

```text
Windows GPU node
     │
     ├── LLM
     ├── Vision
     ├── Memory
     └── Browser
          ↕
      Android
          ↕
       Ubuntu
```

---

# 152. Memory Routing

If Android asks:

> "What resume should I use?"

Android can query the primary memory node.

If offline:

```text
local cache
```

---

# 153. Memory Sync Frequency

For normal preferences:

```text
near real-time
```

For large document embeddings:

```text
batch synchronization
```

---

# 154. Sync Optimization

Do not synchronize entire databases.

Use:

```text
events
changes
content hashes
version vectors
```

---

# 155. Content Hash

For large objects:

```text
SHA-256 content hash
```

can determine whether data changed.

---

# 156. Document Sync

Documents can be synchronized separately:

```text
metadata
 ↓
content hash
 ↓
transfer only if missing
```

---

# 157. Privacy Modes

Recommended:

```text
Normal
Private
Sensitive
Offline-only
```

---

# 158. Offline-Only Memory

User can mark:

```text
memory.scope = local_device
```

It must never synchronize.

---

# 159. Local-Only Project

The JARVIS project itself could have:

```text
sync = enabled
```

while sensitive personal data can remain:

```text
sync = disabled
```

---

# 160. Memory Export

Support:

```text
export profile
export preferences
export selected memories
export project memory
```

Formats:

```text
JSON
Markdown
CSV where appropriate
```

---

# 161. Human-Readable Memory

A useful export:

```markdown
# JARVIS Memory Export

## Preferences

- Preferred role: Software Engineer
- Preferred work mode: Hybrid

## Projects

### JARVIS

- Local-first architecture
- Windows/Ubuntu/Android
```

---

# 162. Memory Import

Import should validate:

```text
schema
version
signature
integrity
```

before writing.

---

# 163. Schema Versioning

Every database export should have:

```text
schema_version
```

Migrations should be explicit.

---

# 164. Memory Testing

Tests should include:

```text
create
retrieve
update
delete
expire
conflict
deduplicate
rank
sync
restore
```

---

# 165. Retrieval Evaluation

Create benchmark queries:

```text
"What resume should I use?"
"Have I applied here?"
"What role am I targeting?"
"What did we decide about Playwright?"
```

Measure:

```text
precision
recall
latency
irrelevant context rate
```

---

# 166. Memory Hallucination Test

Ask questions about information that does not exist.

Expected:

```text
"I don't have that information."
```

not a fabricated answer.

---

# 167. Privacy Test

Verify:

```text
password never appears in memory
OTP never appears
session cookie never appears
secret tokens never appear
```

---

# 168. Deletion Test

Create memory:

```text
test preference
```

Delete it.

Then verify:

```text
database record gone
vector gone
cache invalidated
sync tombstone created if needed
```

---

# 169. Cross-Device Test

```text
Windows creates preference
 ↓
Android syncs
 ↓
Android modifies
 ↓
Ubuntu syncs
 ↓
Windows receives update
```

Test offline conflicts.

---

# 170. Performance Targets

Initial targets:

```text
structured memory lookup: <50 ms
local keyword search: <100 ms
vector retrieval: <300 ms
hybrid retrieval: <500 ms
context assembly: <100 ms
```

Exact performance depends on hardware and database size.

---

# 171. Memory Size

A personal assistant does not necessarily need millions of permanent memories.

The system should optimize for:

```text
high-value memories
```

rather than maximum storage.

---

# 172. Expected Initial Scale

A first implementation may contain:

```text
thousands of memory records
thousands of document chunks
hundreds of entities
thousands of application/task records
```

SQLite can comfortably handle this scale with good indexing.

---

# 173. Scaling Later

If JARVIS becomes a multi-user/server system:

```text
SQLite
 ↓
PostgreSQL
 ↓
pgvector
```

But this should not be the first requirement.

---

# 174. Recommended Initial Stack

## Database

```text
SQLite
```

## Vector Search

```text
SQLite vector extension
```

or equivalent local vector index.

## Embeddings

```text
local BGE / E5 / Nomic / Qwen embedding model
```

## ORM/Database Layer

```text
SQLAlchemy
```

or a lightweight repository layer.

## Serialization

```text
Pydantic
```

## Encryption

```text
OS keyring
```

plus application-level encryption where appropriate.

---

# 175. Memory Service Language

Use:

```text
Python
```

because the core JARVIS architecture already uses Python for:

```text
LLM
RAG
agents
browser
orchestration
```

---

# 176. Memory Service Process

A dedicated process is recommended:

```text
jarvis-memory
```

The Agent Core communicates with it through local RPC.

This prevents every subsystem from directly opening the database.

---

# 177. Memory RPC

Example:

```text
memory.retrieve
memory.remember
memory.update
memory.forget
profile.get
profile.set
knowledge.search
```

---

# 178. Memory Event Bus

Events:

```text
memory.created
memory.updated
memory.deleted
profile.updated
preference.changed
entity.created
application.created
project.updated
```

---

# 179. Memory and Voice

User:

> "Remember that I want remote SDE roles."

Voice pipeline:

```text
Whisper
 ↓
intent
 ↓
Memory Candidate
 ↓
policy
 ↓
store
 ↓
TTS
```

Response:

> "I'll remember that for future job searches."

---

# 180. Memory and Browser

Browser Agent:

```text
"What resume should I upload?"
```

Memory:

```text
profile
+
document metadata
+
preference
```

returns:

```text
software-engineering-v3.pdf
```

---

# 181. Memory and Agent Planner

Planner receives:

```text
goal
+
relevant memory
+
available tools
```

Example:

```text
Goal:
Apply to SDE jobs.

Relevant memory:
- preferred role
- target locations
- resume
- application history
- excluded companies
```

---

# 182. Memory and OS Automation

OS agent may need:

```text
preferred browser
preferred terminal
preferred music app
device capabilities
```

It should retrieve only those fields.

---

# 183. Memory and Documents

Document agent provides:

```text
document search
document metadata
document chunks
```

Memory stores:

```text
which document is preferred
```

---

# 184. Memory and Scheduler

Scheduler can use:

```text
user routines
task preferences
notification preferences
```

Example:

```text
"Every morning, remind me to review job applications."
```

The scheduler stores the recurring task.

Memory stores the preference if relevant.

---

# 185. Memory and Model Routing

JARVIS can use memory to select models.

Example:

```text
simple command
→ small local model

complex research
→ larger local model

vision task
→ vision model
```

Model selection itself can be a procedural preference.

---

# 186. Memory and Personalization

The LLM should not always speak identically.

Memory can provide:

```text
preferred name
voice preference
response verbosity
language
technical depth
```

Only retrieve these when needed.

---

# 187. Avoid Over-Personalization

The system should not inject personal information merely to sound personalized.

Memory is primarily for:

```text
correctness
continuity
automation
```

not artificial familiarity.

---

# 188. Memory Safety Rule

If information is uncertain:

```text
ask
```

If information is sensitive:

```text
protect
```

If information is stale:

```text
verify
```

If information is contradictory:

```text
resolve
```

If information is irrelevant:

```text
do not retrieve
```

---

# 189. Memory Lifecycle

Every memory follows:

```text
DISCOVER
   ↓
CLASSIFY
   ↓
VALIDATE
   ↓
STORE
   ↓
RETRIEVE
   ↓
USE
   ↓
UPDATE / CONSOLIDATE
   ↓
EXPIRE / DELETE
```

---

# 190. Final Memory Architecture

```text
                         JARVIS
                           │
                           ▼
                      AGENT CORE
                           │
                    ┌──────┴──────┐
                    ▼             ▼
              Memory Writer   Memory Reader
                    │             │
                    ▼             ▼
              Memory Policy   Hybrid Retrieval
                    │             │
                    ▼             ▼
                SQLite       Vector Index
                    │             │
         ┌──────────┼─────────────┤
         │          │             │
         ▼          ▼             ▼
      Profile    Memories      Knowledge
         │          │             │
         │          │        Documents/RAG
         │          │
         └──────────┼─────────────┘
                    ▼
              Context Builder
                    │
                    ▼
                 Local LLM
```

---

# 191. Final Cross-Device Architecture

```text
                    Secure Sync
                         │
          ┌──────────────┼──────────────┐
          ▼              ▼              ▼
       Windows         Ubuntu         Android
          │              │              │
       Full DB        Full DB       Local Cache
          │              │              │
          └──────────────┼──────────────┘
                         │
                    Event Log
```

---

# 192. Recommended Development Order

## Step 1

Build:

```text
SQLite schema
```

## Step 2

Build:

```text
Profile service
```

## Step 3

Build:

```text
Memory CRUD
```

## Step 4

Add:

```text
keyword search
```

## Step 5

Add:

```text
embeddings
```

## Step 6

Add:

```text
vector search
```

## Step 7

Add:

```text
hybrid retrieval
```

## Step 8

Add:

```text
memory writer
```

## Step 9

Add:

```text
memory consolidation
```

## Step 10

Add:

```text
project/task/application memory
```

## Step 11

Add:

```text
encrypted synchronization
```

## Step 12

Add:

```text
memory UI
```

---

# 193. First Practical Memory Demo

User:

> "Remember that I am targeting full-stack software engineering roles."

JARVIS:

```text
extract preference
 ↓
validate
 ↓
store
```

Later:

> "Find jobs for me."

JARVIS retrieves:

```text
target role = full-stack software engineer
```

and automatically incorporates it into the job search.

---

# 194. Second Demo

User:

> "Use my latest software-engineering resume when applying."

Store:

```text
job_application.resume_preference
```

Later:

```text
Browser Agent
 ↓
Memory
 ↓
Document Service
 ↓
latest matching resume
 ↓
upload
```

---

# 195. Third Demo

User:

> "Have I already applied to this company?"

Browser:

```text
company = Example
```

Memory/Application Store:

```text
previous applications
```

JARVIS answers using actual history.

---

# 196. Fourth Demo

User:

> "What did we decide about the JARVIS browser architecture?"

Retrieve:

```text
project:jarvis
architecture decisions
Document 5
```

Answer from stored evidence.

---

# 197. Fifth Demo

User:

> "Forget everything about that company."

JARVIS:

```text
resolve company entity
 ↓
find associated memories
 ↓
delete
 ↓
delete vector records
 ↓
invalidate cache
 ↓
create sync tombstones
```

---

# 198. What This Enables

Once Documents 1–6 are implemented together, JARVIS can move from:

```text
voice chatbot
```

to:

```text
persistent personal operating agent
```

because it can:

```text
understand
remember
reason
operate
verify
learn preferences
maintain state
resume tasks
```

---

# 199. Critical Design Decisions

The implementation should follow these rules:

1. **Structured profile data is authoritative.**
2. **Passwords never belong in ordinary memory.**
3. **The LLM cannot arbitrarily write permanent memory.**
4. **Explicit user statements outrank inference.**
5. **Recent observations do not automatically become permanent facts.**
6. **Use relational storage for state.**
7. **Use vector search for semantic retrieval.**
8. **Use hybrid retrieval rather than vector-only retrieval.**
9. **Use scopes to prevent irrelevant context.**
10. **Keep provenance for important memories.**
11. **Support actual deletion.**
12. **Support offline operation.**
13. **Synchronize through authenticated encrypted channels.**
14. **Use least privilege for subsystem access.**
15. **Never allow webpage content to override memory or system policy.**

---

# 200. Final Goal

The final JARVIS memory system should make the assistant feel continuous without becoming uncontrolled.

The ideal interaction is:

```text
User:
"Find me some good SDE jobs."

JARVIS:
"I'll use your saved software-engineering profile,
your preferred locations, your application history,
and your preferred resume."

       ↓

Memory Retrieval
       ↓
Job Agent
       ↓
Browser Agent
       ↓
Application Workflow
       ↓
Memory Update

       ↓

JARVIS:
"I found eight new matches. Three are strong matches
and none duplicate your previous applications."
```

That is the target:

> **JARVIS should remember what makes future actions more accurate, while giving the user complete control over what is remembered.**
