pub mod app;
pub mod pokemon;

pub const JAPANESE_FONT: iced::Font = iced::Font {
    family: iced::font::Family::Name("Noto Sans JP"),
    ..iced::Font::DEFAULT
};

#[cfg(test)]
mod tests {
    #[test]
    fn test_ui_module_exists() {
        assert!(true);
    }
}
