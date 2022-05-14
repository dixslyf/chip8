use pixels::{Pixels, SurfaceTexture};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

pub fn main() {
    let event_loop = EventLoop::new();
    let window = {
        let min_size = LogicalSize::new(chip8::WIDTH as f64, chip8::HEIGHT as f64);
        let scaled_size = LogicalSize::new(chip8::WIDTH as f64 * 3.0, chip8::HEIGHT as f64 * 3.0);
        WindowBuilder::new()
            .with_title("CHIP-8 Emulator")
            .with_inner_size(scaled_size)
            .with_min_inner_size(min_size)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(chip8::WIDTH, chip8::HEIGHT, surface_texture).unwrap()
    };

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::Resized(size) => pixels.resize_surface(size.width, size.height),
            WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
            _ => {}
        },
        Event::MainEventsCleared => {
            window.request_redraw();
        }
        Event::RedrawRequested(_) => {
            pixels.render().unwrap();
        }
        _ => {}
    });
}
