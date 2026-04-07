pub mod bridge;
pub mod model;
pub mod window_probe;

use bridge::CodexAppServer;
use model::{OverlaySnapshot, SetCurrentNodeResult, WindowRect};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Position, Size, State, WebviewWindow,
    WindowEvent,
};
use tokio::sync::Mutex;

const OVERLAY_WINDOW_WIDTH: i32 = 232;
const POLL_INTERVAL_MS: u64 = 900;
const OVERLAY_TITLE: &str = "Codex Chat Tree Overlay";
const TRAY_MENU_SHOW_ID: &str = "tray_show";
const TRAY_MENU_QUIT_ID: &str = "tray_quit";

#[derive(Default)]
struct RuntimeState {
    snapshot: Mutex<OverlaySnapshot>,
    helper: Mutex<Option<Arc<CodexAppServer>>>,
    attached_thread_id: Mutex<Option<String>>,
    current_node_override: Mutex<Option<(String, String)>>,
    last_target_rect: Mutex<Option<WindowRect>>,
}

#[derive(Default)]
struct AppLifecycleState {
    is_quitting: AtomicBool,
}

#[tauri::command]
async fn get_overlay_snapshot(state: State<'_, RuntimeState>) -> Result<OverlaySnapshot, String> {
    Ok(state.snapshot.lock().await.clone())
}

#[tauri::command]
async fn refresh_chat_tree(
    app: AppHandle,
    state: State<'_, RuntimeState>,
) -> Result<OverlaySnapshot, String> {
    reset_helper_state(&state).await;
    sync_runtime(&app, &state).await?;
    Ok(state.snapshot.lock().await.clone())
}

#[tauri::command]
async fn set_current_node(
    app: AppHandle,
    state: State<'_, RuntimeState>,
    node_id: String,
) -> Result<SetCurrentNodeResult, String> {
    let next_node_id = node_id.trim().to_string();
    if next_node_id.is_empty() {
        return Err("nodeId is required".to_string());
    }

    let thread_id = {
        let snapshot = state.snapshot.lock().await;
        snapshot
            .session_id
            .clone()
            .ok_or_else(|| "No active session attached".to_string())?
    };

    {
        let mut snapshot = state.snapshot.lock().await;
        snapshot.is_switching = true;
        snapshot.switching_node_id = Some(next_node_id.clone());
        snapshot.last_error = None;
        snapshot.status_message = "Switching branch and refreshing Codex...".to_string();
    }

    let current_node_id = match set_current_node_once(&state, &thread_id, &next_node_id, true).await
    {
        Ok(current_node_id) => current_node_id,
        Err(error) if should_refresh_helper_for_error(&error) => {
            set_current_node_once(&state, &thread_id, &next_node_id, true).await?
        }
        Err(error) => return Err(error),
    };
    *state.current_node_override.lock().await = Some((thread_id.clone(), current_node_id.clone()));
    trigger_refresh_file()?;

    {
        let mut snapshot = state.snapshot.lock().await;
        snapshot.is_switching = false;
        snapshot.switching_node_id = None;
        snapshot.status_message = "Branch switched. Waiting for Codex Desktop refresh.".to_string();
    }

    sync_runtime(&app, &state).await?;

    Ok(SetCurrentNodeResult {
        current_node_id,
        refresh_triggered: true,
    })
}

