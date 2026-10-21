// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--mcp" || arg == "mcp") {
        wasenoffice_lib::run_mcp_server();
        return;
    }
    wasenoffice_lib::run()
}
