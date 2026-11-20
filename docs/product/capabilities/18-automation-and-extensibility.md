# Automation and extensibility

This area specifies how ArchBuilder is driven without the graphical interface and extended by users:
the command registry, the automation protocol, the headless CLI, scripting, node graphs, plug-ins,
model checking rules and the optional assistant. All of them run the same typed commands as the
interface.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Command | Stable ID, label, parameter schema, enabled predicate, run, prompt machine | Registered in the command registry |
| Node graph | Nodes, links, exposed inputs, outputs | Stored in the document; runs commands |
| Plug-in package | Manifest, WebAssembly modules, requested capabilities, version | Installed per user; trust assigned by the host |
| Rule set | Rules (IDS specifications, parameter rules, naming rules, geometric rules), severity | Run in the application and headless |
| Check result | Rule, element, outcome, message | Shown as warnings or report rows |
| Event rule | Trigger filter, action (node graph or plug-in), enabled flag | Runs inside the triggering transaction |

## Commands and protocol

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-AUTO-001 | Command registry | Registers every model-changing action as a command with a machine-readable parameter schema, reachable from menus, toolbars, the command palette and the command line with the same parameters. | T1 | 2 |
| AB-AUTO-002 | Automation protocol | Serves the Arc Automation Protocol over stdio or a local socket: queries, command execution with preview, change subscriptions and document lifecycle. Programmatic calls never open a dialog. | T1 | 2 |
| AB-AUTO-003 | Headless CLI | Converts, validates, checks, publishes, compares, runs scripts and commands, and serves the automation protocol, without a window. Exit status reflects success; reports are written as JSON. | T1 | 2 |
| AB-AUTO-004 | Python client | Runs Python scripts out of process through the official protocol client, with typed wrappers generated from the command schemas. | T1 | 3 |
| AB-AUTO-005 | Macro recording | Records commands issued in the interface as a script that replays them through the protocol. | T2 | 3 |
| AB-AUTO-006 | Model Context Protocol endpoint | Exposes queries and commands, derived from the registry, through a Model Context Protocol endpoint with the same permission and preview rules as the protocol. | T2 | 3 |
| AB-AUTO-007 | Command journal | Logs commands of a session with parameters and results for diagnosis; a journal replays against the same starting document. | T2 | 2 |
| AB-AUTO-008 | Shortcuts and interface customization | Customizes keyboard shortcuts, toolbars and the command palette per user, exportable and importable as files. | T1 | 2 |

## Graphs, plug-ins and rules

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-AUTO-009 | Node graphs | Edits and runs visual graphs that query the model, compute geometry and data, and apply results through commands. Graphs are stored in the document; a graph player runs a graph with its exposed inputs for users who do not edit graphs. | T2 | 3 |
| AB-AUTO-010 | Design studies | Runs a node graph over ranges of its inputs against stated goals and constraints, presents results for filtering and ranking, and stores a chosen result as a design option. | T3 | 4 |
| AB-AUTO-011 | Sandboxed plug-ins | Loads WebAssembly plug-ins that have no ambient authority, declare the capabilities they need, run within memory and time limits, and change the document only through commands applied as one transaction. | T2 | 3 |
| AB-AUTO-012 | Event rules | Runs node graphs or plug-in actions when elements matching a filter change, inside the triggering transaction and within execution limits. Rules can be disabled per document, and their changes are labeled in history. | T3 | 3 |
| AB-AUTO-013 | Model checking rules | Runs rule sets (IDS specifications, parameter completeness, naming patterns, geometric rules such as minimum clearances) on demand, on save or before export; results appear as warnings, as a report and as BCF issues. | T1 | 3 |
| AB-AUTO-014 | Model checking in CI | Runs rule sets from the CLI against documents in a repository or Hub and returns a nonzero exit status on failures, with JSON and JUnit-style reports. | T2 | 3 |
| AB-AUTO-015 | Assistant panel | Lets a user-configured assistant provider propose commands through the automation protocol; proposals show a preview and apply only on explicit confirmation, and applied changes are labeled in history. No feature requires an assistant. The assistance capabilities are in [32-assistance.md](32-assistance.md). | T3 | 3 |
