use std::{fs, path::PathBuf, time};

use chip8::Chip8;
use clap::Parser;
use pixels::{Pixels, SurfaceTexture};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

struct TimeContext {
    current_time: time::Instant,
    frame_time: time::Duration,
    accumulator: time::Duration,
    target_dt: time::Duration,
}

impl TimeContext {
    pub fn new(target_dt: time::Duration) -> Self {
        Self {
            current_time: time::Instant::now(),
            frame_time: time::Duration::ZERO,
            accumulator: time::Duration::ZERO,
            target_dt,
        }
    }

    pub fn tick(&mut self) {
        let new_time = time::Instant::now();
        self.frame_time = new_time - self.current_time;
        self.current_time = new_time;
        self.accumulator += self.frame_time;
    }

    pub fn should_update(&mut self) -> bool {
        if self.accumulator >= self.target_dt {
            self.accumulator -= self.target_dt;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Parser)]
struct Args {
    rom: PathBuf,
    #[arg(short, long, default_value_t = 500.0)]
    frequency: f64,
}

pub fn main() {
    init_logging();
    let args = Args::parse();

    let rom = fs::read(args.rom).unwrap();
    let mut chip8 = Chip8::new();
    chip8.load(&rom);

    log::trace!("Initialize event loop");
    let event_loop = EventLoop::new();

    log::trace!("Initialize winit window");
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

    log::trace!("Initialize pixel buffer");
    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(chip8::WIDTH as u32, chip8::HEIGHT as u32, surface_texture).unwrap()
    };

    log::trace!("Begin event loop");
    let mut time_ctx = TimeContext::new(time::Duration::from_secs_f64(1.0 / args.frequency));
    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::Resized(size) => {
                log::debug!("Resize window and surface");
                pixels.resize_surface(size.width, size.height);
                pixels.render().unwrap();
            }
            WindowEvent::CloseRequested => {
                log::trace!("Close requested");
                *control_flow = ControlFlow::Exit
            }
            WindowEvent::KeyboardInput { input, .. } => {
                let key = match input.scancode {
                    2 => chip8::Key::Key1,  // 1
                    3 => chip8::Key::Key2,  // 2
                    4 => chip8::Key::Key3,  // 3
                    5 => chip8::Key::KeyC,  // 4
                    16 => chip8::Key::Key4, // Q
                    17 => chip8::Key::Key5, // W
                    18 => chip8::Key::Key6, // E
                    19 => chip8::Key::KeyD, // R
                    30 => chip8::Key::Key7, // A
                    31 => chip8::Key::Key8, // S
                    32 => chip8::Key::Key9, // D
                    33 => chip8::Key::KeyE, // F
                    44 => chip8::Key::KeyA, // Z
                    45 => chip8::Key::Key0, // X
                    46 => chip8::Key::KeyB, // C
                    47 => chip8::Key::KeyF, // V
                    _ => return,
                };

                let input = match input.state {
                    winit::event::ElementState::Pressed => chip8::Input::Down(key),
                    winit::event::ElementState::Released => chip8::Input::Up(key),
                };
                chip8.register_input(input);
            }
            _ => {}
        },
        Event::MainEventsCleared => {
            time_ctx.tick();

            while time_ctx.should_update() {
                chip8.execute_cycle();
                if chip8.should_redraw() {
                    window.request_redraw();
                }
            }
        }
        Event::RedrawRequested(_) => {
            for (dpx, wpx) in chip8
                .display()
                .iter()
                .zip(pixels.get_frame().chunks_exact_mut(4))
            {
                let color = if *dpx {
                    [0xff, 0xff, 0xff, 0xff]
                } else {
                    [0x00, 0x00, 0x00, 0xff]
                };
                wpx.copy_from_slice(&color);
            }
            pixels.render().unwrap();
        }
        _ => {}
    });
}

fn init_logging() {
    let color_config = fern::colors::ColoredLevelConfig::new()
        .info(fern::colors::Color::Green)
        .debug(fern::colors::Color::Magenta)
        .trace(fern::colors::Color::Blue);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            if let Some(file_path) = record.file() {
                let file_path = std::path::Path::new(file_path);
                if let Some(line) = record.line() {
                    out.finish(format_args!(
                        "\x1B[1;{color}m{level}\x1B[0m \x1B[{color}m{target} {file_name}:{line}\x1B[0m {message}",
                        color = color_config.get_color(&record.level()).to_fg_str(),
                        level = record.level(),
                        target = record.target(),
                        file_name = file_path.file_name().unwrap().to_str().unwrap(),
                        line = line,
                        message = message,
                    ));
                }
            } else {
                out.finish(format_args!(
                    "\x1B[1;{color}m{level}\x1B[0m \x1B[{color}m{target}\x1B[0m {message}",
                    color = color_config.get_color(&record.level()).to_fg_str(),
                    level = record.level(),
                    target = record.target(),
                    message = message,
                ));
            }
        })
        .level(log::LevelFilter::Trace)
        .level_for("wgpu_core", log::LevelFilter::Warn)
        .level_for("wgpu_hal", log::LevelFilter::Warn)
        .level_for("naga", log::LevelFilter::Warn)
        .level_for("mio", log::LevelFilter::Warn)
        .chain(std::io::stderr())
        .apply()
        .unwrap();

    log::trace!("Initialize logging");
}
