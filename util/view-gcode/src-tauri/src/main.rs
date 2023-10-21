// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{fs::{File, read_to_string}, env::current_dir};

use gcode::coordinates::PartialPosition;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn lines_in_file(name: &str) -> Result<String, String> {
    read_to_string(name).map_err(|_| "Not found".into())
}

fn main() {
    tauri::Builder::default()
    .setup(|app| Ok(()))
    .invoke_handler(tauri::generate_handler![greet])
    .invoke_handler(tauri::generate_handler![lines_in_file])
    .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
