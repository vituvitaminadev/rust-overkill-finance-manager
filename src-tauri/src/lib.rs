mod commands;
mod desk;
mod error;
mod remember;

use std::sync::Mutex;

use tauri::Manager;

use commands::AppState;
use desk::Desk;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let dir = app.path().app_data_dir()?.join("households");
            app.manage(AppState(Mutex::new(Desk::new(dir)?)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::status,
            commands::login,
            commands::logout,
            commands::create_household,
            commands::select_household,
            commands::close_household,
            commands::host_command,
            commands::members,
            commands::categories,
            commands::entries,
            commands::dashboard,
            commands::recurrences,
            commands::installment_plans,
            commands::create_entry,
            commands::update_entry,
            commands::settle_entry,
            commands::delete_entry,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
