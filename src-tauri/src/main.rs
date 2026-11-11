// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // O renderizador DMABUF do WebKitGTK derruba o app com "Error 71 (Protocol error)"
    // em alguns compositores Wayland. Quem quiser reativá-lo define a variável como 0.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: executado antes de qualquer outra thread existir.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }

    finance_manager_lib::run()
}