pub fn run() {
    tauri::Builder::default()
        .manage(RuntimeState::default())
        .manage(AppLifecycleState::default())
        .setup(|app| {
            setup_tray(app)?;
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = background_watch_loop(app_handle).await;
            });
            Ok(())
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_MENU_SHOW_ID => {
                let _ = show_overlay_window(app);
            }
            TRAY_MENU_QUIT_ID => {
                if let Some(state) = app.try_state::<AppLifecycleState>() {
                    state.is_quitting.store(true, Ordering::SeqCst);
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|app, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => {
                let _ = show_overlay_window(app);
            }
            _ => {}
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            let Some(state) = window.try_state::<AppLifecycleState>() else {
                return;
            };
            if state.is_quitting.load(Ordering::SeqCst) {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_overlay_snapshot,
            refresh_chat_tree,
            set_current_node
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn background_watch_loop(app: AppHandle) -> Result<(), String> {
    loop {
        if let Some(state) = app.try_state::<RuntimeState>() {
            let _ = sync_runtime(&app, &state).await;
        }
        tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
    }
}

async fn sync_runtime(app: &AppHandle, state: &RuntimeState) -> Result<(), String> {
    let session_id = read_current_session_id();
    let foreground = window_probe::classify_foreground_window(OVERLAY_TITLE);
    let codex_window = if foreground == window_probe::ForegroundKind::Codex {
        window_probe::probe_foreground_codex_window()
    } else {
        None
    };

    if let Some(window) = app.get_webview_window("main") {
        let visible = match foreground {
            window_probe::ForegroundKind::Codex => {
                if let Some(ref target) = codex_window {
                    *state.last_target_rect.lock().await = Some(target.rect);
                    position_overlay_window(&window, target.rect)?;
                    true
                } else {
                    false
                }
            }
            window_probe::ForegroundKind::Overlay => {
                if let Some(last_rect) = *state.last_target_rect.lock().await {
                    position_overlay_window(&window, last_rect)?;
                    true
                } else {
                    false
                }
            }
            window_probe::ForegroundKind::Other => false,
        };

        if visible {
            let _ = window.show();
        } else {
            let _ = window.hide();
        }

        let overlay_rect = current_overlay_rect(visible.then_some(&window))?;
        let mut snapshot = state.snapshot.lock().await;
        snapshot.visible = visible;
        snapshot.is_codex_focused = foreground == window_probe::ForegroundKind::Codex;
        snapshot.overlay_rect = overlay_rect;
        snapshot.codex_window_rect = codex_window.as_ref().map(|entry| entry.rect);
    }

    update_session_tree(state, session_id).await?;
    Ok(())
}

async fn update_session_tree(
    state: &RuntimeState,
    session_id: Option<String>,
) -> Result<(), String> {
    let mut snapshot = state.snapshot.lock().await;
    snapshot.session_id = session_id.clone();

    let Some(thread_id) = session_id else {
        snapshot.helper_connected = false;
        snapshot.chat_tree = None;
        snapshot.status_message = "Waiting for Codex Desktop session focus...".to_string();
        snapshot.last_error = None;
        *state.attached_thread_id.lock().await = None;
        *state.current_node_override.lock().await = None;
        return Ok(());
    };
    drop(snapshot);

    let helper = ensure_helper(state).await?;
    let helper_connected = helper.is_alive().await;
    if !helper_connected {
        let mut snapshot = state.snapshot.lock().await;
        snapshot.helper_connected = false;
        snapshot.chat_tree = None;
        snapshot.last_error = Some("Helper Codex app-server is not running.".to_string());
        snapshot.status_message = "Restarting helper app-server...".to_string();
        drop(snapshot);
        reset_helper_state(state).await;
        return Ok(());
    }

    let needs_resume = {
        let attached = state.attached_thread_id.lock().await;
        attached.as_deref() != Some(thread_id.as_str())
    };
    if needs_resume {
        helper.resume_thread(&thread_id).await?;
        *state.attached_thread_id.lock().await = Some(thread_id.clone());
    }

    let mut chat_tree = helper.read_chat_tree(&thread_id).await?;
    apply_current_node_override(state, &thread_id, &mut chat_tree).await;
    let mut snapshot = state.snapshot.lock().await;
    snapshot.helper_connected = true;
    snapshot.chat_tree = Some(chat_tree.clone());
    snapshot.status_message = format!(
        "Attached to session {} with {} node{}.",
        &thread_id[..8],
        chat_tree.nodes.len(),
        if chat_tree.nodes.len() == 1 { "" } else { "s" }
    );
    snapshot.last_error = None;
    Ok(())
}

async fn ensure_helper(state: &RuntimeState) -> Result<Arc<CodexAppServer>, String> {
    if let Some(existing) = state.helper.lock().await.clone() {
        if existing.is_alive().await {
            return Ok(existing);
        }
    }

    let helper = CodexAppServer::spawn(&resolve_codex_home()).await?;
    *state.helper.lock().await = Some(helper.clone());
    Ok(helper)
}

async fn ensure_fresh_helper(state: &RuntimeState) -> Result<Arc<CodexAppServer>, String> {
    reset_helper_state(state).await;
    ensure_helper(state).await
}

async fn reset_helper_state(state: &RuntimeState) {
    let existing = {
        let mut helper_slot = state.helper.lock().await;
        helper_slot.take()
    };
    if let Some(existing) = existing {
        let _ = existing.shutdown().await;
    }
    *state.attached_thread_id.lock().await = None;
    *state.current_node_override.lock().await = None;
}

async fn apply_current_node_override(
    state: &RuntimeState,
    thread_id: &str,
    chat_tree: &mut model::ThreadChatTree,
) {
    let mut override_slot = state.current_node_override.lock().await;
    let Some((override_thread_id, override_node_id)) = override_slot.as_ref() else {
        return;
    };

    if override_thread_id != thread_id {
        *override_slot = None;
        return;
    }

    if chat_tree
        .nodes
        .iter()
        .any(|node| node.node_id == *override_node_id)
    {
        chat_tree.current_node_id = Some(override_node_id.clone());
        return;
    }

    *override_slot = None;
}

async fn set_current_node_once(
    state: &RuntimeState,
    thread_id: &str,
    node_id: &str,
    use_fresh_helper: bool,
) -> Result<String, String> {
    let helper = if use_fresh_helper {
        ensure_fresh_helper(state).await?
    } else {
        ensure_helper(state).await?
    };
    helper.resume_thread(thread_id).await?;
    *state.attached_thread_id.lock().await = Some(thread_id.to_string());
    helper.set_current_node(thread_id, node_id).await
}

fn should_refresh_helper_for_error(error: &str) -> bool {
    error.contains("unknown chat tree node id")
        || error.contains("unknown node")
        || error.contains("thread not loaded:")
        || error.contains("request canceled")
        || error.contains("request timed out")
}

fn resolve_codex_home() -> PathBuf {
    if let Ok(explicit) = env::var("CODEX_HOME") {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    let home = env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".codex")
}

fn read_current_session_id() -> Option<String> {
    let content = fs::read_to_string(codex_app_dir().join("cur-session-id")).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn trigger_refresh_file() -> Result<(), String> {
    let app_dir = codex_app_dir();
    fs::create_dir_all(&app_dir).map_err(|err| err.to_string())?;
    fs::write(app_dir.join("refresh"), b"refresh").map_err(|err| err.to_string())
}

fn codex_app_dir() -> PathBuf {
    let home = env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join("CodexApp")
}

fn position_overlay_window(window: &WebviewWindow, target_rect: WindowRect) -> Result<(), String> {
    let work_area = window_probe::overlay_work_area(target_rect);
    let width = OVERLAY_WINDOW_WIDTH;
    let height = work_area.height.max(240);

    let max_left = work_area.right - width;
    let desired_left = target_rect.right;
    let x = desired_left.min(max_left).max(work_area.left);
    let max_top = work_area.bottom - height;
    let y = target_rect.top.min(max_top).max(work_area.top);

    window
        .set_size(Size::Physical(PhysicalSize::new(
            width as u32,
            height as u32,
        )))
        .map_err(|err| err.to_string())?;
    window
        .set_position(Position::Physical(PhysicalPosition::new(x, y)))
        .map_err(|err| err.to_string())?;
    Ok(())
}

fn current_overlay_rect(window: Option<&WebviewWindow>) -> Result<Option<WindowRect>, String> {
    let Some(window) = window else {
        return Ok(None);
    };
    let position = window.outer_position().map_err(|err| err.to_string())?;
    let size = window.outer_size().map_err(|err| err.to_string())?;
    Ok(Some(WindowRect::new(
        position.x,
        position.y,
        position.x + size.width as i32,
        position.y + size.height as i32,
    )))
}

fn setup_tray(app: &tauri::App) -> Result<(), String> {
    let show_item = MenuItem::with_id(app, TRAY_MENU_SHOW_ID, "Show Overlay", true, None::<&str>)
        .map_err(|err| err.to_string())?;
    let quit_item = MenuItem::with_id(app, TRAY_MENU_QUIT_ID, "Quit", true, None::<&str>)
        .map_err(|err| err.to_string())?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item]).map_err(|err| err.to_string())?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| "default tray icon is missing".to_string())?;

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("Codex Chat Tree Overlay")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .build(app)
        .map_err(|err| err.to_string())?;
    Ok(())
}

fn show_overlay_window(app: &AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Err("main overlay window is missing".to_string());
    };
    let _ = window.unminimize();
    window.show().map_err(|err| err.to_string())?;
    window.set_focus().map_err(|err| err.to_string())
}
