# Sheets and publishing

This area specifies sheets, viewports, title blocks, revisions, sheet sets, and publishing to PDF,
DWG, DXF, images, printers and Arc Hub. Sheets use the same sheet and plot pipeline as DraftBuilder.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Sheet | Number, name, size, title block type, sheet parameters, revisions shown | Owns viewports and schedule placements |
| Viewport | Source (model view, drafting view, detached drawing, legend, schedule), paper position, rotation, type, detail number | Belongs to one sheet |
| Revision | Sequence number, label, date, description, issued flag, numbering scheme | Referenced by clouds and sheets |
| Sheet set | Name, ordered sheets or rule | Input to publishing |
| Publish profile | Format, options, layer mapping, file naming | Used by publish commands |
| Publication record | Document revision, sheets, files, checksums | Produced by every publish |

## Sheets and viewports

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-SHEET-001 | Sheets | Creates sheets from title block types in ISO A, ANSI or custom sizes, with number and name. Title block labels read project and sheet parameters live. | T1 | 2 |
| AB-SHEET-002 | Viewports | Places views, detached drawings and drafting views on sheets at their own scale, with viewport types (with or without title), title line extent and rotation. A model view appears on one sheet only; legends and schedules may appear on many. | T1 | 2 |
| AB-SHEET-003 | Viewport alignment | Aligns viewports across sheets by view origin, by a shared datum or to a guide grid, so plans of different levels sit at the same paper position. | T2 | 2 |
| AB-SHEET-004 | Editing through viewports | Activates a view inside its viewport to edit the model or detail layer in place, with the rest of the sheet shown halftoned. | T1 | 2 |
| AB-SHEET-005 | Detail numbering | Numbers viewports automatically per sheet, keeps numbers unique, and updates every marker that references the viewport when it moves. | T1 | 2 |
| AB-SHEET-006 | Schedules on sheets | Places schedules on sheets, splits long schedules into columns or across sheets, and repeats headers on each part. | T1 | 2 |
| AB-SHEET-007 | Bulk sheet creation | Creates many sheets from a table of numbers and names, including placeholder sheets that appear in sheet lists but are not published. | T2 | 2 |

## Revisions and sets

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-SHEET-008 | Revisions | Maintains a revision sequence with numeric, alphabetic or custom numbering, per project or per sheet. Issuing a revision locks it; title block revision tables are built from clouds on each sheet plus manual assignments. | T1 | 2 |
| AB-SHEET-009 | Sheet sets | Defines named, ordered sheet sets explicitly or by rule (parameter values), used by publishing and by sheet lists. | T1 | 2 |

## Publishing

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-SHEET-010 | Revision-consistent publishing | Publishes every viewport of a set from one document revision; publishing waits for pending view regeneration and never mixes results of different revisions. Each publish writes a publication record. | T1 | 2 |
| AB-SHEET-011 | PDF publishing | Writes vector PDF at true scale with line weights, embedded fonts, searchable text, bookmarks per sheet and links from view markers to referenced sheets, as one file or one file per sheet. | T1 | 2 |
| AB-SHEET-012 | DWG and DXF publishing | Writes sheets and views to DWG or DXF with layer mapping tables, views as external references or bound into the sheet, and a chosen coordinate basis (internal, project or survey). | T1 | 2 |
| AB-SHEET-013 | Image export | Exports views and sheets as raster images at a chosen resolution. | T1 | 2 |
| AB-SHEET-014 | Printing | Prints sheet sets to system printers with paper size and placement options. | T2 | 2 |
| AB-SHEET-015 | Headless publishing | Publishes a sheet set with a publish profile from the command-line interface without opening a window, returning a nonzero exit status on failure. | T1 | 2 |
| AB-SHEET-016 | Publishing to Arc Hub | Publishes sheets and views as versions to an Arc Hub project for web viewing and markup; returned markups are imported as issues linked to the sheet. | T2 | 3 |
