---
name: codex-desktop-refresh-restore-patch
description: Reapply the Codex Desktop Windows unpacked-bundle patch that writes the current session id to `%USERPROFILE%\\CodexApp\\cur-session-id`, polls `%USERPROFILE%\\CodexApp\\refresh` to restart the app server, and restores the previous session after restart while keeping message content sourced directly from `thread/resume`. Use when a freshly unpacked or rehashed `src/win/.vite/build/main-*.js` and `src/win/webview/assets/app-server-manager-hooks-*.js` bundle needs the same behavior reimplemented with minimal edits.
---

# Codex Desktop Refresh Restore Patch

## Overview

Patch the unpacked Windows desktop bundles in two places:

1. Patch the main-process bundle so the app:
   - persists the active session id to `%USERPROFILE%\CodexApp\cur-session-id`
   - polls `%USERPROFILE%\CodexApp\refresh` every second
   - deletes the trigger file, restarts the app server, and navigates back to the saved session
2. Patch the renderer app-server manager bundle so `thread/resume` uses the latest server turns directly instead of merging them with stale local turns.

Use string anchors, not line numbers. These bundles are compressed and their line counts are unstable after each unpack.

If a one-line bundle makes `apply_patch` fragile, use a narrow exact-match replacement script with a single-match assertion. Do not do broad regex rewrites.

Read [bundle-anchors.md](./references/bundle-anchors.md) before editing.

## Workflow

### 1. Locate the current bundle files

- Find the main-process bundle under `src/win/.vite/build/main-*.js`.
- Find the renderer bundle under `src/win/webview/assets/app-server-manager-hooks-*.js`.
- Confirm both files still contain the anchor strings from [bundle-anchors.md](./references/bundle-anchors.md).

### 2. Patch the main-process bundle

Apply these three edits in the main bundle:

- Add or preserve the IPC handler branch keyed by `update-cur-session-id`.
  - Write the trimmed `conversationId` to `%USERPROFILE%\CodexApp\cur-session-id`.
  - Create `%USERPROFILE%\CodexApp` first if it does not exist.
- Add or preserve `consumeExternalRefreshTrigger(...)`.
  - Poll `%USERPROFILE%\CodexApp\refresh` every second.
  - Skip the tick if a previous refresh is still in progress.
  - Delete `refresh` before restarting.
  - Read `%USERPROFILE%\CodexApp\cur-session-id` if present.
  - Call the existing app-server connection `restart()`.
  - If a session id exists, ensure the host window and send the existing `navigate-to-route` message using the existing route helper chain.
- Register the poller during startup.
  - Keep the registration near the end of startup, after pending deep links are flushed.
  - Add the returned disposer to the existing disposable collection.

Do not add new IPC channels or new windows logic when the existing route and window helpers already cover the job.

### 3. Patch the renderer resume path

Change only the `thread/resume` call site in `app-server-manager-hooks-*.js`.

- Find the `async function Fm(...)` block that handles `thread/resume`.
- Replace the old merge call:
  - `let w=sm(v,{fallbackCwd:x??null}),E=Im(l?.turns??[],w);`
- With the direct server-turns form:
  - `let E=sm(v,{fallbackCwd:x??null});`

Keep the later assignment:

```js
e.updateConversationState(t,e=>{e.turns=E.map(B), ... })
```

Do not change `Im(...)` or `Lm(...)` globally. Do not reintroduce the reverted sidebar-only refresh experiment.

### 4. Validate

Run the shortest syntax checks first:

```bash
node --check src/win/.vite/build/main-I2_kj945.js
node --check src/win/webview/assets/app-server-manager-hooks-CmyJdQIE.js
```

If the bundle filenames changed, point the commands at the new files you patched.

For a real startup check from the project root, use:

```bash
ELECTRON_BIN=$(node -e "process.stdout.write(require('electron'))")
CLI_PATH=$(cygpath -am resources/bin/win32-x64/codex.exe)
BUILD_FLAVOR=dev ELECTRON_RENDERER_URL=app://-/index.html CODEX_CLI_PATH="$CLI_PATH" "$ELECTRON_BIN" src/win
```

For the refresh flow, create `%USERPROFILE%\CodexApp\refresh` while the app is running and verify logs contain:

- `Consuming external refresh trigger`
- `External refresh restart completed`

If a saved session id exists, verify the app returns to that session after restart and the message area does not rebuild through stale local turn merging.
