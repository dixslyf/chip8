use std::{fs, path::PathBuf, sync::Arc, time};

use chip8::{Chip8, Clock, Quirks};
use clap::{ArgAction, Parser, ValueEnum};
use pixels::{Pixels, SurfaceTexture};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    error::EventLoopError,
    event::{ElementState, WindowEvent},
    event_loop::EventLoop,
    keyboard::Key,
    window::Window,
};

#[derive(Debug, ValueEnum, Copy, Clone)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Parser)]
struct Args {
    rom: PathBuf,

    #[arg(short, long, default_value_t = 500.0)]
    cpu_frequency: f64,

    #[arg(short, long, default_value_t = 60.0)]
    timers_frequency: f64,

    #[arg(short, long, default_value_t = 440.0)]
    sound_frequency: f32,

    #[arg(long, default_value = "info", value_enum)]
    log_level: LogLevel,

    // Ref: https://github.com/clap-rs/clap/issues/1649#issuecomment-2144879038
    #[arg(
        long,
        action = ArgAction::Set,
        default_value_t = true,
        default_missing_value = "true",
        num_args = 0..=1,
        require_equals = false,
    )]
    quirk_draw_wrap: bool,

    #[arg(
        long,
        action = ArgAction::Set,
        default_value_t = false,
        default_missing_value = "false",
        num_args = 0..=1,
        require_equals = false,
    )]
    quirk_vf_reset: bool,
}

struct App {
    chip8: Chip8,
    cpu_clock: Clock,
    timers_clock: Clock,
    audio_sink: rodio::Sink,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
}

