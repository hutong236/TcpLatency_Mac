use tauri::{
    window::{Effect, EffectState, EffectsBuilder},
    AppHandle, Manager,
};

#[cfg(target_os = "macos")]
fn native_ns_window_ptr(
    app: &AppHandle,
    label: &str,
    missing_message: &str,
) -> Result<std::ptr::NonNull<objc2_app_kit::NSWindow>, String> {
    use objc2_app_kit::NSWindow;

    let Some(window) = app.get_webview_window(label) else {
        return Err(missing_message.into());
    };

    let ptr = window
        .ns_window()
        .map_err(|e| format!("获取 macOS 原生窗口句柄失败: {e}"))? as *mut NSWindow;

    std::ptr::NonNull::new(ptr).ok_or_else(|| "macOS 原生窗口句柄为空".to_string())
}

#[cfg(target_os = "macos")]
pub(crate) fn configure_native_floating_window(app: &AppHandle) -> Result<(), String> {
    use objc2_app_kit::{NSColor, NSFloatingWindowLevel, NSWindowCollectionBehavior};

    let ns_window_ptr = native_ns_window_ptr(app, "main", "未找到悬浮窗口")?;
    let ns_window = unsafe { ns_window_ptr.as_ref() };

    ns_window.setOpaque(false);
    let clear = NSColor::clearColor();
    ns_window.setBackgroundColor(Some(&clear));
    ns_window.setHasShadow(false);
    ns_window.setHidesOnDeactivate(false);
    ns_window.setCanHide(false);
    ns_window.setMovable(true);
    ns_window.setMovableByWindowBackground(true);
    ns_window.setLevel(NSFloatingWindowLevel);

    let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary
        | NSWindowCollectionBehavior::IgnoresCycle;
    ns_window.setCollectionBehavior(behavior);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn configure_native_floating_window(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn configure_native_settings_window(app: &AppHandle) -> Result<(), String> {
    let ns_window_ptr = native_ns_window_ptr(app, "settings", "未找到设置窗口")?;
    let ns_window = unsafe { ns_window_ptr.as_ref() };

    unsafe {
        ns_window.setReleasedWhenClosed(false);
    }
    ns_window.setHidesOnDeactivate(false);
    ns_window.setCanHide(true);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn configure_native_settings_window(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn activate_settings_window_native(app: &AppHandle) -> Result<(), String> {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};

    let ns_window_ptr = native_ns_window_ptr(app, "settings", "未找到设置窗口")?;
    let ns_window = unsafe { ns_window_ptr.as_ref() };
    let running_app = NSRunningApplication::currentApplication();
    let _ = running_app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
    ns_window.makeKeyAndOrderFront(None);
    ns_window.orderFrontRegardless();
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn activate_settings_window_native(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

pub(crate) fn set_floating_visibility(app: &AppHandle, visible: bool) {
    if let Some(window) = app.get_webview_window("main") {
        if visible {
            let _ = window.show();
        } else {
            let _ = window.hide();
        }
    }
}

fn floating_window_dimensions(size: &str) -> (f64, f64) {
    match size {
        "compact" => (178.0, 76.0),
        "large" => (268.0, 116.0),
        _ => (228.0, 100.0),
    }
}

pub(crate) fn apply_floating_window_size(app: &AppHandle, size: &str) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Err("未找到悬浮窗口".into());
    };
    let (width, height) = floating_window_dimensions(size);
    window
        .set_size(tauri::Size::Logical(tauri::LogicalSize::new(width, height)))
        .map_err(|e| format!("设置悬浮窗尺寸失败: {e}"))
}

fn floating_effect_radius(size: &str) -> f64 {
    match size {
        "compact" => 20.0,
        "large" => 28.0,
        _ => 24.0,
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn apply_floating_window_effect(app: &AppHandle, size: &str) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Err("未找到悬浮窗口".into());
    };

    window
        .set_effects(
            EffectsBuilder::new()
                .effect(Effect::UnderWindowBackground)
                .state(EffectState::Active)
                .radius(floating_effect_radius(size))
                .build(),
        )
        .map_err(|e| format!("设置 macOS 磨砂玻璃效果失败: {e}"))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn apply_floating_window_effect(_app: &AppHandle, _size: &str) -> Result<(), String> {
    Ok(())
}

pub(crate) fn set_mouse_passthrough_native(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Err("未找到悬浮窗口".into());
    };
    window
        .set_ignore_cursor_events(enabled)
        .map_err(|e| format!("设置鼠标穿透失败: {e}"))
}

pub(crate) fn show_settings_window(app: &AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("settings") else {
        return Err("未找到设置窗口".into());
    };

    #[cfg(target_os = "macos")]
    {
        app.set_activation_policy(tauri::ActivationPolicy::Regular)
            .map_err(|e| format!("切换 macOS 激活策略失败: {e}"))?;
        let _ = app.set_dock_visibility(true);
    }

    if window.is_minimized().unwrap_or(false) {
        window
            .unminimize()
            .map_err(|e| format!("恢复设置窗口失败: {e}"))?;
    }
    window
        .show()
        .map_err(|e| format!("显示设置窗口失败: {e}"))?;
    let _ = window.set_focus();
    activate_settings_window_native(app)?;
    Ok(())
}
