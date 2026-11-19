// Release: tanpa jendela konsol. Debug tetap punya konsol untuk log.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod browser;
mod creds;
mod keys;
mod server;
mod store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}