fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        if let Err(err) = run_cli(&args[1..]) {
            eprintln!("{err}");
            std::process::exit(1);
        }
        return;
    }

    codex_chat_tree_overlay_lib::run();
}

fn run_cli(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("trigger-refresh") => {
            let home = std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_else(|_| ".".to_string());
            let app_dir = std::path::Path::new(&home).join("CodexApp");
            std::fs::create_dir_all(&app_dir).map_err(|err| err.to_string())?;
            let refresh_path = app_dir.join("refresh");
            std::fs::write(&refresh_path, b"refresh").map_err(|err| err.to_string())?;
            println!("{}", refresh_path.display());
            Ok(())
        }
        Some("window-probe") => {
            #[cfg(target_os = "windows")]
            {
                let kind = codex_chat_tree_overlay_lib::window_probe::classify_foreground_window(
                    "Codex Chat Tree Overlay",
                );
                let match_info =
                    codex_chat_tree_overlay_lib::window_probe::probe_foreground_codex_window();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "foregroundKind": format!("{kind:?}"),
                        "codexWindow": match_info.as_ref().map(|entry| serde_json::json!({
                            "hwnd": entry.hwnd,
                            "title": entry.title,
                            "processName": entry.process_name,
                            "rect": entry.rect
                        }))
                    }))
                    .map_err(|err| err.to_string())?
                );
                Ok(())
            }
            #[cfg(not(target_os = "windows"))]
            {
                Err("window-probe is only supported on Windows.".to_string())
            }
        }
        Some("chat-tree-dump") | Some("set-current-node") => {
            let thread_id = value_after(args, "--thread")
                .ok_or_else(|| "--thread <id> is required".to_string())?;
            let codex_home = resolve_codex_home();
            let runtime = tokio::runtime::Runtime::new().map_err(|err| err.to_string())?;
            runtime.block_on(async move {
                let helper = codex_chat_tree_overlay_lib::bridge::CodexAppServer::spawn(&codex_home)
                    .await?;
                helper.resume_thread(&thread_id).await?;

                if args[0] == "chat-tree-dump" {
                    let tree = helper.read_chat_tree(&thread_id).await?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&tree).map_err(|err| err.to_string())?
                    );
                    return Ok(());
                }

                let node_id = value_after(args, "--node")
                    .ok_or_else(|| "--node <id> is required".to_string())?;
                let current = helper.set_current_node(&thread_id, &node_id).await?;
                let do_refresh = args.iter().any(|value| value == "--refresh");
                if do_refresh {
                    let home = std::env::var("USERPROFILE")
                        .or_else(|_| std::env::var("HOME"))
                        .unwrap_or_else(|_| ".".to_string());
                    let app_dir = std::path::Path::new(&home).join("CodexApp");
                    std::fs::create_dir_all(&app_dir).map_err(|err| err.to_string())?;
                    std::fs::write(app_dir.join("refresh"), b"refresh")
                        .map_err(|err| err.to_string())?;
                }
                println!("{current}");
                Ok(())
            })
        }
        Some(other) => Err(format!("Unknown command: {other}")),
        None => Ok(()),
    }
}

fn value_after(args: &[String], flag: &str) -> Option<String> {
    let index = args.iter().position(|value| value == flag)?;
    args.get(index + 1).cloned()
}

fn resolve_codex_home() -> std::path::PathBuf {
    if let Ok(explicit) = std::env::var("CODEX_HOME") {
        let trimmed = explicit.trim();
        if !trimmed.is_empty() {
            return std::path::PathBuf::from(trimmed);
        }
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    std::path::Path::new(&home).join(".codex")
}
