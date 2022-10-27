use std::sync::mpsc;

use arrayvec::ArrayVec;
use bitvec::{array::BitArray, order::Msb0, view::BitView, BitArr};
use rand::Rng;

pub const WIDTH: usize = 64;
pub const HEIGHT: usize = 32;

const REGISTER_COUNT: usize = 16;
const STACK_SIZE: usize = 16;
const MEMORY_SIZE: usize = 4096;
const START_ROM_ADDRESS: usize = 0x200;
const MAX_ROM_SIZE: usize = MEMORY_SIZE - START_ROM_ADDRESS;
const FONTSET: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Event {
    Tick,
    Input(Input),
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Input {
    Up(Key),
    Down(Key),
}

#[cfg_attr(rustfmt, rustfmt_skip)]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Key {
    Key1, Key2, Key3, KeyC,
    Key4, Key5, Key6, KeyD,
    Key7, Key8, Key9, KeyE,
    KeyA, Key0, KeyB, KeyF
}

pub struct Chip8 {
    pc: u16,                                     // 12-bit program counter
    i: u16,                                      // 12-bit address register
    v: [u8; REGISTER_COUNT],                     // 16 8-bit data registers
    stack: ArrayVec<u16, STACK_SIZE>,            // 16-level stack
    memory: [u8; MEMORY_SIZE],                   // 4KB memory
    display: BitArr!(for WIDTH * HEIGHT, in u8), // 64 * 32 monochrome display
    should_redraw: bool,
    dt: u8,
    st: u8,
    keypad: BitArr!(for 16, in u8), // 16-key keypad
    event_tx: mpsc::Sender<Event>,
    event_rx: mpsc::Receiver<Event>,
    display_tx: mpsc::SyncSender<Vec<u8>>,
}

impl Chip8 {
    pub fn new(display_tx: mpsc::SyncSender<Vec<u8>>) -> Self {
        let mut memory = [0; MEMORY_SIZE];
        memory[..FONTSET.len()].copy_from_slice(&FONTSET);

        let (event_tx, event_rx) = mpsc::channel();

        Self {
            pc: START_ROM_ADDRESS as u16,
            i: 0,
            v: [0; REGISTER_COUNT],
            stack: ArrayVec::new(),
            memory,
            display: BitArray::ZERO,
            should_redraw: false,
            dt: 0,
            st: 0,
            keypad: BitArray::ZERO,
            event_tx,
            event_rx,
            display_tx,
        }
    }

    pub fn load(&mut self, rom: &[u8]) {
        self.pc = START_ROM_ADDRESS as u16;

        let rom_size = rom.len();
        if rom_size > MAX_ROM_SIZE {
            log::warn!(
                "ROM of size {} bytes is larger than the max ROM size of {} bytes. ROM will be truncated!",
                rom_size, MAX_ROM_SIZE
            );
            self.memory[START_ROM_ADDRESS..].copy_from_slice(&rom[..MAX_ROM_SIZE]);
        } else {
            self.memory[START_ROM_ADDRESS..(START_ROM_ADDRESS + rom_size)].copy_from_slice(&rom);
        };
        log::info!("Loaded ROM of size {} bytes", MAX_ROM_SIZE.min(rom_size));
    }

    pub fn event_tx(&self) -> &mpsc::Sender<Event> {
        &self.event_tx
    }

    fn register_input(&mut self, input: Input) {
        match input {
            Input::Up(key) => *self.keypad.get_mut(key as usize).unwrap() = false,
            Input::Down(key) => *self.keypad.get_mut(key as usize).unwrap() = true,
        };
    }

    pub fn run_event_loop(&mut self) {
        loop {
            match self.event_rx.recv() {
                Ok(ev) => match ev {
                    Event::Tick => {
                        self.execute_cycle();
                        if self.should_redraw() {
                            match self.display_tx.send(self.display.to_bitvec().into_vec()) {
                                Ok(_) => log::trace!("Send display data"),
                                Err(_) => log::error!("Display channel disconnected!"),
                            }
                        }
                    }
                    Event::Input(input) => self.register_input(input),
                },
                Err(_) => log::error!("Event channel disconnected!"),
            }
        }
    }

