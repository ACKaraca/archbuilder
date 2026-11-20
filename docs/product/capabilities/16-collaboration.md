# Collaboration

This area specifies working together on ArchBuilder documents through an optional Arc Hub: file-level
versions, locks, comparison, packages, branches, and element-level multi-user editing. Every
capability here is additive; a document without a Hub connection keeps every other capability.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Hub binding | Hub URL, project, file, local cache state | Associates a local document with a Hub file |
| Version | Number, author, comment, timestamp, content hash, parent versions | Stored in Hub; addressable read-only |
| Lifecycle state | Work in progress, shared, published, archived | Per Hub file version |
| Change set | Transactions from the operation journal, base version, author | Unit of exchange in multi-user editing |
| Branch | Name, base version, head version | Line of versions in Hub |
| Element ownership | Element, owner, borrow state | Multi-user editing session state |
| Work area | Name, members or rule, default owner | Partition for loading, visibility and ownership |
| Comparison result | Added, removed and modified elements with changed parameters and geometry | Derived from two document states |
| Checkpoint | Name, document revision, timestamp, content hash | Local to a document; restorable only as a new document |
| Document archive | Manifest with checksums, document, linked documents and files, underlays | Portable transfer of a document with its sources |

## File-level collaboration

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-COLLAB-001 | Hub-connected documents | Opens and saves documents in an Arc Hub project as versions with comments, keeps a local cache, and continues to work offline; queued versions upload when the connection returns. | T1 | 3 |
| AB-COLLAB-002 | Check-out locks and states | Locks a Hub file for editing by one user and moves versions through lifecycle states (work in progress, shared, published, archived). A locked file opens read-only for others with the owner shown. | T1 | 3 |
| AB-COLLAB-003 | Version history | Lists versions with author, comment and date, opens any version read-only, and restores an older version as a new version. | T1 | 3 |
| AB-COLLAB-004 | Model comparison | Compares two versions or two documents element by element and reports added, removed and modified elements with changed parameters and geometry, in a table and as color-coded views. | T1 | 3 |
| AB-COLLAB-005 | Canonical text form | Writes a document's semantic content as a canonical text form (one record per element, stable ordering, stable formatting) from the command line, so general-purpose diff and review tools show meaningful differences. | T3 | 3 |
| AB-COLLAB-006 | Exchange packages | Shares a frozen set of document versions and sheets with another team as a package; the receiving team links the package's documents and is notified of newer packages. | T2 | 3 |
| AB-COLLAB-017 | Local checkpoints | Saves named checkpoints of a document locally without Arc Hub, kept until the user deletes them. A checkpoint opens read-only, compares with the current state (AB-COLLAB-004) and restores only as a new document, never by overwriting the current one. | T2 | 3 |
| AB-COLLAB-018 | Document archive | Writes one file holding a document, its linked documents and files, underlays and a manifest with checksums, after listing any source that is missing; opening the archive restores the links with relative paths and verifies every checksum. | T2 | 3 |

## Branches and multi-user editing

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-COLLAB-007 | Branches and merge | Creates named branches of a Hub file and merges them with an element-level three-way merge. Conflicting edits to the same element are never resolved silently: both states are kept and the user resolves each conflict, and the resolution is recorded. | T3 | 5 |
| AB-COLLAB-008 | Multi-user sessions | Lets several users edit one Hub document at the same time; each user's transactions travel as change sets from the operation journal. | T2 | 5 |
| AB-COLLAB-009 | Element borrowing | Borrows elements automatically when a user edits them; elements borrowed by others are read-only with the owner shown, and the user can request them. Borrowed elements are released on synchronization unless kept. | T2 | 5 |
| AB-COLLAB-010 | Work areas | Partitions a document into named work areas used for partial loading, visibility and default ownership. | T2 | 5 |
| AB-COLLAB-011 | Partial loading | Opens only chosen work areas, buildings or storeys of a shared document, loading the rest on demand. | T2 | 5 |
| AB-COLLAB-012 | Synchronization | Sends local change sets and receives others' in one operation. Unsynchronized own transactions stay undoable; synchronized transactions are reverted by new compensating transactions. | T2 | 5 |
| AB-COLLAB-013 | Conflict preservation | Keeps both versions of an element when change sets conflict and shows them for comparison; no work is discarded by synchronization. | T2 | 5 |
| AB-COLLAB-014 | Ownership display | Colors elements in views by owner, borrow state, work area or recent change. | T2 | 5 |
| AB-COLLAB-015 | Element authorship | Records creator, last editor and last changing version per element, readable in properties and schedules. | T2 | 5 |
| AB-COLLAB-016 | Standalone copy | Saves a shared document as a standalone document, keeping or discarding work areas, for archiving or handover. | T2 | 5 |
