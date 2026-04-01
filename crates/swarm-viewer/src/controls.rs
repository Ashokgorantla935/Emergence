use winit::keyboard::KeyCode;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SimSpeed {
    Normal,
    Fast10x,
    Fast100x,
}

pub struct TimeControls {
    pub paused: bool,
    pub speed: SimSpeed,
    pub single_step: bool,
}

impl TimeControls {
    pub fn new() -> Self {
        TimeControls {
            paused: false,
            speed: SimSpeed::Normal,
            single_step: false,
        }
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Space => self.paused = !self.paused,
            KeyCode::Period => {
                if self.paused {
                    self.single_step = true;
                }
            }
            KeyCode::Digit1 => self.speed = SimSpeed::Normal,
            KeyCode::Digit2 => self.speed = SimSpeed::Fast10x,
            KeyCode::Digit3 => self.speed = SimSpeed::Fast100x,
            _ => {}
        }
    }

    pub fn ticks_this_frame(&mut self) -> u32 {
        if self.paused {
            if self.single_step {
                self.single_step = false;
                1
            } else {
                0
            }
        } else {
            match self.speed {
                SimSpeed::Normal => 1,
                SimSpeed::Fast10x => 10,
                SimSpeed::Fast100x => 100,
            }
        }
    }
}
