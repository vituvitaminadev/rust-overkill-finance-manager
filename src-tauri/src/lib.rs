mod commands;
mod desk;
mod error;
mod net;
mod remember;

use std::sync::Mutex;

use tauri::Manager;

use commands::AppState;
use desk::Desk;
use net::Network;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let dir = app.path().app_data_dir()?.join("households");
            let desk = Desk::new(dir)?;
            let hosted = desk.hosted_files();
            app.manage(AppState {
                desk: Mutex::new(desk),
                net: Network::new(app.handle().clone()),
            });

            // As residências criadas nesta máquina ficam disponíveis na rede
            // enquanto o app estiver aberto, com ou sem alguém conectado.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                for path in hosted {
                    let _ = state.net.host(&path).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::status,
            commands::login,
            commands::logout,
            commands::create_household,
            commands::select_household,
            commands::close_household,
            commands::discover,
            commands::inspect_host,
            commands::join_household,
            commands::sync_info,
            commands::conflicts,
            commands::resolve_conflict,
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