    pub fn execute_cycle(&mut self) {
        let opcode = self.fetch_opcode();
        self.execute_opcode(opcode);

        // Redraw only if the opcode is one of the display opcodes
        self.should_redraw = opcode == 0x00E0 || opcode & 0xF000 == 0xD000;

        self.dt = self.dt.saturating_sub(1);

        if self.st == 1 {
            // TODO: beep
            log::info!("Beep!")
        }
        self.st = self.st.saturating_sub(1);
    }

    pub fn should_redraw(&self) -> bool {
        self.should_redraw
    }

    pub fn fetch_opcode(&self) -> u16 {
        let pc = self.pc as usize;
        let op1 = self.memory[pc];
        let op2 = self.memory[pc + 1];
        (op1 as u16) << 8 | op2 as u16
    }

    pub fn execute_opcode(&mut self, opcode: u16) {
        // Break into nibbles
        let nibbles = [
            ((opcode & 0xF000) >> 12) as u8,
            ((opcode & 0x0F00) >> 8) as u8,
            ((opcode & 0x00F0) >> 4) as u8,
            (opcode & 0x000F) as u8,
        ];

        let nnn = opcode & 0x0FFF;
        let kk = (opcode & 0x00FF) as u8;
        let n = (opcode & 0x000F) as u8;
        let x = ((opcode & 0x0F00) >> 8) as u8;
        let y = ((opcode & 0x00F0) >> 4) as u8;

        log::trace!("Execute opcode: {:#06X}", opcode);
        match nibbles {
            [0x0, 0x0, 0xE, 0x0] => self.op_00e0(),
            [0x0, 0x0, 0xE, 0xE] => self.op_00ee(),
            [0x0, _, _, _] => self.op_0nnn(nnn),
            [0x1, _, _, _] => self.op_1nnn(nnn),
            [0x2, _, _, _] => self.op_2nnn(nnn),
            [0x3, _, _, _] => self.op_3xnn(x, kk),
            [0x4, _, _, _] => self.op_4xnn(x, kk),
            [0x5, _, _, 0x0] => self.op_5xy0(x, y),
            [0x6, _, _, _] => self.op_6xnn(x, kk),
            [0x7, _, _, _] => self.op_7xnn(x, kk),
            [0x8, _, _, 0x0] => self.op_8xy0(x, y),
            [0x8, _, _, 0x1] => self.op_8xy1(x, y),
            [0x8, _, _, 0x2] => self.op_8xy2(x, y),
            [0x8, _, _, 0x3] => self.op_8xy3(x, y),
            [0x8, _, _, 0x4] => self.op_8xy4(x, y),
            [0x8, _, _, 0x5] => self.op_8xy5(x, y),
            [0x8, _, _, 0x6] => self.op_8xy6(x, y),
            [0x8, _, _, 0x7] => self.op_8xy7(x, y),
            [0x8, _, _, 0xe] => self.op_8xye(x, y),
            [0x9, _, _, 0x0] => self.op_9xy0(x, y),
            [0xA, _, _, _] => self.op_annn(nnn),
            [0xB, _, _, _] => self.op_bnnn(nnn),
            [0xC, _, _, _] => self.op_cxnn(x, kk),
            [0xD, _, _, _] => self.op_dxyn(x, y, n),
            [0xE, _, 0x9, 0xE] => self.op_ex9e(x),
            [0xE, _, 0xA, 0x1] => self.op_exa1(x),
            [0xF, _, 0x0, 0x7] => self.op_fx07(x),
            [0xF, _, 0x0, 0xA] => self.op_fx0a(x),
            [0xF, _, 0x1, 0x5] => self.op_fx15(x),
            [0xF, _, 0x1, 0x8] => self.op_fx18(x),
            [0xF, _, 0x1, 0xE] => self.op_fx1e(x),
            [0xF, _, 0x2, 0x9] => self.op_fx29(x),
            [0xF, _, 0x3, 0x3] => self.op_fx33(x),
            [0xF, _, 0x5, 0x5] => self.op_fx55(x),
            [0xF, _, 0x6, 0x5] => self.op_fx65(x),
            _ => panic!("Unknown opcode {:#06X}", opcode),
        }
    }

