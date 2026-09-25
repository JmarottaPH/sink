// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Renderer choices applied before WebKitGTK starts. Each flag is set only
/// when the user has not already set that variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WebkitWorkaround {
    /// `WEBKIT_DISABLE_DMABUF_RENDERER=1`
    disable_dmabuf: bool,
    /// `WEBKIT_USE_SKIA_FOR_COMPOSITION=0` (TextureMapper, the pre-2.54 compositor)
    texture_mapper: bool,
    /// `__NV_DISABLE_EXPLICIT_SYNC=1`
    disable_nvidia_explicit_sync: bool,
}

/// WebKitGTK 2.54 composites with Skia. On the proprietary NVIDIA driver under
/// KWin that clips a transparent window to about half its height, flickers,
/// and then drops the surface. TextureMapper paints the whole window, but only
/// if the DMABUF renderer stays on: forcing `WEBKIT_DISABLE_DMABUF_RENDERER`
/// is what leaves the window half-drawn. KWin also rejects WebKit's DMABUF
/// commits unless NVIDIA explicit sync is disabled.
///
/// Other GPUs, and NVIDIA on WebKitGTK older than 2.54, keep the previous
/// workaround (DMABUF renderer off). 2.54 is the release that made the Skia
/// compositor the default.
fn webkit_workaround(nvidia: bool, skia_compositor: bool) -> WebkitWorkaround {
    if nvidia && skia_compositor {
        WebkitWorkaround {
            disable_dmabuf: false,
            texture_mapper: true,
            disable_nvidia_explicit_sync: true,
        }
    } else {
        WebkitWorkaround {
            disable_dmabuf: true,
            texture_mapper: false,
            disable_nvidia_explicit_sync: false,
        }
    }
}

/// WebKitGTK 2.54 is the first stable release whose default compositor is Skia.
fn skia_compositor_default(major: u32, minor: u32) -> bool {
    major > 2 || (major == 2 && minor >= 54)
}

fn webkit_version() -> (u32, u32) {
    // These only read the library version. They do not start the renderer.
    unsafe {
        (
            webkit2gtk::ffi::webkit_get_major_version(),
            webkit2gtk::ffi::webkit_get_minor_version(),
        )
    }
}

fn nvidia_module_loaded() -> bool {
    std::path::Path::new("/sys/module/nvidia").exists()
}

fn set_if_unset(key: &str, value: &str) {
    if std::env::var_os(key).is_none() {
        std::env::set_var(key, value);
    }
}

fn apply_webkit_workaround(workaround: WebkitWorkaround) {
    if workaround.disable_dmabuf {
        set_if_unset("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    if workaround.texture_mapper {
        set_if_unset("WEBKIT_USE_SKIA_FOR_COMPOSITION", "0");
    }
    if workaround.disable_nvidia_explicit_sync {
        set_if_unset("__NV_DISABLE_EXPLICIT_SYNC", "1");
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        let (major, minor) = webkit_version();
        apply_webkit_workaround(webkit_workaround(
            nvidia_module_loaded(),
            skia_compositor_default(major, minor),
        ));
    }

    sink_lib::run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvidia_on_webkit_2_54_uses_texture_mapper_without_disabling_dmabuf() {
        let workaround = webkit_workaround(true, true);
        assert!(!workaround.disable_dmabuf);
        assert!(workaround.texture_mapper);
        assert!(workaround.disable_nvidia_explicit_sync);
    }

    #[test]
    fn nvidia_on_older_webkit_keeps_the_dmabuf_workaround() {
        let workaround = webkit_workaround(true, false);
        assert!(workaround.disable_dmabuf);
        assert!(!workaround.texture_mapper);
        assert!(!workaround.disable_nvidia_explicit_sync);
    }

    #[test]
    fn other_gpus_still_disable_the_dmabuf_renderer() {
        let workaround = webkit_workaround(false, true);
        assert!(workaround.disable_dmabuf);
        assert!(!workaround.texture_mapper);
        assert!(!workaround.disable_nvidia_explicit_sync);
    }

    #[test]
    fn skia_compositor_starts_at_webkit_2_54() {
        assert!(!skia_compositor_default(2, 52));
        assert!(skia_compositor_default(2, 54));
        assert!(skia_compositor_default(3, 0));
    }
}
