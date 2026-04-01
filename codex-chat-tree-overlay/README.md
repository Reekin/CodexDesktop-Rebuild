# Codex Chat Tree Overlay

Windows sidecar overlay for Codex Desktop. The app watches the focused Codex window, docks a narrow transparent panel to its right edge, reads the active session id from `%USERPROFILE%\CodexApp\cur-session-id`, and renders the current chat tree for that session.

The overlay keeps its own helper `codex app-server` process. It reads chat-tree data through the same app-server methods used by the chat-tree branch and writes `%USERPROFILE%\CodexApp\refresh` after a branch switch so the main Codex Desktop window reloads the selected node.

## Requirements

- Windows
- Node.js 20+
- Rust toolchain
- A Codex binary that includes:
  - `thread/chatTree/read`
  - `thread/chatTree/current/set`
- Codex Desktop already patched to:
  - write `%USERPROFILE%\CodexApp\cur-session-id`
  - poll `%USERPROFILE%\CodexApp\refresh`

## Install

```bash
npm install
```

## Run The Overlay

```bash
npm run tauri:dev
```

From the repository root on Windows, you can also use:

```bat
run-overlay-dev.bat
```

That launcher keeps the console open while the dev server is running, stops only when you press `Ctrl+C`, clears a stale overlay process from the same target path, and sets `CARGO_PROFILE_DEV_DEBUG=0` to reduce Windows `.pdb` lock conflicts during rebuilds.

Behavior:

- When the focused foreground window is Codex Desktop, the overlay appears on the right.
- When the foreground window is the overlay itself, it stays visible in the same docked position.
- When focus moves elsewhere, the overlay hides.
- Double-click a node to switch the active branch for the current Codex session.

## Helper Codex Binary Resolution

The helper process resolves the Codex binary in this order:

1. `CODEX_CLI_PATH`
2. `CODEX_CHAT_TREE_BIN`
3. `CODEX_BIN`
4. `I:\gpt-projects\codex\codex-rs\target\debug\codex.exe`
5. `resources\bin\win32-x64\codex.exe` from this repository
6. `codex` from `PATH`

Set `CODEX_CLI_PATH` when you want the overlay to follow the same Codex CLI binary as the rest of your environment.

## Commands

The executable also supports command-line entry points so the full control path can be tested without opening the UI.

Dump the chat tree for a thread:

```bash
cargo run -- chat-tree-dump --thread <session-id>
```

Set the current node for a thread:

```bash
cargo run -- set-current-node --thread <session-id> --node <node-id>
```

Set the current node and ask Codex Desktop to reload:

```bash
cargo run -- set-current-node --thread <session-id> --node <node-id> --refresh
```

Write the desktop refresh signal file directly:

```bash
cargo run -- trigger-refresh
```

Print foreground-window classification and the matched Codex window:

```bash
cargo run -- window-probe
```

## Notes

- The overlay uses polling instead of Win32 event hooks. That keeps the implementation small and predictable while still updating position and visibility every 900ms.
- The UI is intentionally compact and reuses the same tree semantics as the monitor implementation: current node highlighting, branch edges, and double-click switching.
- The helper app-server uses the local `CODEX_HOME` or falls back to `%USERPROFILE%\.codex`.