impl App {
    pub fn new(
        chip8: Chip8,
        cpu_freq: f64,
        timers_freq: f64,
        sound_freq: f32,
        mixer: &rodio::mixer::Mixer,
    ) -> Self {
        log::debug!("Initializing audio sink");
        let audio_sink = rodio::Sink::connect_new(mixer);
        let sine_wave = rodio::source::SineWave::new(sound_freq);
        audio_sink.append(sine_wave);
        audio_sink.pause();

        let cpu_clock = Clock::new("cpu", time::Duration::from_secs_f64(1.0 / cpu_freq));
        let timers_clock = Clock::new("timers", time::Duration::from_secs_f64(1.0 / timers_freq));

        Self {
            chip8,
            cpu_clock,
            timers_clock,
            audio_sink,
            window: None,
            pixels: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        log::debug!("Creating window");
        let window = {
            let min_size =
                LogicalSize::new(chip8::DISPLAY_WIDTH as f64, chip8::DISPLAY_HEIGHT as f64);
            let scaled_size = LogicalSize::new(
                chip8::DISPLAY_WIDTH as f64 * 3.0,
                chip8::DISPLAY_HEIGHT as f64 * 3.0,
            );
            let win_attrs = Window::default_attributes()
                .with_title("CHIP-8 Emulator")
                .with_inner_size(scaled_size)
                .with_min_inner_size(min_size);
            Arc::new(event_loop.create_window(win_attrs).unwrap())
        };
        self.window = Some(window.clone());

        log::debug!("Initializing pixel buffer");
        let pixels = {
            let window_size = window.inner_size();
            let surface_texture =
                SurfaceTexture::new(window_size.width, window_size.height, window.clone());
            Pixels::new(
                chip8::DISPLAY_WIDTH as u32,
                chip8::DISPLAY_HEIGHT as u32,
                surface_texture,
            )
            .unwrap()
        };
        self.pixels = Some(pixels);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) => {
                log::debug!("Received window resize request");
                if let Err(err) = self
                    .pixels
                    .as_mut()
                    .unwrap()
                    .resize_surface(size.width, size.height)
                {
                    log::error!("Failed to resize surface: {}", err);
                    event_loop.exit();
                }

                if let Err(err) = self.pixels.as_ref().unwrap().render() {
                    log::error!("Failed to render pixels: {}", err);
                    event_loop.exit();
                }
            }
            WindowEvent::CloseRequested => {
                log::debug!("Received window close request");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.timers_clock.tick();
                self.cpu_clock.tick();

                while self.timers_clock.should_update() {
                    self.chip8.update_timers();
                }

                if self.chip8.should_beep() && self.audio_sink.is_paused() {
                    self.audio_sink.play();
                    log::debug!("Start beep")
                } else if !self.chip8.should_beep() && !self.audio_sink.is_paused() {
                    self.audio_sink.pause();
                    log::debug!("Stop beep")
                }

                while self.cpu_clock.should_update() {
                    self.chip8.execute_cycle();

                    if self.chip8.waiting_for_keypress() {
                        self.cpu_clock.pause();
                    }

                    // Update pixels
                    if self.chip8.should_redraw() {
                        for (dpx, wpx) in self.chip8.display().iter().zip(
                            self.pixels
                                .as_mut()
                                .unwrap()
                                .frame_mut()
                                .chunks_exact_mut(4),
                        ) {
                            let color = if *dpx {
                                [0xff, 0xff, 0xff, 0xff]
                            } else {
                                [0x00, 0x00, 0x00, 0xff]
                            };
                            wpx.copy_from_slice(&color);
                        }
                    }
                }

                match self.pixels.as_ref().unwrap().render() {
                    Ok(_) => self.window.as_ref().unwrap().request_redraw(),
                    Err(err) => {
                        log::error!("Failed to render pixels: {}", err);
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                log::debug!("Received keyboard input event: {:?}", event);
                let key = match event.logical_key {
                    Key::Character(s) => match s.as_ref() {
                        "1" => chip8::Key::Key1,
                        "2" => chip8::Key::Key2,
                        "3" => chip8::Key::Key3,
                        "4" => chip8::Key::KeyC,
                        "q" => chip8::Key::Key4,
                        "w" => chip8::Key::Key5,
                        "e" => chip8::Key::Key6,
                        "r" => chip8::Key::KeyD,
                        "a" => chip8::Key::Key7,
                        "s" => chip8::Key::Key8,
                        "d" => chip8::Key::Key9,
                        "f" => chip8::Key::KeyE,
                        "z" => chip8::Key::KeyA,
                        "x" => chip8::Key::Key0,
                        "c" => chip8::Key::KeyB,
                        "v" => chip8::Key::KeyF,
                        _ => return,
                    },
                    _ => return,
                };

                if self.chip8.waiting_for_keypress() {
                    self.cpu_clock.unpause();
                }

                let input = match event.state {
                    ElementState::Pressed => chip8::Input::Down(key),
                    ElementState::Released => chip8::Input::Up(key),
                };
                self.chip8.register_input(input);
            }
            _ => {}
        }
    }
}

pub fn main() -> Result<(), EventLoopError> {
    let args = Args::parse();
    init_logging(args.log_level);

    log::debug!("Initializing event loop");
    let event_loop = EventLoop::new()?;

    log::debug!("Initializing audio output stream handle");
    let stream_handle = rodio::OutputStreamBuilder::open_default_stream().unwrap();

    log::debug!("Reading ROM from filesystem");
    let rom = fs::read(args.rom).unwrap();

    log::debug!("Initializing core emulator");
    let mut chip8 = Chip8::new(Quirks {
        draw_wrap: args.quirk_draw_wrap,
        vf_reset: args.quirk_vf_reset,
    });
    chip8.load(&rom);

    log::debug!("Initializing application");
    let mut app = App::new(
        chip8,
        args.cpu_frequency,
        args.timers_frequency,
        args.sound_frequency,
        stream_handle.mixer(),
    );
    event_loop.run_app(&mut app)
}

fn init_logging(log_level: LogLevel) {
    let level_filter = match log_level {
        LogLevel::Error => log::LevelFilter::Error,
        LogLevel::Warn => log::LevelFilter::Warn,
        LogLevel::Info => log::LevelFilter::Info,
        LogLevel::Debug => log::LevelFilter::Debug,
        LogLevel::Trace => log::LevelFilter::Trace,
    };

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
        .level(level_filter)
        // These are too noisy.
        .level_for("wgpu_core", log::LevelFilter::Warn)
        .level_for("wgpu_hal", log::LevelFilter::Warn)
        .level_for("naga", log::LevelFilter::Warn)
        .level_for("mio", log::LevelFilter::Warn)
        .level_for("calloop", log::LevelFilter::Warn)
        .chain(std::io::stderr())
        .apply()
        .unwrap();

    log::trace!("Initialize logging");
}
