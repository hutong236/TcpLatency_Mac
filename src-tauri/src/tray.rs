use crate::{
    config::persist_config,
    macos_window::{set_floating_visibility, set_mouse_passthrough_native, show_settings_window},
    runtime::{all_snapshots, emit_active_snapshot, SharedState, TRAY_ID},
};
use std::sync::{atomic::Ordering, Arc};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

fn build_tray_menu(app: &AppHandle, state: &Arc<SharedState>) -> tauri::Result<Menu<tauri::Wry>> {
    let config = state.config.read().map(|c| c.clone()).unwrap_or_default();
    let mut items: Vec<MenuItem<tauri::Wry>> = Vec::new();

    for target in &config.targets {
        let mark = if target.id == config.active_target_id { "✓ " } else { "  " };
        let disabled = if target.enabled { "" } else { " [停用]" };
        let label = format!("{}{} — {}:{}{}", mark, target.name, target.host, target.port, disabled);
        items.push(MenuItem::with_id(
            app,
            format!("target::{}", target.id),
            label,
            true,
            None::<&str>,
        )?);
    }

    let separator1 = PredefinedMenuItem::separator(app)?;
    let floating_item =
        MenuItem::with_id(app, "toggle-floating", "显示/隐藏悬浮窗", true, None::<&str>)?;
    let passthrough_label = if config.mouse_passthrough {
        "✓ 悬浮窗鼠标穿透"
    } else {
        "  悬浮窗鼠标穿透"
    };
    let passthrough_item = MenuItem::with_id(
        app,
        "toggle-passthrough",
        passthrough_label,
        true,
        None::<&str>,
    )?;
    let pause_label = if state.paused.load(Ordering::Relaxed) {
        "恢复监测"
    } else {
        "暂停监测"
    };
    let pause_item = MenuItem::with_id(app, "toggle-pause", pause_label, true, None::<&str>)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let settings_item = MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let mut refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = items
        .iter()
        .map(|item| item as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
        .collect();
    refs.push(&separator1);
    refs.push(&floating_item);
    refs.push(&passthrough_item);
    refs.push(&pause_item);
    refs.push(&separator2);
    refs.push(&settings_item);
    refs.push(&quit_item);

    Menu::with_items(app, &refs)
}

pub(crate) fn refresh_tray_menu(app: &AppHandle, state: &Arc<SharedState>) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_tray_menu(app, state)) {
        let _ = tray.set_menu(Some(menu));
    }
}

pub(crate) fn build_tray(app: &mut tauri::App, state: Arc<SharedState>) -> tauri::Result<()> {
    let menu = build_tray_menu(app.handle(), &state)?;
    let menu_state = state.clone();

    TrayIconBuilder::with_id(TRAY_ID)
        .title("-- ms")
        .tooltip("TCP Latency")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            let id = event.id.as_ref();
            if let Some(target_id) = id.strip_prefix("target::") {
                if let Ok(mut config) = menu_state.config.write() {
                    if config.targets.iter().any(|target| target.id == target_id) {
                        config.active_target_id = target_id.to_string();
                        let _ = persist_config(&config);
                        let emitted = config.clone();
                        drop(config);
                        refresh_tray_menu(app, &menu_state);
                        emit_active_snapshot(app, &menu_state);
                        let _ = app.emit("config-update", emitted);
                    }
                }
                return;
            }

            match id {
                "toggle-floating" => {
                    if let Ok(mut config) = menu_state.config.write() {
                        config.show_floating = !config.show_floating;
                        let _ = persist_config(&config);
                        set_floating_visibility(app, config.show_floating);
                        let _ = app.emit("config-update", config.clone());
                    }
                }
                "toggle-passthrough" => {
                    if let Ok(mut config) = menu_state.config.write() {
                        config.mouse_passthrough = !config.mouse_passthrough;
                        let _ = set_mouse_passthrough_native(app, config.mouse_passthrough);
                        let _ = persist_config(&config);
                        let _ = app.emit("config-update", config.clone());
                        drop(config);
                        refresh_tray_menu(app, &menu_state);
                    }
                }
                "toggle-pause" => {
                    let paused = menu_state.paused.load(Ordering::Relaxed);
                    menu_state.paused.store(!paused, Ordering::Relaxed);
                    menu_state.scheduler_notify.notify_one();
                    emit_active_snapshot(app, &menu_state);
                    let _ = app.emit("targets-update", all_snapshots(&menu_state));
                    refresh_tray_menu(app, &menu_state);
                }
                "settings" => {
                    if let Err(err) = show_settings_window(app) {
                        eprintln!("[ui] 打开设置失败: {err}");
                        let _ = app.emit("app-error", err);
                    }
                }
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}
