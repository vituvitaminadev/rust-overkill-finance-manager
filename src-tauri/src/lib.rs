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
            commands::members,
            commands::categories,
            commands::create_category,
            commands::rename_category,
            commands::set_category_archived,
            commands::entries,
            commands::create_entry,
            commands::update_entry,
            commands::delete_entry,
            commands::settle_entry,
            commands::recurrences,
            commands::create_recurrence,
            commands::end_recurrence,
            commands::installment_plans,
            commands::create_installment_plan,
            commands::delete_installment_plan,
            commands::dashboard,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
