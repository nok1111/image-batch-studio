# ADR 0004 — Output safety rules

Status: accepted (enforced from Phase 2)

- The engine refuses any output path that resolves to an input path, whatever the conflict policy.
- Output is written to a temporary file in the destination folder, verified (decodes, expected dimensions, size > 0), then atomically renamed. Success is reported only after verification.
- The `overwrite` conflict policy only applies to existing *output* files.
- Bulk Renamer copies renamed files to the output folder by default; in-place rename is an explicit option with an undo journal.
