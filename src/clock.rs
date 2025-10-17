use std::time;

#[derive(Debug)]
pub struct Clock {
    label: &'static str,
    current_time: time::Instant,
    accumulator: time::Duration,
    target_dt: time::Duration,
    paused: bool,
}

impl Clock {
    pub fn new(label: &'static str, target_dt: time::Duration) -> Self {
        Self {
            label,
            current_time: time::Instant::now(),
            accumulator: time::Duration::ZERO,
            target_dt,
            paused: false,
        }
    }

    pub fn tick(&mut self) {
        if !self.paused {
            let new_time = time::Instant::now();
            self.accumulator += new_time - self.current_time;
            self.current_time = new_time;
            log::trace!("{:?}", self);
        }
    }

    pub fn should_update(&mut self) -> bool {
        if !self.paused && self.accumulator >= self.target_dt {
            self.accumulator -= self.target_dt;
            true
        } else {
            false
        }
    }

    pub fn pause(&mut self) {
        log::trace!("{:?}", self);
        self.accumulator += time::Instant::now() - self.current_time;
        self.paused = true;
        log::trace!("{:?}", self);
    }

    pub fn unpause(&mut self) {
        self.current_time = time::Instant::now();
        self.paused = false;
        log::trace!("{:?}", self);
    }
}