    /// Calls the machine code routine at address `nnn`.
    /// This opcode is unimplemented. Programs that use this opcode are written specifically for
    /// the hardware that the CHIP-8 interpreter is running on.
    fn op_0nnn(&mut self, _nnn: u16) {
        log::warn!("Opcode 0NNN is unimplemented.");
        self.pc += 2;
    }

    /// Clears the display.
    fn op_00e0(&mut self) {
        self.display.fill(false);
        self.pc += 2;
    }

    /// Returns from a subroutine.
    /// The program counter is set to the address popped from the stack.
    fn op_00ee(&mut self) {
        self.pc = self.stack.pop().unwrap();
    }

    /// Jumps to address `nnn`.
    /// The program counter is set to the address `nnn`.
    fn op_1nnn(&mut self, nnn: u16) {
        self.pc = nnn;
    }

    /// Executes the subroutine starting at address `nnn`.
    /// The program counter for the next instruction is pushed onto the stack, and then set to `nnn`.
    fn op_2nnn(&mut self, nnn: u16) {
        self.stack.push(self.pc + 2);
        self.pc = nnn;
    }

    /// Skips the next instruction if `vx` equals `kk`.
    fn op_3xnn(&mut self, x: u8, kk: u8) {
        if self.v[x as usize] == kk {
            self.pc += 4;
        } else {
            self.pc += 2;
        }
    }

    /// Skips the next instruction if `vx` does not equal `kk`.
    fn op_4xnn(&mut self, x: u8, kk: u8) {
        if self.v[x as usize] != kk {
            self.pc += 4;
        } else {
            self.pc += 2;
        }
    }

    /// Skips the next instruction if `vx` equals `vy`.
    fn op_5xy0(&mut self, x: u8, y: u8) {
        if self.v[x as usize] == self.v[y as usize] {
            self.pc += 4;
        } else {
            self.pc += 2;
        }
    }

    /// Sets `vx` to `kk`.
    fn op_6xnn(&mut self, x: u8, kk: u8) {
        self.v[x as usize] = kk;
        self.pc += 2;
    }

    /// Adds `kk` to `vx`.
    fn op_7xnn(&mut self, x: u8, kk: u8) {
        self.v[x as usize] = self.v[x as usize].wrapping_add(kk);
        self.pc += 2;
    }

    /// Sets `vx` to `vy`.
    fn op_8xy0(&mut self, x: u8, y: u8) {
        self.v[x as usize] = self.v[y as usize];
        self.pc += 2;
    }

    /// Sets `vx` to the bitwise OR of `vx` and `vy`.
    fn op_8xy1(&mut self, x: u8, y: u8) {
        self.v[x as usize] |= self.v[y as usize];
        self.pc += 2;
    }

    /// Sets `vx` to the bitwise AND of `vx` and `vy`.
    fn op_8xy2(&mut self, x: u8, y: u8) {
        self.v[x as usize] &= self.v[y as usize];
        self.pc += 2;
    }

    /// Sets `vx` to the XOR of `vx` and `vy`.
    fn op_8xy3(&mut self, x: u8, y: u8) {
        self.v[x as usize] ^= self.v[y as usize];
        self.pc += 2;
    }

    /// Adds `vy` to `vx`. `vf` is set to `1` if a carry occurs, and `0` if not.
    fn op_8xy4(&mut self, x: u8, y: u8) {
        let (sum, carry) = self.v[x as usize].overflowing_add(self.v[y as usize]);
        self.v[x as usize] = sum;
        self.v[0xF] = carry as u8;
        self.pc += 2;
    }

