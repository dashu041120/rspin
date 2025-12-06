// GPU-accelerated rendering using wgpu with raw Wayland surface
// This renderer integrates with layer-shell surfaces without winit

use crate::image_loader::ImageData;
use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::ptr::NonNull;
use wgpu::rwh::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use wgpu::util::DeviceExt;

// Maximum surface size to prevent GPU memory issues
const MAX_SURFACE_SIZE: u32 = 4096;
const MAX_TEXTURE_SIZE: u32 = 8192;

pub struct WgpuRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    texture: Option<wgpu::Texture>,
    texture_bind_group: Option<wgpu::BindGroup>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    max_texture_size: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-1.0, -1.0, 0.0],
        tex_coords: [0.0, 1.0],
    }, // Bottom-left
    Vertex {
        position: [1.0, -1.0, 0.0],
        tex_coords: [1.0, 1.0],
    }, // Bottom-right
    Vertex {
        position: [1.0, 1.0, 0.0],
        tex_coords: [1.0, 0.0],
    }, // Top-right
    Vertex {
        position: [-1.0, 1.0, 0.0],
        tex_coords: [0.0, 0.0],
    }, // Top-left
];

const INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    opacity: f32,
    _padding: [f32; 3],
}

impl WgpuRenderer {
    /// Create a new WgpuRenderer from raw Wayland display and surface pointers
    ///
    /// # Safety
    /// - `display_ptr` must be a valid pointer to a wl_display
    /// - `surface_ptr` must be a valid pointer to a wl_surface
    /// - The display and surface must remain valid for the lifetime of the renderer
    pub fn new(
        display_ptr: *mut std::ffi::c_void,
        surface_ptr: *mut std::ffi::c_void,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        info!("Initializing wgpu renderer with size {}x{}", width, height);

        let display_non_null = NonNull::new(display_ptr)
            .context("Display pointer is null")?;
        let surface_non_null = NonNull::new(surface_ptr)
            .context("Surface pointer is null")?;

        let raw_display_handle =
            RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display_non_null));
        let raw_window_handle = 
            RawWindowHandle::Wayland(WaylandWindowHandle::new(surface_non_null));

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            ..Default::default()
        });

        // Create surface from raw handles
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle,
                raw_window_handle,
            })?
        };

        pollster::block_on(Self::init_async(surface, instance, width, height))
    }

    async fn init_async(
        surface: wgpu::Surface<'static>,
        instance: wgpu::Instance,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("Failed to find an appropriate adapter")?;

        info!("Using adapter: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .context("Failed to create device")?;

        let surface_caps = surface.get_capabilities(&adapter);
        debug!("Surface capabilities: {:?}", surface_caps);

        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        // Select alpha mode - prefer PreMultiplied for transparency
        let alpha_mode = if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else {
            surface_caps.alpha_modes[0]
        };
        info!("Using alpha mode: {:?}", alpha_mode);

        // Get device limits
        let max_texture_size = adapter
            .limits()
            .max_texture_dimension_2d
            .min(MAX_TEXTURE_SIZE);
        info!("Max texture size: {}", max_texture_size);

        // Clamp dimensions to safe limits
        let safe_width = width.max(1).min(MAX_SURFACE_SIZE).min(max_texture_size);
        let safe_height = height.max(1).min(MAX_SURFACE_SIZE).min(max_texture_size);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: safe_width,
            height: safe_height,
            present_mode: wgpu::PresentMode::Fifo, // VSync, stable
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Texture bind group layout
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        // Uniform bind group layout
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("uniform_bind_group_layout"),
            });

        // Uniform buffer
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[Uniforms {
                opacity: 1.0,
                _padding: [0.0; 3],
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&texture_bind_group_layout, &uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            render_pipeline,
            texture: None,
            texture_bind_group: None,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            uniform_bind_group,
            width: safe_width,
            height: safe_height,
            max_texture_size,
        })
    }

    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        if new_width > 0 && new_height > 0 {
            // Clamp to safe limits to prevent broken pipe
            let safe_width = new_width.min(MAX_SURFACE_SIZE).min(self.max_texture_size);
            let safe_height = new_height.min(MAX_SURFACE_SIZE).min(self.max_texture_size);

            if safe_width != self.width || safe_height != self.height {
                self.width = safe_width;
                self.height = safe_height;
                self.config.width = safe_width;
                self.config.height = safe_height;

                // Reconfigure surface with new size
                self.surface.configure(&self.device, &self.config);
                debug!("Resized to {}x{}", safe_width, safe_height);
            }
        }
    }

    pub fn upload_texture(&mut self, image: &ImageData) -> Result<()> {
        // Clamp texture size to device limits
        let tex_width = image.width.min(MAX_TEXTURE_SIZE).min(self.max_texture_size);
        let tex_height = image.height.min(MAX_TEXTURE_SIZE).min(self.max_texture_size);

        debug!(
            "Uploading texture: {}x{} (clamped from {}x{})",
            tex_width, tex_height, image.width, image.height
        );

        let texture_size = wgpu::Extent3d {
            width: tex_width,
            height: tex_height,
            depth_or_array_layers: 1,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("image_texture"),
            view_formats: &[],
        });

        // Select appropriate mipmap level if texture was clamped
        let (source_width, source_height, source_data) =
            if tex_width < image.width || tex_height < image.height {
                // Use mipmap to reduce upload size
                let scale_ratio = (tex_width as f32 / image.width as f32)
                    .min(tex_height as f32 / image.height as f32);

                let mip_level = if !image.mipmaps.is_empty() && scale_ratio < 0.5 {
                    let ideal_level = (1.0 / scale_ratio).log2().floor() as usize;
                    ideal_level.min(image.mipmaps.len())
                } else {
                    0
                };

                if mip_level > 0 && mip_level <= image.mipmaps.len() {
                    let mipmap = &image.mipmaps[mip_level - 1];
                    debug!(
                        "Using mipmap level {} ({}x{})",
                        mip_level, mipmap.width, mipmap.height
                    );
                    (mipmap.width, mipmap.height, &mipmap.data)
                } else {
                    (image.width, image.height, &image.rgba_data)
                }
            } else {
                (image.width, image.height, &image.rgba_data)
            };

        // Convert BGRA to RGBA for wgpu
        let mut rgba_data = source_data.clone();
        for pixel in rgba_data.chunks_exact_mut(4) {
            pixel.swap(0, 2); // Swap B and R back to RGBA
        }

        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * source_width),
                rows_per_image: Some(source_height),
            },
            texture_size,
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let texture_bind_group_layout = &self.render_pipeline.get_bind_group_layout(0);

        let texture_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("texture_bind_group"),
        });

        self.texture = Some(texture);
        self.texture_bind_group = Some(texture_bind_group);

        Ok(())
    }

    pub fn update_opacity(&mut self, opacity: f32) {
        let uniforms = Uniforms {
            opacity,
            _padding: [0.0; 3],
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Render a frame and return whether successful
    pub fn render(&mut self) -> Result<bool> {
        if self.texture_bind_group.is_none() {
            return Ok(false); // No texture uploaded yet
        }

        let output = match self.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Timeout) => {
                debug!("Surface timeout, skipping frame");
                return Ok(false);
            }
            Err(wgpu::SurfaceError::Outdated) => {
                debug!("Surface outdated, reconfiguring");
                self.surface.configure(&self.device, &self.config);
                return Ok(false);
            }
            Err(wgpu::SurfaceError::Lost) => {
                debug!("Surface lost, reconfiguring");
                self.surface.configure(&self.device, &self.config);
                return Ok(false);
            }
            Err(e) => {
                warn!("Surface error: {:?}", e);
                return Err(e.into());
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, self.texture_bind_group.as_ref().unwrap(), &[]);
            render_pass.set_bind_group(1, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(true)
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}
