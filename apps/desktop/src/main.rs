mod capture;
mod composition;
mod observability;
mod recognition;
mod services;
pub mod ui;

fn main() -> iced::Result {
    let debug_mode = std::env::args().skip(1).any(|arg| arg == "--debug");
    composition::run(debug_mode)
}
