# Bundle Anchors

Use these anchors to reapply the patch after future unpack/rebuild runs. Prefer exact string search over line numbers.

## Main Bundle

Target file pattern:

- `src/win/.vite/build/main-*.js`

Apply three edits in this file.

### 1. Persist the active session id

Search for:

```js
update-cur-session-id
```

Desired behavior:

- Accept `conversationId`
- Trim it
- Ignore empty ids
- Write it to `%USERPROFILE%\CodexApp\cur-session-id`

Stable anchor excerpt:

```js
if(n.type===`update-cur-session-id`){ ... }
```

### 2. Add the external refresh poller

Search for:

```js
function consumeExternalRefreshTrigger(
```

Desired behavior:

- Resolve `%USERPROFILE%` or `homedir()`
- Watch `%USERPROFILE%\CodexApp\refresh`
- Read `%USERPROFILE%\CodexApp\cur-session-id`
- `await appServerConnection.restart()`
- If a session id exists, call the existing `navigate-to-route` flow

Stable anchor excerpt:

```js
function consumeExternalRefreshTrigger({appServerConnection:n,windowServices:i,hostId:a,pollIntervalMs:o=1e3})
```

### 3. Register the poller during startup

Search for:

```js
external refresh trigger watcher initialized
```

Desired behavior:

- Build the disposer with `consumeExternalRefreshTrigger(...)`
- Add it to the existing disposables collection
- Keep it near the end of startup, after `flushPendingDeepLinks()`

Stable anchor excerpt:

```js
let externalRefreshDisposer=consumeExternalRefreshTrigger(...)
```

## Renderer Resume Bundle

Target file pattern:

- `src/win/webview/assets/app-server-manager-hooks-*.js`

Patch only the `thread/resume` call site in `Fm(...)`.

### Old merged form

Search for:

```js
let w=sm(v,{fallbackCwd:x??null}),E=Im(l?.turns??[],w);
```

This is the stale merge path that makes message content look incrementally restored.

### Desired direct form

Replace it with:

```js
let E=sm(v,{fallbackCwd:x??null});
```

Keep the later assignment:

```js
e.updateConversationState(t,e=>{e.turns=E.map(B),...})
```

### Do not change

- `Im(...)`
- `Lm(...)`
- Sidebar refresh behavior

The goal is to change only the `thread/resume` call site, not the global merge helpers.
