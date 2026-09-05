mod commands;
mod config;
mod macos_window;
mod probe;
mod runtime;
mod tray;

use crate::{
    config::load_config,
    macos_window::{
        apply_floating_window_effect, apply_floating_window_size,
        configure_native_floating_window, configure_native_settings_window,
        set_floating_visibility, set_mouse_passthrough_native,
    },
    runtime::{all_snapshots, emit_active_snapshot, probe_scheduler, SharedState},
    tray::build_tray,
};
use std::sync::Arc;
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_window_state::StateFlags;

fn main() {
    let config = load_config();
    let state = Arc::new(SharedState::new(config.clone()));
    let setup_state = state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::POSITION)
                .with_filter(|label| label == "main")
                .build(),
        )
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::get_snapshot,
            commands::get_all_snapshots,
            commands::get_history,
            commands::test_target,
            commands::save_config,
            commands::set_paused,
            commands::is_paused,
            commands::toggle_floating,
            commands::set_mouse_passthrough,
            commands::set_active_target,
            commands::show_settings
        ])
        .on_window_event(|window, event| {
            if window.label() != "settings" {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                #[cfg(target_os = "macos")]
                {
                    let app = window.app_handle();
                    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                    let _ = app.set_dock_visibility(false);
                }
            }
        })
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            {
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                let _ = app.set_dock_visibility(false);
            }

            build_tray(app, setup_state.clone())?;
            configure_native_floating_window(app.handle())
                .map_err(|e| format!("初始化 macOS 悬浮窗口失败: {e}"))?;
            configure_native_settings_window(app.handle())
                .map_err(|e| format!("初始化 macOS 设置窗口失败: {e}"))?;
            let _ = apply_floating_window_size(app.handle(), &config.floating_size);
            let _ = apply_floating_window_effect(app.handle(), &config.floating_size);
            set_floating_visibility(app.handle(), config.show_floating);
            let _ = set_mouse_passthrough_native(app.handle(), config.mouse_passthrough);

            if config.autostart {
                let _ = app.handle().autolaunch().enable();
            }
            if config.notifications_enabled {
                let _ = app.handle().notification().request_permission();
            }

            emit_active_snapshot(app.handle(), &setup_state);
            let _ = app.emit("targets-update", all_snapshots(&setup_state));

            let app_handle = app.handle().clone();
            let loop_state = setup_state.clone();
            tauri::async_runtime::spawn(async move {
                probe_scheduler(app_handle, loop_state).await;
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running TCP Latency");
}
