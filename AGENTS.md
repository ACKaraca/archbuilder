# AGENTS.md

Instructions for automated coding agents working in this repository.

## What this repository is

ArchBuilder: Arc suite BIM authoring for architecture, structure and building services, in Rust.
One semantic model per `.archb` document; views, schedules, sheets and exports are derived from it.
Depends only on `arc-core` and third-party crates; never on other Arc products.

## Status and where to start

The spatial structure (project, site, building, storey, level) exists as code, with a windowed
application and a headless CLI on one pinned `arc-core`. Built so far: walls (create, delete, flip,
layer-cleaned joins, wrapping, `snap.rs`), doors and windows hosted by walls, floors
(walls or sketch, holes, arcs, boundary sketch editing; edges follow wall faces), rooms
(`archbuilder.room.place`, `place_all`: derived from walls around a point, warnings), stairs (straight, L, U; floor openings, railings), `archbuilder.modify.*` (move, copy, rotate, mirror, align, offset, trim, arrays), wall split and merge, pin, a warnings panel, direct manipulation (drag, handles, temporary dimensions), everyday habits (Tab, context menu, clipboard, typed shortcuts, hide and isolate),
Delete, `archbuilder.property.set`, outliner, property grid, `inspect` lists, IFC4 export (rooms
as spaces), element types (layered wall and floor types, door and window types; type editor,
create similar, match type), roofs with wall attach, ceilings, wall, shaft and vertical openings, room solids. Families and views are still specifications.
Read in this order:
`docs/product/vision.md`,
`docs/product/capabilities/README.md`, `docs/architecture/overview.md`,
`docs/architecture/domain-model.md`, `docs/roadmap.md`, `docs/adr/README.md`.

## Repository layout

Present:

```
crates/archbuilder-model/     spatial, wall, insert, floor, roof, room and element types, picking order, validation rules, solids, wall joins, readers (headless)
crates/archbuilder-commands/  type set and the commands (headless)
crates/archbuilder-io/        IFC4 export of walls, doors, windows, floors, roofs, ceilings, openings, rooms with a loss report; `archbuilder.file.export_ifc` (headless)
apps/archbuilder/             desktop application: arc-app shell, panels, extractors, tools
apps/archbuilder-cli/         headless CLI: the shared subcommands of the suite, `inspect` with wall, door, window and floor lists, `export-ifc`
xtask/                        layers, catalogs, ci
locales/en-US/                message catalogs, one per crate that has messages
locales/tr-TR/                Turkish catalogs of the same crates
```

Planned, not created yet (`archbuilder-io` will also map DWG/DXF, PDF, meshes, BCF and more):

```
crates/archbuilder-ui/        tools, panels, viewport interaction (egui); until it exists the panels
                              are in apps/archbuilder
```

The foundation is `arc-core`, pinned by revision in the workspace `Cargo.toml` (declared once in
`[workspace.dependencies]`, never as a path). Toolchain, lint table and `deny.toml` follow
`arc-core`; the layer table is `xtask/layers.toml`. Documents use the extension `.archb`.

## Commands

