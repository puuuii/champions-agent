use champions_runtime::PreviewFrame;
use iced::widget::{container, image};
use iced::{ContentFit, Element, Length};

pub struct VideoPreview;

impl VideoPreview {
    pub fn view<'a, Message: 'a>(frame: Option<&PreviewFrame>) -> Element<'a, Message> {
        match frame {
            Some(f) => {
                let handle = image::Handle::from_rgba(f.width, f.height, (*f.rgba).to_vec());
                container(
                    image::Image::new(handle)
                        .content_fit(ContentFit::Contain)
                        .width(Length::Fill)
                        .height(Length::Fill),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
            }
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
