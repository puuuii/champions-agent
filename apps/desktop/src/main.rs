mod capture;
mod composition;
mod recognition;
mod services;
pub mod ui;

fn main() -> iced::Result {
    composition::run()
}
