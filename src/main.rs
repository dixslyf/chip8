use std::{fs, path::PathBuf};

use chip8::Chip8;
use clap::Parser;
use pixels::{Pixels, SurfaceTexture};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[derive(Debug, Parser)]
struct Args {
    #[clap(parse(from_os_str))]
    rom: PathBuf,
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
    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::Resized(size) => {
                log::debug!("Resize window and surface");
                pixels.resize_surface(size.width, size.height)
            }
            WindowEvent::CloseRequested => {
                log::trace!("Close requested");
                *control_flow = ControlFlow::Exit
            }
            _ => {}
        },
        Event::MainEventsCleared => {
            window.request_redraw();
        }
        Event::RedrawRequested(_) => {
            for (&set, pixel) in chip8
                .display()
                .iter()
                .zip(pixels.get_frame().chunks_exact_mut(4))
            {
                let color = if set {
                    [0xff, 0xff, 0xff, 0xff]
                } else {
                    [0x00, 0x00, 0x00, 0xff]
                };
                pixel.copy_from_slice(&color);
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
