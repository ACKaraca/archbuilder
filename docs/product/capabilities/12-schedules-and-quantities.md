# Schedules and quantities

This area specifies schedules as live tables of model data, material takeoffs, document lists,
quantity rules, carbon and cost data, and spreadsheet exchange. Schedules are views of the model;
editing a cell edits the element.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Schedule | Categories, fields, filters, sort and group rules, totals, formatting, phase, option | Queries the model; placed on sheets |
| Field | Parameter or calculated expression, format, conditional styles | Belongs to a schedule |
| Key schedule | Key name, key values setting several parameters | Referenced by a key parameter on elements |
| Quantity rule | Element kind, measure (length, net and gross area, volume, count), method | Read by schedules, takeoffs and IFC quantities |
| Carbon factor | Material, unit basis, value, source reference | Attached to a material |
| Cost item | Type or material, unit, rate | Attached to types or materials |

## Schedules

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-SCHED-001 | Element schedules | Lists elements of one or several categories with chosen fields (instance, type, built-in, library, calculated), filters, sorting and grouping with headers, footers and totals, either itemized or aggregated by group. | T1 | 2 |
| AB-SCHED-002 | Calculated fields | Adds fields computed with the bounded expression language, including percentages of group or grand totals. | T1 | 2 |
| AB-SCHED-003 | Two-way editing | Writes a cell edit to the element or type parameter in one command; editing a type parameter states how many elements change. Read-only and computed cells cannot be edited. | T1 | 2 |
| AB-SCHED-004 | Conditional formatting | Styles cells or rows (text color, background, bold) by rules over field values, and can show rule violations as highlighted rows. | T2 | 2 |
| AB-SCHED-005 | Schedule formatting | Sets column widths, grouped headers, text styles, borders, hidden columns, row height and image fields. | T1 | 2 |
| AB-SCHED-006 | Key schedules | Defines key values that set several parameters at once (for example a room style setting all finishes); changing a key row updates every element that uses it. | T2 | 2 |
| AB-SCHED-007 | Document lists | Generates sheet lists, view lists, revision lists and note blocks (counts of annotation symbols by value) as schedules. | T1 | 2 |
| AB-SCHED-008 | Selection from schedules | Selects and highlights the elements of selected rows in open views, and opens a view showing a chosen element. | T1 | 2 |
| AB-SCHED-009 | Linked elements in schedules | Includes elements from linked documents as read-only rows, identified by link. | T2 | 3 |
| AB-SCHED-010 | Phase and option scheduling | Filters schedules by phase status and by design option. | T2 | 3 |
| AB-SCHED-011 | Cross-tab schedules | Groups rows by one field and columns by another, with totals in each cell. | T3 | 3 |

## Quantities and data exchange

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-SCHED-012 | Quantity rules | Computes base quantities per element kind (length, net and gross side and footprint area, net and gross volume, count, weight) by a documented measurement method, used identically by schedules, takeoffs and IFC quantity sets. | T1 | 2 |
| AB-SCHED-013 | Material takeoff | Lists areas and volumes per material, including the individual layers of compound elements and parts, with material parameters as fields. | T1 | 2 |
| AB-SCHED-014 | Embodied carbon | Attaches carbon factors (per mass, volume or area, with source reference) to materials, computes totals per element, material and group in takeoffs, and keeps factors editable and schedulable. | T2 | 3 |
| AB-SCHED-015 | Cost data | Attaches unit rates to types and materials and computes cost totals in schedules and takeoffs. | T2 | 3 |
| AB-SCHED-016 | Spreadsheet exchange | Exports schedules to CSV and XLSX with stable element IDs, and imports edited files back with a preview of changed values, rejected cells and conflicts with edits made since export. | T2 | 2 |
