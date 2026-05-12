mod capture;
mod composition;
mod observability;
mod recognition;
mod services;
pub mod ui;

fn main() -> iced::Result {
    composition::run()
}
