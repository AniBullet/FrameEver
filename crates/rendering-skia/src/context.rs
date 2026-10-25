use crate::SkiaRenderingError;
use skia_safe::{Color, Surface, gpu::DirectContext, surfaces};

pub struct SkiaRenderContext {
    _phantom: std::marker::PhantomData<()>,
}

impl SkiaRenderContext {
    pub fn new() -> Result<Self, SkiaRenderingError> {
        tracing::info!("Creating Skia context (CPU backend)");
        Ok(Self {
            _phantom: std::marker::PhantomData,
        })
    }

    pub fn create_surface(
        &mut self,
        width: u32,
        height: u32,
    ) -> Result<Surface, SkiaRenderingError> {
        if width == 0 || height == 0 {
            return Err(SkiaRenderingError::InvalidDimensions(format!(
                "Invalid surface dimensions: {width}x{height}"
            )));
        }

        // Fallback to CPU surface
        tracing::debug!("Creating CPU surface with dimensions {}x{}", width, height);
        surfaces::raster_n32_premul((width as i32, height as i32)).ok_or_else(|| {
            SkiaRenderingError::SurfaceCreationFailed(format!(
                "Failed to create {width}x{height} surface"
            ))
        })
    }

    pub fn flush(&mut self) {}

    /// Test rendering - draws a simple gradient
    pub fn test_render(&mut self, width: u32, height: u32) -> Result<Vec<u8>, SkiaRenderingError> {
        let mut surface = self.create_surface(width, height)?;
        let canvas = surface.canvas();

        // Clear to white
        canvas.clear(Color::WHITE);

        // Draw a simple gradient
        use skia_safe::{Paint, Point, Shader, TileMode};

        let mut paint = Paint::default();
        let colors = vec![Color::from_rgb(255, 0, 0), Color::from_rgb(0, 0, 255)];
        let shader = Shader::linear_gradient(
            (
                Point::new(0.0, 0.0),
                Point::new(width as f32, height as f32),
            ),
            colors.as_slice(),
            None,
            TileMode::Clamp,
            None,
            None,
        )
        .unwrap();
        paint.set_shader(shader);

        canvas.draw_rect(
            skia_safe::Rect::from_xywh(0.0, 0.0, width as f32, height as f32),
            &paint,
        );

        // Draw a circle
        let mut circle_paint = Paint::default();
        circle_paint.set_color(Color::from_rgb(0, 255, 0));
        circle_paint.set_anti_alias(true);
        canvas.draw_circle(
            Point::new(width as f32 / 2.0, height as f32 / 2.0),
            width.min(height) as f32 * 0.3,
            &circle_paint,
        );

        // Flush GPU operations if using GPU backend
        self.flush();

        // Read pixels
        let image_info = surface.image_info();
        let mut pixels = vec![0u8; (width * height * 4) as usize];

        if !surface.read_pixels(&image_info, &mut pixels, (width * 4) as usize, (0, 0)) {
            return Err(SkiaRenderingError::ReadPixelsFailed);
        }

        Ok(pixels)
    }

    pub fn is_gpu_accelerated(&self) -> bool {
        false
    }

    pub fn direct_context(&mut self) -> Option<&mut DirectContext> {
        None
    }
}
