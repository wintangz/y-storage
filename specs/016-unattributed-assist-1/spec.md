# Feature Specification: `unattributed-assist-1`

**Feature Branch**: `main` (building on `6d8b121`)
**Created**: 2026-11-26
**Status**: Active
**Input**: Owner feature request (`unattributed-assist-1`): help the owner identify folders in the Unattributed group while keeping `y-storage` strictly read-only (external OS launches only; no network fetches, writes, moves, or deletes).

## Requirements
- **FIX 1 — Locate**: Every folder row in the expanded `Unattributed` group renders a compact `Locate` outline button that opens Windows Explorer with that folder selected (`explorer.exe /select,"<full path>"`). If the folder no longer exists on disk, the button is disabled with hover tooltip `"no longer on disk"` and never opens Explorer to a fallback location.
- **FIX 2 — Web search**: Every unattributed folder row also renders a compact `Web search` outline button that opens the owner's default browser via `ShellExecuteW` to Google Search (`https://www.google.com/search?q=...`) for `"<folder_name>" "<parent_name>" "what is this folder"`. The exact constructed query is visible on hover before clicking; `y-storage` makes zero direct network requests.
- **FIX 3 — Placement + Scope**: Both actions appear strictly on unattributed folder rows (the pinned ledger's expanded `Unattributed` view) and MUST NOT appear on attributed app locations, `Windows & system`, `Your files`, or `Shared & system caches`. Follow the compact small-outline button pattern and document the usage note in `DESIGN.md` Part II.
