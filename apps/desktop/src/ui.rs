pub mod app;
pub mod components;
pub mod pokemon;
pub mod subscriptions;

pub const JAPANESE_FONT: iced::Font = iced::Font {
    family: iced::font::Family::Name("Noto Sans JP"),
    ..iced::Font::DEFAULT
};
