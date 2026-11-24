//! The renderer that draws the window. It is wgpu, which runs on Vulkan, DirectX 12, Metal or
//! OpenGL ES. On Linux AnyVector can also draw with OpenGL (glow), for the graphics drivers wgpu
//! can't start on (no Vulkan driver and OpenGL older than ES 3.0, some virtual machines and
//! remote desktops): when wgpu fails before the window has opened, AnyVector starts again with
//! OpenGL, and `ANYVECTOR_RENDERER=gl` starts with OpenGL straight away.

/// The variable that picks the renderer on Linux: `gl` (or `glow`, `opengl`) for OpenGL.
#[cfg(target_os = "linux")]
pub const ENV: &str = "ANYVECTOR_RENDERER";

/// The renderer `ANYVECTOR_RENDERER` asks for: OpenGL for `gl`, `glow` or `opengl`, else wgpu.
#[cfg(target_os = "linux")]
pub fn from_env(value: Option<&str>) -> eframe::Renderer {
    match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
        Some("gl" | "glow" | "opengl") => eframe::Renderer::Glow,
        _ => eframe::Renderer::Wgpu,
    }
}

/// Why wgpu couldn't draw the window, when AnyVector should start again with OpenGL: wgpu
/// returned an error or panicked (no adapter, no device, a surface the adapter can't present
/// to) before the app was created. Once the app runs, its errors and panics are its own.
#[cfg(target_os = "linux")]
pub fn wgpu_failure(app_started: bool, result: &std::thread::Result<eframe::Result<()>>) -> Option<String> {
    if app_started {
        return None;
    }
    match result {
        Ok(Err(e @ eframe::Error::Wgpu(_))) => Some(e.to_string()),
        // no window system, a window that couldn't be made: OpenGL wouldn't do better
        Ok(_) => None,
        Err(panic) => Some(
            panic
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic".into()),
        ),
    }
}

/// Starts AnyVector again with OpenGL: the same program, arguments and environment plus
/// `ANYVECTOR_RENDERER=gl`, in this process (exec). A fresh start, as the failed attempt may
/// have left the display connection or the graphics driver half set up. Only returns if that
/// fails.
#[cfg(target_os = "linux")]
pub fn restart_with_opengl() -> std::io::Error {
    use std::os::unix::process::CommandExt as _;
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => return e,
    };
    let mut args = std::env::args_os();
    let mut cmd = std::process::Command::new(exe);
    if let Some(arg0) = args.next() {
        cmd.arg0(arg0);
    }
    cmd.args(args).env(ENV, "gl").exec()
}

/// The OpenGL renderer drawing the window, for Help ▸ System Info, like the wgpu adapter's name:
/// "llvmpipe (LLVM 17.0.6, 256 bits) (OpenGL, 4.5 (Core Profile) Mesa 24.0.5)". Empty when the
/// window is drawn with wgpu.
pub fn opengl_name(cc: &eframe::CreationContext<'_>) -> String {
    #[cfg(target_os = "linux")]
    if let Some(gl) = cc.gl.as_ref() {
        use eframe::glow::HasContext as _;
        // SAFETY: plain queries on the window's current OpenGL context
        let (renderer, version) = unsafe { (gl.get_parameter_string(eframe::glow::RENDERER), gl.get_parameter_string(eframe::glow::VERSION)) };
        return describe_opengl(&renderer, &version);
    }
    let _ = cc;
    String::new()
}

/// "{renderer} (OpenGL, {version})", without the "OpenGL" OpenGL ES puts in front of its version.
#[cfg(any(target_os = "linux", test))]
fn describe_opengl(renderer: &str, version: &str) -> String {
    let version = version.trim();
    let version = version.strip_prefix("OpenGL").map(str::trim_start).unwrap_or(version);
    format!("{} (OpenGL, {version})", renderer.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opengl_is_described_like_a_wgpu_adapter() {
        assert_eq!(
            describe_opengl("llvmpipe (LLVM 17.0.6, 256 bits)", "4.5 (Core Profile) Mesa 24.0.5"),
            "llvmpipe (LLVM 17.0.6, 256 bits) (OpenGL, 4.5 (Core Profile) Mesa 24.0.5)"
        );
        assert_eq!(describe_opengl("Mali-G52 ", "OpenGL ES 3.2 Mesa 24.0.5\n"), "Mali-G52 (OpenGL, ES 3.2 Mesa 24.0.5)");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn opengl_is_picked_by_name() {
        use eframe::Renderer::{Glow, Wgpu};
        for v in ["gl", "GL", "glow", " opengl ", "OpenGL"] {
            assert_eq!(from_env(Some(v)), Glow, "{v:?}");
        }
        for v in [None, Some(""), Some("wgpu"), Some("vulkan"), Some("gles")] {
            assert_eq!(from_env(v), Wgpu, "{v:?}");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn only_wgpu_failing_before_the_app_starts_falls_back() {
        let wgpu_err = || -> std::thread::Result<eframe::Result<()>> { Ok(Err(eframe::Error::Wgpu(eframe::egui_wgpu::WgpuError::NoSurfaceFormatsAvailable))) };
        let panicked = |p: Box<dyn std::any::Any + Send>| -> std::thread::Result<eframe::Result<()>> { Err(p) };
        // wgpu errors and panics before the app exists: start again with OpenGL, saying why
        assert_eq!(wgpu_failure(false, &wgpu_err()), Some("WGPU error: There was no valid format for the surface at all.".into()));
        assert_eq!(wgpu_failure(false, &panicked(Box::new("The surface isn't supported by this adapter"))), Some("The surface isn't supported by this adapter".into()));
        assert_eq!(wgpu_failure(false, &panicked(Box::new(String::from("no adapter")))), Some("no adapter".into()));
        // the app ran: its exit, errors and panics are passed on as they are
        assert_eq!(wgpu_failure(true, &wgpu_err()), None);
        assert_eq!(wgpu_failure(true, &panicked(Box::new("a bug"))), None);
        assert_eq!(wgpu_failure(true, &Ok(Ok(()))), None);
        // closed before it started, or failures OpenGL wouldn't fix
        assert_eq!(wgpu_failure(false, &Ok(Ok(()))), None);
        let app_err: std::thread::Result<eframe::Result<()>> = Ok(Err(eframe::Error::AppCreation("x".into())));
        assert_eq!(wgpu_failure(false, &app_err), None);
    }
}
