mod app;
mod keys;
mod server;
mod store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}