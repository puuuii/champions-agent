use champions_interface::PreviewFrame;
use iced::widget::{container, image};
use iced::{ContentFit, Element, Length};

pub struct VideoPreview;

impl VideoPreview {
    pub fn handle_from_frame(frame: &PreviewFrame) -> image::Handle {
        image::Handle::from_rgba(frame.width, frame.height, (*frame.rgba).to_vec())
    }

    pub fn view<'a, Message: 'a>(frame: Option<&image::Handle>) -> Element<'a, Message> {
        match frame {
            Some(handle) => container(
                image::Image::new(handle.clone())
                    .content_fit(ContentFit::Contain)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
            None => container(
                iced::widget::text("カメラ接続待機中...")
                    .font(super::super::JAPANESE_FONT)
                    .size(16),
            )
            .width(Length::Fill)
            .height(Length::Fixed(200.0))
            .center_x(Length::Fill)
            .center_y(Length::Fixed(200.0))
            .into(),
        }
    }
}
