use arrayvec::ArrayVec;
use bitvec::{array::BitArray, order::Msb0, slice::BitSlice, view::BitView, BitArr};
use rand::Rng;

pub const DISPLAY_WIDTH: usize = 64;
pub const DISPLAY_HEIGHT: usize = 32;

const REGISTER_COUNT: usize = 16;
const STACK_SIZE: usize = 16;
const FONTSET_SIZE: usize = 80;
const MEMORY_SIZE: usize = 4096;
const START_ROM_ADDRESS: usize = 0x200;
const MAX_ROM_SIZE: usize = MEMORY_SIZE - START_ROM_ADDRESS;
const FONTSET: [u8; FONTSET_SIZE] = [
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
pub enum Input {
    Up(Key),
    Down(Key),
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Key {
    Key0,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
}

#[derive(Debug, Copy, Clone)]
pub struct Quirks {
    pub draw_wrap: bool,
    pub vf_reset: bool,
}

impl Default for Quirks {
    fn default() -> Self {
        Self {
            draw_wrap: true,
            vf_reset: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Chip8 {
    pc: u16,                                                     // 12-bit program counter
    i: u16,                                                      // 12-bit address register
    v: [u8; REGISTER_COUNT],                                     // 16 8-bit data registers
    stack: ArrayVec<u16, STACK_SIZE>,                            // 16-level stack
    memory: [u8; MEMORY_SIZE],                                   // 4KB memory
    display: BitArr!(for DISPLAY_WIDTH * DISPLAY_HEIGHT, in u8), // 64 * 32 monochrome display
    should_redraw: bool,
    dt: u8,
    st: u8,
    keypad: BitArr!(for 16, in u8), // 16-key keypad
    waiting_for_keypress: bool,
    keypress_register: u8,
    quirks: Quirks,
}

impl Default for Chip8 {
    fn default() -> Self {
        Self::new(Quirks::default())
    }
}

impl Chip8 {
    pub fn new(quirks: Quirks) -> Self {
        let mut memory = [0; MEMORY_SIZE];
        memory[..FONTSET.len()].copy_from_slice(&FONTSET);
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
            waiting_for_keypress: false,
            keypress_register: 0,
            quirks,
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
            self.memory[START_ROM_ADDRESS..(START_ROM_ADDRESS + rom_size)].copy_from_slice(rom);
        };
        log::info!("Loaded ROM of size {} bytes", MAX_ROM_SIZE.min(rom_size));
    }

    pub fn register_input(&mut self, input: Input) {
        match input {
            Input::Up(key) => {
                *self.keypad.get_mut(key as usize).unwrap() = false;
                if self.waiting_for_keypress {
                    self.waiting_for_keypress = false;
                    log::debug!("No longer waiting for keypress");
                }
            }
            Input::Down(key) => {
                *self.keypad.get_mut(key as usize).unwrap() = true;
                if self.waiting_for_keypress {
                    self.v[self.keypress_register as usize] = key as u8;
                }
            }
        };
    }

    pub fn update_timers(&mut self) {
        if self.dt > 0 {
            self.dt -= 1;
            log::trace!("Delay timer decremented to {}", self.dt);
        }

        if self.st > 0 {
            self.st -= 1;
            log::trace!("Sound timer decremented to {}", self.st);
        }
    }

    pub fn should_beep(&self) -> bool {
        self.st >= 2 // The COSMAC VIP manual states that min value must be 2
    }

    pub fn display(&self) -> &BitSlice<u8> {
        self.display.as_bitslice()
    }

    pub fn execute_cycle(&mut self) {
        if self.waiting_for_keypress {
            self.should_redraw = false;
        } else {
            let opcode = self.fetch_opcode();
            self.execute_opcode(opcode);

            // Redraw only if the opcode is one of the display opcodes
            self.should_redraw = opcode == 0x00E0 || opcode & 0xF000 == 0xD000;
        }
    }

    pub fn should_redraw(&self) -> bool {
        self.should_redraw
    }

    pub fn waiting_for_keypress(&self) -> bool {
        self.waiting_for_keypress
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

    fn increment_pc(&mut self) {
        self.increment_pc_by(1);
    }

    fn increment_pc_by(&mut self, count: u16) {
        self.pc += count * 2;
    }

    /// Calls the machine code routine at address `nnn`.
    /// This opcode is unimplemented. Programs that use this opcode are written specifically for
    /// the hardware that the CHIP-8 interpreter is running on.
    fn op_0nnn(&mut self, _nnn: u16) {
        log::warn!("Opcode 0NNN is unimplemented.");
        self.increment_pc();
    }

    /// Clears the display.
    fn op_00e0(&mut self) {
        self.display.fill(false);
        self.increment_pc();
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
            self.increment_pc_by(2);
        } else {
            self.increment_pc();
        }
    }

    /// Skips the next instruction if `vx` does not equal `kk`.
    fn op_4xnn(&mut self, x: u8, kk: u8) {
        if self.v[x as usize] != kk {
            self.increment_pc_by(2);
        } else {
            self.increment_pc();
        }
    }

    /// Skips the next instruction if `vx` equals `vy`.
    fn op_5xy0(&mut self, x: u8, y: u8) {
        if self.v[x as usize] == self.v[y as usize] {
            self.increment_pc_by(2);
        } else {
            self.increment_pc();
        }
    }

    /// Sets `vx` to `kk`.
    fn op_6xnn(&mut self, x: u8, kk: u8) {
        self.v[x as usize] = kk;
        self.increment_pc();
    }

    /// Adds `kk` to `vx`.
    fn op_7xnn(&mut self, x: u8, kk: u8) {
        self.v[x as usize] = self.v[x as usize].wrapping_add(kk);
        self.increment_pc();
    }

    /// Sets `vx` to `vy`.
    fn op_8xy0(&mut self, x: u8, y: u8) {
        self.v[x as usize] = self.v[y as usize];
        self.increment_pc();
    }

    /// Sets `vx` to the bitwise OR of `vx` and `vy`.
    fn op_8xy1(&mut self, x: u8, y: u8) {
        self.v[x as usize] |= self.v[y as usize];

        // Quirk: Original CHIP-8 resets the flag register.
        if self.quirks.vf_reset {
            self.v[0xF] = 0;
        }

        self.increment_pc();
    }

    /// Sets `vx` to the bitwise AND of `vx` and `vy`.
    fn op_8xy2(&mut self, x: u8, y: u8) {
        self.v[x as usize] &= self.v[y as usize];

        // Quirk: Original CHIP-8 resets the flag register.
        if self.quirks.vf_reset {
            self.v[0xF] = 0;
        }

        self.increment_pc();
    }

    /// Sets `vx` to the XOR of `vx` and `vy`.
    fn op_8xy3(&mut self, x: u8, y: u8) {
        self.v[x as usize] ^= self.v[y as usize];

        // Quirk: Original CHIP-8 resets the flag register.
        if self.quirks.vf_reset {
            self.v[0xF] = 0;
        }

        self.increment_pc();
    }

    /// Adds `vy` to `vx`. `vf` is set to `1` if a carry occurs, and `0` if not.
    fn op_8xy4(&mut self, x: u8, y: u8) {
        let (sum, carry) = self.v[x as usize].overflowing_add(self.v[y as usize]);
        self.v[x as usize] = sum;
        self.v[0xF] = carry as u8;
        self.increment_pc();
    }

    /// Subtracts `vy` from `vx`. `vf` is set to `0` if a borrow occurs, and `1` if not.
    fn op_8xy5(&mut self, x: u8, y: u8) {
        let (diff, borrow) = self.v[x as usize].overflowing_sub(self.v[y as usize]);
        self.v[x as usize] = diff;
        self.v[0xF] = !borrow as u8;
        self.increment_pc();
    }

    /// Sets `vx` to `vy` shifted right by one bit. `vf` is set to the least significant bit of `vy`
    /// prior to the shift.
    fn op_8xy6(&mut self, x: u8, y: u8) {
        // It is possible that 0xF is passsed as x or y, so
        // we store the old value of y in a separate variable
        // and mutate vF last.
        let old_vy = self.v[y as usize];
        self.v[x as usize] = old_vy >> 1;
        self.v[0xF] = old_vy & 0x1;
        self.increment_pc();
    }

    /// Sets `vx` to `vy - vx`. `vf` is set to `0` if a borrow occurs, and `1` if not.
    fn op_8xy7(&mut self, x: u8, y: u8) {
        let (diff, borrow) = self.v[y as usize].overflowing_sub(self.v[x as usize]);
        self.v[x as usize] = diff;
        self.v[0xF] = !borrow as u8;
        self.increment_pc();
    }

    /// Sets `vx` to `vy` shifted left by one bit. `vf` is set to the most significant bit of `vy`
    /// prior to the shift.
    fn op_8xye(&mut self, x: u8, y: u8) {
        // It is possible that 0xF is passsed as x or y, so
        // we store the old value of y in a separate variable
        // and mutate vF last.
        let old_vy = self.v[y as usize];
        self.v[x as usize] = old_vy << 1;
        self.v[0xF] = old_vy >> 7;
        self.increment_pc();
    }

    /// Skips the next instruction if `vx` does not equal `vy`.
    fn op_9xy0(&mut self, x: u8, y: u8) {
        if self.v[x as usize] != self.v[y as usize] {
            self.increment_pc_by(2);
        } else {
            self.increment_pc();
        }
    }

    /// Sets the address register `i` to `nnn`.
    fn op_annn(&mut self, nnn: u16) {
        self.i = nnn;
        self.increment_pc();
    }

    /// Jumps to address `nnn + v0`.
    /// The program counter is set to the address `nnn + v0`.
    fn op_bnnn(&mut self, nnn: u16) {
        self.pc = nnn + self.v[0] as u16;
    }

    /// Sets `vx` to the bitwise AND of a random number and `kk`.
    fn op_cxnn(&mut self, x: u8, kk: u8) {
        self.v[x as usize] = rand::rng().random::<u8>() & kk;
        self.increment_pc();
    }

    /// Draws a sprite at coordinates (`v[x], `v[y]`) with a width of 8 pixels and a height of `n`
    /// pixels. The row pixel data are read starting from the memory location at `i`. Since there
    /// are `n` such rows, `n` bytes will be read. Each byte is XOR'd onto the corresponding row
    /// to determine the final displayed pixels of that row. That is, the displayed pixel is
    /// flipped if the corresponding sprite pixel is set, and unchanged if not.
    ///
    /// If the x-coordinate `v[x]` is outside the range of the display, then it is reduced modulo
    /// `64`, the width of the display. Likewise, for the y-coordinate `v[y]`, it will be reduced
    /// modulo `32`, the height of the display. Whether sprites that are drawn partially offscreen
    /// should be clipped or wrapped depends on whether the draw-wrap quirk is enabled.
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

        // Starting coordinates wrap.
        let (scoord_x, scoord_y) = (
            self.v[x as usize] % DISPLAY_WIDTH as u8,
            self.v[y as usize] % DISPLAY_HEIGHT as u8,
        );

        // Iterate the n rows.
        // oy = offset for y-coordinate.
        for oy in 0..n {
            let unchecked_coord_y = scoord_y as usize + oy as usize;

            // Check bounds along the y-axis if draw_wrap is off.
            if !self.quirks.draw_wrap && unchecked_coord_y >= DISPLAY_HEIGHT {
                break;
            }

            // We already check for bounds, so this would be a no-op if draw_wrap is off.
            let coord_y = unchecked_coord_y % DISPLAY_HEIGHT;

            // Each bit in the byte represents one pixel of the sprite row.
            let sprite_pixels = self.memory[self.i as usize + oy as usize];

            // Iterate the sprite pixels (horizontally).
            // ox = offset for x-coordinate.
            for (ox, spx) in sprite_pixels.view_bits::<Msb0>().iter().enumerate() {
                let unchecked_coord_x = scoord_x as usize + ox;

                // Check bounds along the x-axis if draw_wrap is off.
                if !self.quirks.draw_wrap && unchecked_coord_x >= DISPLAY_WIDTH {
                    break;
                }

                // We already check for bounds, so this would be a no-op if draw_wrap is off.
                let coord_x = unchecked_coord_x % DISPLAY_WIDTH;

                // Index into the display.
                let idx = coord_x + coord_y * DISPLAY_WIDTH;
                let mut dpx = self.display.get_mut(idx).unwrap();

                self.v[0xF] |= (*dpx && *spx) as u8;

                *dpx ^= *spx;
            }
        }
        self.increment_pc();
    }

    /// Skips the next instruction if the key stored in `vx` is pressed.
    fn op_ex9e(&mut self, x: u8) {
        let vx = self.v[x as usize];
        if self.keypad[vx as usize] {
            self.increment_pc_by(2);
        } else {
            self.increment_pc();
        }
    }

    /// Skips the next instruction if the key stored in `vx` is not pressed.
    fn op_exa1(&mut self, x: u8) {
        let vx = self.v[x as usize];
        if !self.keypad[vx as usize] {
            self.increment_pc_by(2);
        } else {
            self.increment_pc();
        }
    }

    /// Sets `vx` to the current value of the delay timer.
    fn op_fx07(&mut self, x: u8) {
        self.v[x as usize] = self.dt;
        self.increment_pc();
    }

    /// Blocks until a key is pressed, then stores the result in `vx`.
    fn op_fx0a(&mut self, x: u8) {
        self.waiting_for_keypress = true;
        self.keypress_register = x;
        self.increment_pc();
        log::debug!("Waiting for keypress");
    }

    /// Sets the delay timer to `vx`.
    fn op_fx15(&mut self, x: u8) {
        self.dt = self.v[x as usize];
        log::trace!("Delay timer set to {}", self.dt);
        self.increment_pc();
    }

    /// Sets the sound timer to `vx`.
    fn op_fx18(&mut self, x: u8) {
        self.st = self.v[x as usize];
        log::trace!("Sound timer set to {}", self.st);
        self.increment_pc();
    }

    /// Adds `vx` to the address register `i`.
    fn op_fx1e(&mut self, x: u8) {
        self.i = self.i.wrapping_add(self.v[x as usize] as u16);
        self.increment_pc();
    }

    /// Sets the address register `i` to the memory address of the sprite for the hexadecimal
    /// digit in `vx`.
    fn op_fx29(&mut self, x: u8) {
        // The fontset is loaded at address 0 in ascending order,
        // and each digit is represented by a sprite which takes up 5 bytes.
        self.i = (self.v[x as usize] * 5) as u16;
        self.increment_pc();
    }

    /// Stores the binary-coded decimal equivalent of `vx` at addresses `i`, `i + 1` and `i + 2`.
    fn op_fx33(&mut self, x: u8) {
        self.memory[self.i as usize] = self.v[x as usize] / 100;
        self.memory[(self.i + 1) as usize] = (self.v[x as usize] % 100) / 10;
        self.memory[(self.i + 2) as usize] = self.v[x as usize] % 10;
        self.increment_pc();
    }

    /// Stores the values of `v0` to `vx` (inclusive) in memory, starting from the address in `i`. `i` is set to `i + x + 1`.
    fn op_fx55(&mut self, x: u8) {
        self.memory[self.i as usize..=self.i as usize + x as usize]
            .copy_from_slice(&self.v[..=x as usize]);
        self.i = self.i + x as u16 + 1;
        self.increment_pc();
    }

    /// Fills `v0` to `vx` (inclusive) with the values stored in memory, starting from the address
    /// in `i`. `i` is set to `i + x + 1`.
    fn op_fx65(&mut self, x: u8) {
        self.v[..=x as usize]
            .copy_from_slice(&self.memory[self.i as usize..=self.i as usize + x as usize]);
        self.i = self.i + x as u16 + 1;
        self.increment_pc();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    prop_compose! {
        /// Generates a Chip8 with random state.
        fn arb_chip8()
                    (
                        pc_offset_half in 0u16..=((0xFFE - 4 - START_ROM_ADDRESS as u16) / 2), // -4 to give some leeway for increments.
                                                                                               // Divide the max by 2 so that
                                                                                               // we can multiply by 2 to only
                                                                                               // generate even offsets.
                        i in 0u16..=0xFFF,
                        v in proptest::collection::vec(any::<u8>(), REGISTER_COUNT),
                        stack in proptest::collection::vec(any::<u16>(), 0..(STACK_SIZE - 1)), // -1 to give some leeway for pushes.
                        memory in proptest::collection::vec(any::<u8>(), MEMORY_SIZE - FONTSET_SIZE),
                        display_bools in proptest::collection::vec(any::<bool>(), DISPLAY_WIDTH * DISPLAY_HEIGHT),
                        should_redraw in any::<bool>(),
                        dt in any::<u8>(),
                        st in any::<u8>(),
                        keypad_bools in proptest::collection::vec(any::<bool>(), 16),
                        // waiting_for_keypress in any::<bool>(),
                        keypress_register in any::<u8>(),
                    )
                    -> Chip8
        {
            let mut chip8 = Chip8::default();
            chip8.pc += pc_offset_half * 2;
            chip8.i = i;
            chip8.v.copy_from_slice(&v);
            chip8.stack.extend(stack.into_iter());
            chip8.memory[FONTSET_SIZE..].copy_from_slice(&memory); // Be careful not to replace the
                                                                   // fontset.
            for (i, bit) in display_bools.into_iter().enumerate() {
                chip8.display.set(i, bit);
            }
            chip8.should_redraw = should_redraw;
            chip8.dt = dt;
            chip8.st = st;
            for (i, bit) in keypad_bools.into_iter().enumerate() {
                chip8.keypad.set(i, bit);
            }
            // chip8.waiting_for_keypress = waiting_for_keypress;
            chip8.keypress_register = keypress_register;

            chip8
        }
    }

    proptest! {
        #[test]
        fn test_0nnn_increments_pc(
            mut chip8 in arb_chip8(),
            nnn in 0u16..=0xFFF,
        ) {
            let old_pc = chip8.pc;
            chip8.op_0nnn(nnn);
            assert_eq!(chip8.pc, old_pc + 2);
        }

        #[test]
        fn test_00e0_increments_pc(mut chip8 in arb_chip8()) {
            let old_pc = chip8.pc;
            chip8.op_00e0();
            assert_eq!(chip8.pc, old_pc + 2);
        }

        #[test]
        fn test_00e0_clears_display(mut chip8 in arb_chip8()) {
            chip8.op_00e0();
            assert!(chip8.display.not_any());
        }

        #[test]
        fn test_00e0_display_idempotence(mut chip8 in arb_chip8()) {
            chip8.op_00e0();
            assert!(chip8.display.not_any());
            chip8.op_00e0();
            assert!(chip8.display.not_any());
        }

        #[test]
        fn test_1nnn_jumping_to_pc_should_leave_pc_unchanged(
            mut chip8 in arb_chip8(),
        ) {
            let old_pc = chip8.pc;
            chip8.op_1nnn(chip8.pc);
            prop_assert_eq!(chip8.pc, old_pc);
        }

        #[test]
        fn test_1nnn_two_jumps_behaves_like_one_jump_to_second_address(
            mut chip8 in arb_chip8(),
            nnn1 in 0u16..=0xFFF,
            nnn2 in 0u16..=0xFFF,
        ) {
            chip8.op_1nnn(nnn1);
            chip8.op_1nnn(nnn2);
            prop_assert_eq!(chip8.pc, nnn2);
        }

        #[test]
        fn test_1nnn_idempotence(
            mut chip8 in arb_chip8(),
            nnn in 0u16..=0xFFF,
        ) {
            chip8.op_1nnn(nnn);
            prop_assert_eq!(chip8.pc, nnn);
            chip8.op_1nnn(nnn);
            prop_assert_eq!(chip8.pc, nnn);
        }

        #[test]
        fn test_2nnn_pushes_incremented_pc_to_stack(
            mut chip8 in arb_chip8(),
            nnn in 0u16..=0xFFF,
        ) {
            let old_pc = chip8.pc;
            let old_stack_len = chip8.stack.len();

            chip8.op_2nnn(nnn);

            prop_assert_eq!(chip8.stack.len(), old_stack_len + 1);
            prop_assert_eq!(*chip8.stack.last().unwrap(), old_pc + 2);
        }

        // With this test, we don't have to re-implement the same tests we have for 1nnn.
        #[test]
        fn test_2nnn_sets_pc_like_1nnn(
            mut chip8 in arb_chip8(),
            nnn in 0u16..=0xFFF,
        ) {
            let mut chip8_1nnn = chip8.clone();
            chip8_1nnn.op_1nnn(nnn);

            chip8.op_2nnn(nnn);

            prop_assert_eq!(chip8.pc, chip8_1nnn.pc);
        }

        #[test]
        fn test_3xnn_skips_next_instruction_when_vx_eq_kk(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            kk in any::<u8>(),
        ) {
            chip8.v[x as usize] = kk;
            let old_pc = chip8.pc;

            chip8.op_3xnn(x, kk);

            // Next instruction skipped, so pc should have moved by 4.
            prop_assert_eq!(chip8.pc - old_pc, 4);
        }

        #[test]
        fn test_3xnn_does_not_skip_next_instruction_when_vx_neq_kk(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            kk in any::<u8>(),
        ) {
            // If the generated value for vx happens to be kk,
            // add 1 so that it's different.
            if chip8.v[x as usize] == kk {
                chip8.v[x as usize] = kk.wrapping_add(1);
            }
            let old_pc = chip8.pc;

            chip8.op_3xnn(x, kk);

            prop_assert_eq!(chip8.pc - old_pc, 2);
        }

        #[test]
        fn test_4xnn_does_not_skip_next_instruction_when_vx_eq_kk(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            kk in any::<u8>(),
        ) {
            chip8.v[x as usize] = kk;
            let old_pc = chip8.pc;

            chip8.op_4xnn(x, kk);

            prop_assert_eq!(chip8.pc - old_pc, 2);
        }

        #[test]
        fn test_4xnn_skips_next_instruction_when_vx_neq_kk(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            kk in any::<u8>(),
        ) {
            // If the generated value for vx happens to be kk,
            // add 1 so that it's different.
            if chip8.v[x as usize] == kk {
                chip8.v[x as usize] = kk.wrapping_add(1);
            }
            let old_pc = chip8.pc;

            chip8.op_4xnn(x, kk);

            // Next instruction skipped, so pc should have moved by 4.
            prop_assert_eq!(chip8.pc - old_pc, 4);
        }

        #[test]
        fn test_only_one_of_3xnn_and_4xnn_should_skip(
            mut chip8_3xnn in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            kk in any::<u8>(),
        ) {
            let mut chip8_4xnn = chip8_3xnn.clone();

            let pc = chip8_3xnn.pc;

            chip8_3xnn.op_3xnn(x, kk);
            chip8_4xnn.op_4xnn(x, kk);

            let delta_3xnn = chip8_3xnn.pc - pc;
            let delta_4xnn = chip8_4xnn.pc - pc;

            prop_assert!([delta_3xnn, delta_4xnn].contains(&2));
            prop_assert!([delta_3xnn, delta_4xnn].contains(&4));
            prop_assert_ne!(delta_3xnn, delta_4xnn);
        }

        #[test]
        fn test_5xy0_skips_next_instruction_when_vx_eq_vy(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8), // Note that we explicitly allow x == y
            y in 0u8..(REGISTER_COUNT as u8),
        ) {
            chip8.v[x as usize] = chip8.v[y as usize];
            let old_pc = chip8.pc;

            chip8.op_5xy0(x, y);

            // Next instruction skipped, so pc should have moved by 4.
            prop_assert_eq!(chip8.pc - old_pc, 4);
        }

        #[test]
        fn test_5xy0_does_not_skip_next_instruction_when_vx_neq_vy(
            mut chip8 in arb_chip8(),
            (x, y) in (0u8..(REGISTER_COUNT as u8), 0u8..(REGISTER_COUNT as u8)).prop_filter(
                "x and y must be different",
                |(x, y)| x != y,
            ),
        ) {
            // If the generated value for vx happens to be the same as vy,
            // add 1 so that it's different.
            if chip8.v[x as usize] == chip8.v[y as usize] {
                chip8.v[x as usize] = chip8.v[y as usize].wrapping_add(1);
            }
            let old_pc = chip8.pc;

            chip8.op_5xy0(x, y);

            prop_assert_eq!(chip8.pc - old_pc, 2);
        }

        #[test]
        fn test_6xnn_increments_pc(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            kk in any::<u8>(),
        ) {
            let old_pc = chip8.pc;
            chip8.op_6xnn(x, kk);
            prop_assert_eq!(chip8.pc, old_pc + 2);
        }

        #[test]
        fn test_6xnn_two_calls_sets_vx_to_kk_arg_of_second_call(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            kk1 in any::<u8>(),
            kk2 in any::<u8>(),
        ) {
            chip8.op_6xnn(x, kk1);
            chip8.op_6xnn(x, kk2);
            prop_assert_eq!(chip8.v[x as usize], kk2);
        }

        #[test]
        fn test_6xnn_vx_idempotence(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            kk in any::<u8>(),
        ) {
            chip8.op_6xnn(x, kk);
            prop_assert_eq!(chip8.v[x as usize], kk);
            chip8.op_6xnn(x, kk);
            prop_assert_eq!(chip8.v[x as usize], kk);
        }

        #[test]
        fn test_7xnn_increments_pc(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            kk in any::<u8>(),
        ) {
            let old_pc = chip8.pc;
            chip8.op_7xnn(x, kk);
            prop_assert_eq!(chip8.pc, old_pc + 2);
        }

        #[test]
        fn test_7xnn_adding_0_leaves_vx_unchanged(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
        ) {
            let old_vx = chip8.v[x as usize];
            chip8.op_7xnn(x, 0);
            prop_assert_eq!(chip8.v[x as usize], old_vx);
        }

        // Wrapping behaviour.
        #[test]
        fn test_7xnn_adding_256_leaves_vx_unchanged(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
        ) {
            let old_vx = chip8.v[x as usize];
            chip8.op_7xnn(x, u8::MAX);
            chip8.op_7xnn(x, 1);
            prop_assert_eq!(chip8.v[x as usize], old_vx);
        }

        #[test]
        fn test_7xnn_adding_two_numbers_should_be_equivalent_to_adding_their_sum(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            kk1 in any::<u8>(),
            kk2 in any::<u8>(),
        ) {
            let mut chip8_sum = chip8.clone();
            let kk_sum = kk1.wrapping_add(kk2);

            chip8.op_7xnn(x, kk1);
            chip8.op_7xnn(x, kk2);

            chip8_sum.op_7xnn(x, kk_sum);

            prop_assert_eq!(chip8_sum.v[x as usize], chip8.v[x as usize]);
        }

        #[test]
        fn test_8xy0_increments_pc(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            y in 0u8..(REGISTER_COUNT as u8),
        ) {
            let old_pc = chip8.pc;
            chip8.op_8xy0(x, y);
            prop_assert_eq!(chip8.pc, old_pc + 2);
        }

        #[test]
        fn test_8xy0_two_calls_sets_vx_to_vy_arg_of_second_call(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            y1 in 0u8..(REGISTER_COUNT as u8),
            y2 in 0u8..(REGISTER_COUNT as u8),
        ) {
            chip8.op_8xy0(x, y1);
            chip8.op_8xy0(x, y2);
            prop_assert_eq!(chip8.v[x as usize], chip8.v[y2 as usize]);
        }

        #[test]
        fn test_8xy0_x_as_second_arg_leaves_vx_unchanged(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
        ) {
            let old_vx = chip8.v[x as usize];
            chip8.op_8xy0(x, x);
            prop_assert_eq!(chip8.v[x as usize], old_vx);
        }

        #[test]
        fn test_8xy0_vx_idempotence(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            y in 0u8..(REGISTER_COUNT as u8),
        ) {
            chip8.op_8xy0(x, y);
            prop_assert_eq!(chip8.v[x as usize], chip8.v[y as usize]);
            chip8.op_8xy0(x, y);
            prop_assert_eq!(chip8.v[x as usize], chip8.v[y as usize]);
        }

        #[test]
        fn test_8xy1_increments_pc(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            y in 0u8..(REGISTER_COUNT as u8),
        ) {
            let old_pc = chip8.pc;
            chip8.op_8xy1(x, y);
            prop_assert_eq!(chip8.pc, old_pc + 2);
        }

        #[test]
        fn test_8xy1_commutative(
            mut chip8_x in arb_chip8(),
            // Because 8xy1 sets the flags register (0xF) to 0,
            // we need to exclude 0xF from this test.
            x in 0u8..((REGISTER_COUNT - 1) as u8),
            y in 0u8..((REGISTER_COUNT - 1) as u8),
        ) {
            let mut chip8_y = chip8_x.clone();

            chip8_x.op_8xy1(x, y);
            chip8_y.op_8xy1(y, x);

            prop_assert_eq!(chip8_x.v[x as usize], chip8_y.v[y as usize]);
        }

        #[test]
        fn test_8xy1_monotonicity(
            mut chip8 in arb_chip8(),
            // Because 8xy1 sets the flags register (0xF) to 0,
            // we need to exclude 0xF from this test.
            x in 0u8..((REGISTER_COUNT - 1) as u8),
            y in 0u8..((REGISTER_COUNT - 1) as u8),
        ) {
            let old_vx = chip8.v[x as usize];
            chip8.op_8xy1(x, y);
            prop_assert_eq!(chip8.v[x as usize] & old_vx, old_vx);
            prop_assert_eq!(chip8.v[x as usize] & chip8.v[y as usize], chip8.v[y as usize]);
        }

        #[test]
        fn test_8xy1_resets_flags_register(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            y in 0u8..(REGISTER_COUNT as u8),
        ) {
            chip8.op_8xy1(x, y);
            prop_assert_eq!(chip8.v[0xF], 0);
        }

        #[test]
        fn test_8xy1_x_as_second_arg_leaves_vx_unchanged(
            mut chip8 in arb_chip8(),
            x in 0u8..((REGISTER_COUNT - 1) as u8),
        ) {
            let old_vx = chip8.v[x as usize];
            chip8.op_8xy1(x, x);
            prop_assert_eq!(chip8.v[x as usize], old_vx);
        }

        #[test]
        fn test_8xy1_idempotence(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            y in 0u8..(REGISTER_COUNT as u8),
        ) {
            chip8.op_8xy1(x, y);
            let vx1 = chip8.v[x as usize];
            chip8.op_8xy1(x, y);
            let vx2 = chip8.v[x as usize];
            prop_assert_eq!(vx1, vx2);
        }

        #[test]
        fn test_8xy2_increments_pc(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            y in 0u8..(REGISTER_COUNT as u8),
        ) {
            let old_pc = chip8.pc;
            chip8.op_8xy2(x, y);
            prop_assert_eq!(chip8.pc, old_pc + 2);
        }

        #[test]
        fn test_8xy2_commutative(
            mut chip8_x in arb_chip8(),
            // Because 8xy2 sets the flags register (0xF) to 0,
            // we need to exclude 0xF from this test.
            x in 0u8..((REGISTER_COUNT - 1) as u8),
            y in 0u8..((REGISTER_COUNT - 1) as u8),
        ) {
            let mut chip8_y = chip8_x.clone();

            chip8_x.op_8xy2(x, y);
            chip8_y.op_8xy2(y, x);

            prop_assert_eq!(chip8_x.v[x as usize], chip8_y.v[y as usize]);
        }

        #[test]
        fn test_8xy2_monotonicity(
            mut chip8 in arb_chip8(),
            // Because 8xy2 sets the flags register (0xF) to 0,
            // we need to exclude 0xF from this test.
            x in 0u8..((REGISTER_COUNT - 1) as u8),
            y in 0u8..((REGISTER_COUNT - 1) as u8),
        ) {
            let old_vx = chip8.v[x as usize];
            chip8.op_8xy2(x, y);
            prop_assert_eq!(chip8.v[x as usize] | old_vx, old_vx);
            prop_assert_eq!(chip8.v[x as usize] | chip8.v[y as usize], chip8.v[y as usize]);
        }

        #[test]
        fn test_8xy2_resets_flags_register(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            y in 0u8..(REGISTER_COUNT as u8),
        ) {
            chip8.op_8xy2(x, y);
            prop_assert_eq!(chip8.v[0xF], 0);
        }

        #[test]
        fn test_8xy2_x_as_second_arg_leaves_vx_unchanged(
            mut chip8 in arb_chip8(),
            x in 0u8..((REGISTER_COUNT - 1) as u8),
        ) {
            let old_vx = chip8.v[x as usize];
            chip8.op_8xy2(x, x);
            prop_assert_eq!(chip8.v[x as usize], old_vx);
        }

        #[test]
        fn test_8xy2_idempotence(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            y in 0u8..((REGISTER_COUNT - 1) as u8),
        ) {
            chip8.op_8xy2(x, y);
            let vx1 = chip8.v[x as usize];
            chip8.op_8xy2(x, y);
            let vx2 = chip8.v[x as usize];
            prop_assert_eq!(vx1, vx2);
        }

        #[test]
        fn test_8xy3_increments_pc(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            y in 0u8..(REGISTER_COUNT as u8),
        ) {
            let old_pc = chip8.pc;
            chip8.op_8xy3(x, y);
            prop_assert_eq!(chip8.pc, old_pc + 2);
        }

        #[test]
        fn test_8xy3_commutative(
            mut chip8_x in arb_chip8(),
            // Because 8xy3 sets the flags register (0xF) to 0,
            // we need to exclude 0xF from this test.
            x in 0u8..((REGISTER_COUNT - 1) as u8),
            y in 0u8..((REGISTER_COUNT - 1) as u8),
        ) {
            let mut chip8_y = chip8_x.clone();

            chip8_x.op_8xy3(x, y);
            chip8_y.op_8xy3(y, x);

            prop_assert_eq!(chip8_x.v[x as usize], chip8_y.v[y as usize]);
        }

        // XOR is its own inverse.
        #[test]
        fn test_8xy3_involution(
            mut chip8 in arb_chip8(),
            // Because 8xy3 sets the flags register (0xF) to 0,
            // we need to exclude 0xF from this test.
            x in 0u8..((REGISTER_COUNT - 1) as u8),
            y in 0u8..((REGISTER_COUNT - 1) as u8),
        ) {
            prop_assume!(x != y);

            let old_vx = chip8.v[x as usize];

            chip8.op_8xy3(x, y);
            chip8.op_8xy3(x, y);

            prop_assert_eq!(chip8.v[x as usize], old_vx);
        }

        #[test]
        fn test_8xy3_with_0_leaves_vx_unchanged(
            mut chip8 in arb_chip8(),
            // Because 8xy3 sets the flags register (0xF) to 0,
            // we need to exclude 0xF from this test.
            x in 0u8..((REGISTER_COUNT - 1) as u8),
            y in 0u8..(REGISTER_COUNT as u8),
        ) {
            chip8.v[y as usize] = 0;
            let old_vx = chip8.v[x as usize];

            chip8.op_8xy3(x, y);

            prop_assert_eq!(chip8.v[x as usize], old_vx);
        }

        #[test]
        fn test_8xy3_resets_flags_register(
            mut chip8 in arb_chip8(),
            x in 0u8..(REGISTER_COUNT as u8),
            y in 0u8..(REGISTER_COUNT as u8),
        ) {
            chip8.op_8xy3(x, y);
            prop_assert_eq!(chip8.v[0xF], 0);
        }

        #[test]
        fn test_8xy3_x_as_second_arg_sets_vx_to_0(
            mut chip8 in arb_chip8(),
            x in 0u8..((REGISTER_COUNT - 1) as u8),
        ) {
            chip8.op_8xy3(x, x);
            prop_assert_eq!(chip8.v[x as usize], 0);
        }
    }
}