    /// Subtracts `vy` from `vx`. `vf` is set to `0` if a borrow occurs, and `1` if not.
    fn op_8xy5(&mut self, x: u8, y: u8) {
        let (diff, borrow) = self.v[x as usize].overflowing_sub(self.v[y as usize]);
        self.v[x as usize] = diff;
        self.v[0xF] = !borrow as u8;
        self.pc += 2;
    }

    /// Sets `vx` to `vy` shifted right by one bit. `vf` is set to the least significant bit of `vy`
    /// prior to the shift.
    fn op_8xy6(&mut self, x: u8, y: u8) {
        self.v[0xF] = self.v[y as usize] & 0x1;
        self.v[x as usize] = self.v[y as usize] >> 1;
        self.pc += 2;
    }

    /// Sets `vx` to `vy - vx`. `vf` is set to `0` if a borrow occurs, and `1` if not.
    fn op_8xy7(&mut self, x: u8, y: u8) {
        let (diff, borrow) = self.v[y as usize].overflowing_sub(self.v[x as usize]);
        self.v[x as usize] = diff;
        self.v[0xF] = !borrow as u8;
        self.pc += 2;
    }

    /// Sets `vx` to `vy` shifted left by one bit. `vf` is set to the most significant bit of `vy`
    /// prior to the shift.
    fn op_8xye(&mut self, x: u8, y: u8) {
        self.v[0xF] = self.v[y as usize] & 0b1000_0000;
        self.v[x as usize] = self.v[y as usize] << 1;
        self.pc += 2;
    }

    /// Skips the next instruction if `vx` does not equal `vy`.
    fn op_9xy0(&mut self, x: u8, y: u8) {
        if self.v[x as usize] != self.v[y as usize] {
            self.pc += 4;
        } else {
            self.pc += 2;
        }
    }

    /// Sets the address register `i` to `nnn`.
    fn op_annn(&mut self, nnn: u16) {
        self.i = nnn;
        self.pc += 2;
    }

    /// Jumps to address `nnn + v0`.
    /// The program counter is set to the address `nnn + v0`.
    fn op_bnnn(&mut self, nnn: u16) {
        self.pc = nnn + self.v[0] as u16;
    }

    /// Sets `vx` to the bitwise AND of a random number and `kk`.
    fn op_cxnn(&mut self, x: u8, kk: u8) {
        self.v[x as usize] = rand::thread_rng().gen::<u8>() & kk;
        self.pc += 2;
    }

    /// Draws a sprite at coordinates (`v[x], `v[y]`) with a width of 8 pixels and a height of `n`
    /// pixels. The row pixel data are read starting from the memory location at `i`. Since there
    /// are `n` such rows, `n` bytes will be read. Each byte is XOR'd onto the corresponding row
    /// to determine the final displayed pixels of that row. That is, the displayed pixel is
    /// flipped if the corresponding sprite pixel is set, and unchanged if not.
    ///
    /// If the x-coordinate `v[x]` is outside the range of the display, then it is reduced modulo
    /// `64`, the width of the display. Likewise, for the y-coordinate `v[y]`, it will be reduced
    /// modulo `32`, the height of the display. However, sprites that are drawn partially offscreen
    /// are clipped rather than wrapped.
    ///
    /// If any of the displayed pixels are flipped from set to unset, then the carry flag `v[0xF]`
    /// is set to `1`. Otherwise, it is set to `0`. This occurs if and only if both the sprite
    /// pixel and corresponding display pixel are both `1`.
    ///
    /// # Arguments
    /// * `x` - the data register identifier from which the x-coordinate of the sprite will be read
    /// * `y` - the data register identifier from which the y-coordinate of the sprite will be read
    /// * `n` - the height of the sprite
    fn op_dxyn(&mut self, x: u8, y: u8, n: u8) {
        self.v[0xF] = 0;
        let (vx, vy) = (
            self.v[x as usize] % WIDTH as u8,
            self.v[y as usize] % HEIGHT as u8,
        );

        for oy in 0..n {
            let y = vy as usize + oy as usize;
            if y >= HEIGHT {
                break;
            }

            let sprite_pixels = self.memory[(self.i + oy as u16) as usize];
            for (ox, spx) in sprite_pixels.view_bits::<Msb0>().iter().enumerate() {
                let x = vx as usize + ox;
                if x >= WIDTH {
                    break;
                }

                let idx = x + y * WIDTH;
                let mut dpx = self.display.get_mut(idx).unwrap();
                self.v[0xF] |= (*dpx & *spx) as u8;
                *dpx ^= *spx;
            }
        }
        self.pc += 2;
    }