The toolchain is pinned in `rust-toolchain.toml`; the gate also needs `cargo-nextest`,
`cargo-deny` and `cargo-audit`.

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo nextest run --workspace --all-features` (`cargo test --workspace --doc` for documentation tests)
- `cargo xtask layers` and `cargo xtask catalogs` (single gates of `ci`)
- `cargo xtask ci` runs all gates, including `cargo deny check` and `cargo audit`. CI runs it when
  started by hand, once per batch of changes: on Windows by default, on Linux, Windows and macOS on
  request.
- `cargo run -p archbuilder` starts the application; `cargo run -p archbuilder-cli -- help` lists
  the CLI subcommands.

## Suite rules

These rules apply in every Arc repository. The canonical text is
[`docs/engineering/agent-rules.md`](https://github.com/ACKaraca/arc-suite/blob/main/docs/engineering/agent-rules.md)
in `arc-suite`; change it there first.

1. **A person is accountable.** Whoever runs an agent answers for what it submits.
2. **Start from an issue.** Search issues and pull requests, check that nobody else is assigned,
   assign yourself and set the status to *In Progress* before changing files. When you finish,
   comment with what changed, the commit or pull request link and open items, then update the status.
   *Done* requires met and verified acceptance criteria.
3. **Verify, do not guess.** Open a definition before using it. Confirm a cause in the code.
4. **The document is the authority and everything is a command.** No change reaches a document
   except through a validated command inside a transaction. Programmatic paths never open dialogs.
5. **Respect the layers.** A crate depends only on lower layers. Nothing below the UI layer depends
   on windowing, GPU or UI crates. Products never depend on other products.
6. **Never crash, never lose work.** No `unwrap`, `expect`, `panic!`, `todo!` or `unreachable!` in
   production code. No `unsafe` outside the FFI crates named in an ADR. Treat every file and message
   as hostile: bound recursion, allocation and counts. A crash fix ships with a regression test.
7. **Clean room.** Implement from published specifications, observed behavior and original
   reasoning. Never copy or consult another product's code, decompiled output, file-format reverse
   engineering, documentation text, icons, fonts, templates or sample files. Do not name our
   features after another company's features.
8. **Licenses.** Add a dependency only if its license is on the allowlist in
   `docs/engineering/dependency-policy.md` in `arc-suite`. Record every asset's origin and license.
9. **Rust only in desktop applications.** No web view, no JavaScript or TypeScript.
10. **Small, tested changes.** One concern per pull request, with the test that fails without it.
    Update the documents that describe the behavior you change in the same pull request.
11. **Report only what ran.** Say what you tested and what you could not run.
12. **Ask a human before** adding a license outside the allowlist, publishing or releasing,
    changing a file format or public protocol, adding a new repository or remote, spending money,
    handling secrets, or any destructive git operation. Otherwise keep working.
13. **Keep secrets and personal data out** of repositories, logs and requests.
14. **The maintainer commits directly to `main` for now.** Until the project has a second maintainer
    and this repository has reached its MVP release (for `arc-suite` and `arc-core`: the first
    product MVP), the maintainer works on `main` without branches or pull requests; for that work,
    read "commit" wherever the documents say "pull request". Other contributors work on a branch and
    open a pull request, which the maintainer reviews and merges. Run the checks before every push
    and name the issue in the commit message. This rule is temporary and is deleted from a
    repository when that point is reached.

## Repository invariants

- Records store authored data only; geometry, joins, room areas, view drawings, schedule rows and
  quantities are derived and never stored as authority.
- Every derived result carries its source revision; a stale result is never applied or published.
- Three editing contexts: model views edit the model; view detail layers never change the model;
  detached drawings never write back. A command batch never mixes model and drafting targets.
- Families are declarative. Family computation uses only the bounded expression language; no
  family carries executable code.
- Element identifiers never change; generated children get deterministic identifiers from parent
  and child key. IFC GlobalIds derive from identifiers.
- Persistent references and annotation anchors are never rebound by proximity; unresolved means
  orphaned and reported.
- Exports never fall back to proxies silently; every import and export produces a loss report;
  imports keep unmodeled content as preserved foreign data.
- One quantity-rule implementation feeds schedules, takeoffs and IFC quantity sets.
- Every new element kind ships with its IFC mapping, fixtures and undo tests.

## Documentation rules

- Never mention internal planning material, research or how documents were produced.
- Other companies' names appear only as formats, standards and protocols, in factual
  interoperability statements, or in the single positioning paragraph of `docs/product/vision.md`.
- Use our own vocabulary; never name a feature after another product's feature.
- Each fact lives in one place; link suite rules in `arc-suite` instead of copying them.
- Write behavior descriptions in original words; do not reproduce other products' documentation,
  dialog text or option lists.
- Capability identifiers (`AB-<AREA>-<NNN>`) are permanent; status lives in issues, not tables.

## Issue workflow

Search issues and pull requests first; confirm nobody is assigned; assign yourself and set
*In Progress*. Statuses: Backlog, Ready, In Progress, In Review, Blocked, Done (*Done* only with
met and verified acceptance criteria). On finishing, comment with what changed, the commit or pull
request link and open items, update the status, and open follow-up issues after a duplicate search.
Labels: `type:`, `tier:`, `phase:`, `area: <token>` (area tokens are listed in
`docs/product/capabilities/README.md`). Conventional Commits with DCO sign-off.

## Verification

Run the narrowest relevant check first (one crate's tests, one fixture, one exporter's independent
validation), broaden only when it fails for a relevant reason or shared code changed. Report only
what ran and what could not be run.

## Ask a human about

- Changes to the `.archb` or `.archbfam` formats, the expression language function set, or the
  canonical text form.
- Changes to IFC default mappings or to the meaning of a capability row.
- New crates, new dependencies outside the allowlist, or anything listed under suite rule 12.
- Conflicts between this repository's documents and suite documents.
