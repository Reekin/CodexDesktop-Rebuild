use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WindowRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub width: i32,
    pub height: i32,
}

impl WindowRect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
            width: right - left,
            height: bottom - top,
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadChatTreeNode {
    pub node_id: String,
    pub parent_node_id: Option<String>,
    pub summary: Option<String>,
    pub turn_id: Option<String>,
    pub order: u32,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadChatTree {
    pub current_node_id: Option<String>,
    pub nodes: Vec<ThreadChatTreeNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlaySnapshot {
    pub visible: bool,
    pub session_id: Option<String>,
    pub is_codex_focused: bool,
    pub status_message: String,
    pub codex_window_rect: Option<WindowRect>,
    pub overlay_rect: Option<WindowRect>,
    pub chat_tree: Option<ThreadChatTree>,
    pub helper_connected: bool,
    pub is_switching: bool,
    pub switching_node_id: Option<String>,
    pub last_error: Option<String>,
}

impl Default for OverlaySnapshot {
    fn default() -> Self {
        Self {
            visible: false,
            session_id: None,
            is_codex_focused: false,
            status_message: "Waiting for Codex Desktop...".to_string(),
            codex_window_rect: None,
            overlay_rect: None,
            chat_tree: None,
            helper_connected: false,
            is_switching: false,
            switching_node_id: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetCurrentNodeResult {
    pub current_node_id: String,
    pub refresh_triggered: bool,
}