    /// Skips the next instruction if the key stored in `vx` is pressed.
    fn op_ex9e(&mut self, x: u8) {
        let vx = self.v[x as usize];
        if self.keypad[vx as usize] {
            self.pc += 4;
        } else {
            self.pc += 2;
        }
    }

    /// Skips the next instruction if the key stored in `vx` is not pressed.
    fn op_exa1(&mut self, x: u8) {
        let vx = self.v[x as usize];
        if !self.keypad[vx as usize] {
            self.pc += 4;
        } else {
            self.pc += 2;
        }
    }

    /// Sets `vx` to the current value of the delay timer.
    fn op_fx07(&mut self, x: u8) {
        self.v[x as usize] = self.dt;
        self.pc += 2;
    }

    /// Blocks until a key is pressed, then stores the result in `vx`.
    fn op_fx0a(&mut self, x: u8) {
        loop {
            match self.event_rx.recv() {
                Ok(ev) => match ev {
                    Event::Tick => (),
                    Event::Input(input) => {
                        self.register_input(input);
                        match input {
                            Input::Down(key) => {
                                self.v[x as usize] = key as u8;
                                break;
                            }
                            Input::Up(_) => (),
                        }
                    }
                },
                Err(_) => log::error!("Event channel disconnected!"),
            }
        }
        self.pc += 2;
    }

    /// Sets the delay timer to `vx`.
    fn op_fx15(&mut self, x: u8) {
        self.dt = self.v[x as usize];
        self.pc += 2;
    }

    /// Sets the sound timer to `vx`.
    fn op_fx18(&mut self, x: u8) {
        self.st = self.v[x as usize];
        self.pc += 2;
    }

    /// Adds `vx` to the address register `i`.
    fn op_fx1e(&mut self, x: u8) {
        self.i = self.i.wrapping_add(self.v[x as usize] as u16);
        self.pc += 2;
    }

    /// Sets the address register `i` to the memory address of the sprite for the hexadecimal
    /// digit in `vx`.
    fn op_fx29(&mut self, x: u8) {
        // The fontset is loaded at address 0 in ascending order,
        // and each digit is represented by a sprite which takes up 5 bytes.
        self.i = (self.v[x as usize] * 5) as u16;
        self.pc += 2;
    }

    /// Stores the binary-coded decimal equivalent of `vx` at addresses `i`, `i + 1` and `i + 2`.
    fn op_fx33(&mut self, x: u8) {
        self.memory[self.i as usize] = self.v[x as usize] / 100;
        self.memory[(self.i + 1) as usize] = (self.v[x as usize] % 100) / 10;
        self.memory[(self.i + 2) as usize] = self.v[x as usize] % 10;
        self.pc += 2;
    }

    /// Stores the values of `v0` to `vx` (inclusive) in memory, starting from the address in `i`. `i` is set to `i + x + 1`.
    fn op_fx55(&mut self, x: u8) {
        self.memory[self.i as usize..=self.i as usize + x as usize]
            .copy_from_slice(&self.v[..=x as usize]);
        self.i = self.i + x as u16 + 1;
        self.pc += 2;
    }

    /// Fills `v0` to `vx` (inclusive) with the values stored in memory, starting from the address
    /// in `i`. `i` is set to `i + x + 1`.
    fn op_fx65(&mut self, x: u8) {
        self.v[..=x as usize]
            .copy_from_slice(&self.memory[self.i as usize..=self.i as usize + x as usize]);
        self.i = self.i + x as u16 + 1;
        self.pc += 2;
    }
}
