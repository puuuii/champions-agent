use champions_runtime::{PixelFormat, PreviewFrame};
use iced::widget::container;
use iced::widget::shader::{
    Pipeline as ShaderPipeline, Primitive as ShaderPrimitive, Program as ShaderProgram,
};
use iced::{Element, Length, Rectangle, mouse, wgpu};
use std::fmt;
use std::sync::Arc;

pub struct VideoPreview;

impl VideoPreview {
    pub fn view<'a, Message: 'a>(frame: Option<&'a PreviewFrame>) -> Element<'a, Message> {
        match frame {
            Some(frame) => container(
                iced::widget::Shader::new(GpuPreviewProgram::new(frame))
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

struct GpuPreviewProgram<'a> {
    frame: &'a PreviewFrame,
}

impl<'a> GpuPreviewProgram<'a> {
    fn new(frame: &'a PreviewFrame) -> Self {
        Self { frame }
    }
}

impl<Message> ShaderProgram<Message> for GpuPreviewProgram<'_> {
    type State = ();
    type Primitive = PreviewPrimitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        PreviewPrimitive {
            bounds,
            upload: PreviewUpload::from_frame(self.frame),
        }
    }
}

struct PreviewUpload {
    sequence: u64,
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
    pixels: Arc<[u8]>,
}

impl PreviewUpload {
    fn from_frame(frame: &PreviewFrame) -> Self {
        Self {
            sequence: frame.frame_sequence.0,
            width: frame.width,
            height: frame.height,
            pixel_format: frame.pixel_format,
            pixels: frame.pixels.clone(),
        }
    }
}

impl fmt::Debug for PreviewUpload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreviewUpload")
            .field("sequence", &self.sequence)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pixel_format", &self.pixel_format)
            .field("pixels_len", &self.pixels.len())
            .finish()
    }
}

struct PreviewPrimitive {
    bounds: Rectangle,
    upload: PreviewUpload,
}

impl fmt::Debug for PreviewPrimitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreviewPrimitive")
            .field("bounds", &self.bounds)
            .field("upload", &self.upload)
            .finish()
    }
}

impl ShaderPrimitive for PreviewPrimitive {
    type Pipeline = PreviewPipeline;

    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        _viewport: &iced::widget::shader::Viewport,
    ) {
        pipeline.prepare_frame(device, queue, bounds, &self.upload);
    }

    fn draw(&self, pipeline: &Self::Pipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        let Some(bind_group) = pipeline.bind_group.as_ref() else {
            return true;
        };

        render_pass.set_pipeline(&pipeline.render_pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.draw(0..6, 0..1);
        true
    }
}

struct PreviewPipeline {
    render_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    bind_group: Option<wgpu::BindGroup>,
    texture: Option<PreviewTexture>,
    uploaded_sequence: Option<u64>,
}

struct PreviewTexture {
    texture: wgpu::Texture,
    _view: wgpu::TextureView,
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
}

impl ShaderPipeline for PreviewPipeline {
    fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("champions.preview.bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("champions.preview.sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("champions.preview.uniforms"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("champions.preview.shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("video_preview.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("champions.preview.pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("champions.preview.pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Self {
            render_pipeline,
            bind_group_layout,
            sampler,
            uniform_buffer,
            bind_group: None,
            texture: None,
            uploaded_sequence: None,
        }
    }
}

impl PreviewPipeline {
    fn prepare_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        upload: &PreviewUpload,
    ) {
        if upload.width == 0 || upload.height == 0 {
            return;
        }

        let expected_len =
            upload.width as usize * upload.height as usize * upload.pixel_format.bytes_per_pixel();
        if upload.pixels.len() != expected_len {
            return;
        }

        if texture_format(upload.pixel_format).is_none() {
            return;
        }

        self.ensure_texture(device, upload);
        self.write_uniforms(queue, bounds, upload);

        if self.uploaded_sequence == Some(upload.sequence) {
            return;
        }

        let Some(texture) = self.texture.as_ref() else {
            return;
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            upload.pixels.as_ref(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(upload.width * upload.pixel_format.bytes_per_pixel() as u32),
                rows_per_image: Some(upload.height),
            },
            wgpu::Extent3d {
                width: upload.width,
                height: upload.height,
                depth_or_array_layers: 1,
            },
        );

        self.uploaded_sequence = Some(upload.sequence);
    }

    fn ensure_texture(&mut self, device: &wgpu::Device, upload: &PreviewUpload) {
        let reuse_texture = self.texture.as_ref().is_some_and(|texture| {
            texture.width == upload.width
                && texture.height == upload.height
                && texture.pixel_format == upload.pixel_format
        });

        if reuse_texture {
            return;
        }

        let Some(format) = texture_format(upload.pixel_format) else {
            return;
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("champions.preview.texture"),
            size: wgpu::Extent3d {
                width: upload.width,
                height: upload.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("champions.preview.bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
            ],
        });

        self.texture = Some(PreviewTexture {
            texture,
            _view: view,
            width: upload.width,
            height: upload.height,
            pixel_format: upload.pixel_format,
        });
        self.bind_group = Some(bind_group);
        self.uploaded_sequence = None;
    }

    fn write_uniforms(&self, queue: &wgpu::Queue, bounds: &Rectangle, upload: &PreviewUpload) {
        let bytes = preview_uniform_bytes(
            bounds.width.max(1.0),
            bounds.height.max(1.0),
            upload.width as f32,
            upload.height as f32,
        );

        queue.write_buffer(&self.uniform_buffer, 0, &bytes);
    }
}

fn preview_uniform_bytes(
    widget_width: f32,
    widget_height: f32,
    texture_width: f32,
    texture_height: f32,
) -> [u8; 16] {
    let values = [widget_width, widget_height, texture_width, texture_height];
    let mut bytes = [0u8; 16];

    for (index, value) in values.into_iter().enumerate() {
        let offset = index * 4;
        bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }

    bytes
}

fn texture_format(pixel_format: PixelFormat) -> Option<wgpu::TextureFormat> {
    match pixel_format {
        PixelFormat::Rgba8 => Some(wgpu::TextureFormat::Rgba8Unorm),
        PixelFormat::Bgra8 => Some(wgpu::TextureFormat::Bgra8Unorm),
        _ => None,
    }
}
