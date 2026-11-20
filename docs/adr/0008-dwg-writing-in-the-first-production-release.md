# 0008 DWG writing in the first production release

Status: Accepted

## Context

The first production release (M2) ships sheet output in PDF, DXF and DWG, and every export format is
validated by an independent reader that is not part of the writing code path
([ADR-0005](0005-housing-acceptance-and-independent-validation.md)). The DWG writing path depends on
a library whose license, process isolation and output fidelity are not yet settled
(OQ-05 in [open-questions.md](../product/open-questions.md)). A release must not be held by one
format, and a format must not be offered without validation results.

## Decision

- OQ-05 is closed during M1: the DWG writing library, its license and its process isolation are
  decided and the independent-reader corpus is in place before M1 exits.
- If DWG writing does not pass the independent validation that the M2 exit criterion requires, the
  M2 release ships with PDF and DXF sheet output and DWG reading. DWG writing is stated as not
  available in that release and is never presented as available without validation results.
- DWG writing is released in the next release after it passes the independent validation.

## Consequences

- The M2 exit criterion in the [roadmap](../roadmap.md) carries this condition and no other change.
- Release notes, the exchange profile table and the user interface state the DWG writing status that
  matches the validation results.
- The DWG write decision is made early enough to leave M2 time to validate it.

## Alternatives considered

- **Hold the release until DWG writing validates.** Delays all other M2 output for one format.
- **Ship DWG writing with a warning and no validation.** Contradicts the rule that self-reading and
  unvalidated output are not evidence of compatibility.

## Reopen when

DWG writing is validated, which ends the conditional, or the decision on the writing library
changes.
