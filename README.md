# Codex Desktop Rebuild

This branch is intended to be used together with the chat-tree-enabled Codex branch:

- <https://github.com/Reekin/codex/tree/rebase/v0.116-chat-tree>

It contains a Windows-focused Codex Desktop adaptation for chat-tree workflows.

## What This Branch Includes

- A standalone sidecar application that displays the current conversation node graph and lets you switch branches from an external overlay UI.
- Lightweight adjustments to the unpacked Codex Desktop code so the desktop app can cooperate with the sidecar workflow.
- Branch-specific support files under `.codex/skills/`.

## Sidecar Overlay

The sidecar project lives in `codex-chat-tree-overlay/`.

Its purpose is to:

- detect the focused Codex Desktop window,
- dock a narrow floating panel beside it,
- read the current session id from `%USERPROFILE%\CodexApp\cur-session-id`,
- render the conversation chat tree,
- switch the current node,
- notify Codex Desktop to refresh after a branch switch.

This overlay is designed for the chat-tree behavior provided by the `rebase/v0.116-chat-tree` Codex branch linked above.

## Desktop Adaptation

This branch also includes small edits to the unpacked desktop bundle so the rebuilt Codex Desktop app can work with the overlay flow.

These edits are used to:

- persist the active session id to `%USERPROFILE%\CodexApp\cur-session-id`,
- react to `%USERPROFILE%\CodexApp\refresh`,
- restore the active session after the app-server restart required by node switching.

## Skills Directory

The `.codex/skills/` directory is part of this branch and should be committed.

It is not temporary local tooling. It contains branch-specific instructions used to reapply and maintain the desktop adaptation logic.

## Notes

- This branch is not a generic upstream rebuild snapshot.
- It is a working integration branch for chat-tree visualization and control.
- If you use this branch without the matching Codex branch, the overlay and refresh workflow will be incomplete.
