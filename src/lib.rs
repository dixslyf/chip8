pub const WIDTH: u8 = 64;
pub const HEIGHT: u8 = 32;
const MEMORY_SIZE: usize = 4096;

pub struct Chip8 {
    i: u16,                                            // 12-bit address register
    v: [u8; 16],                                       // 16 8-bit data registers
    memory: [u8; MEMORY_SIZE],                         // 4KB memory
    display: [bool; WIDTH as usize * HEIGHT as usize], // 64 * 32 monochrome display
}

impl Chip8 {
    pub fn new() -> Self {
        Self {
            i: 0,
            v: [0; 16],
            memory: [0; MEMORY_SIZE],
            display: [false; WIDTH as usize * HEIGHT as usize],
        }
    }

    pub fn execute_opcode(&mut self, opcode: u16) {
        // Break into nibbles
        let nibbles = (
            (opcode & 0xF000) >> 12 as u8,
            (opcode & 0x0F00) >> 8 as u8,
            (opcode & 0x00F0) >> 4 as u8,
            (opcode & 0x000F) as u8,
        );

        let nnn = opcode & 0x0FFF;
        let kk = (opcode & 0x00FF) as u8;
        let n = (opcode & 0x000F) as u8;
        let x = ((opcode & 0x0F00) >> 8) as u8;
        let y = ((opcode & 0x00F0) >> 4) as u8;

        log::info!("Execute opcode: {:#06X}", opcode);
        match nibbles {
            (0x0, 0x0, 0xE, 0x0) => self.op_00e0(),
            (0x0, 0x0, 0xE, 0xE) => self.op_00ee(),
            (0x0, _, _, _) => self.op_0nnn(nnn),
            (0x1, _, _, _) => self.op_1nnn(nnn),
            (0x2, _, _, _) => self.op_2nnn(nnn),
            (0x3, _, _, _) => self.op_3xnn(x, kk),
            (0x4, _, _, _) => self.op_4xnn(x, kk),
            (0x5, _, _, 0x0) => self.op_5xy0(x, y),
            (0x6, _, _, _) => self.op_6xnn(x, kk),
            (0x7, _, _, _) => self.op_7xnn(x, kk),
            (0x8, _, _, 0x0) => self.op_8xy0(x, y),
            (0x8, _, _, 0x1) => self.op_8xy1(x, y),
            (0x8, _, _, 0x2) => self.op_8xy2(x, y),
            (0x8, _, _, 0x3) => self.op_8xy3(x, y),
            (0x8, _, _, 0x4) => self.op_8xy4(x, y),
            (0x8, _, _, 0x5) => self.op_8xy5(x, y),
            (0x8, _, _, 0x6) => self.op_8xy6(x, y),
            (0x8, _, _, 0x7) => self.op_8xy7(x, y),
            (0x8, _, _, 0xe) => self.op_8xye(x, y),
            (0x9, _, _, 0x0) => self.op_9xy0(x, y),
            (0xA, _, _, _) => self.op_annn(nnn),
            (0xB, _, _, _) => self.op_bnnn(nnn),
            (0xC, _, _, _) => self.op_cxnn(x, kk),
            (0xD, _, _, _) => self.op_dxyn(x, y, n),
            (0xE, _, 0x9, 0xE) => self.op_ex9e(x),
            (0xE, _, 0xA, 0x1) => self.op_exa1(x),
            (0xF, _, 0x0, 0x7) => self.op_fx07(x),
            (0xF, _, 0x0, 0xA) => self.op_fx0a(x),
            (0xF, _, 0x1, 0x5) => self.op_fx15(x),
            (0xF, _, 0x1, 0x8) => self.op_fx18(x),
            (0xF, _, 0x1, 0xE) => self.op_fx1e(x),
            (0xF, _, 0x2, 0x9) => self.op_fx29(x),
            (0xF, _, 0x3, 0x3) => self.op_fx33(x),
            (0xF, _, 0x5, 0x5) => self.op_fx55(x),
            (0xF, _, 0x6, 0x5) => self.op_fx65(x),
            _ => panic!("Unknown opcode {:#06X}", opcode),
        }
    }

    fn op_0nnn(&mut self, nnn: u16) {}

    /// Clears the display.
    fn op_00e0(&mut self) {
        self.display.iter_mut().for_each(|p| *p = false);
    }

    fn op_00ee(&mut self) {}
    fn op_1nnn(&mut self, nnn: u16) {}
    fn op_2nnn(&mut self, nnn: u16) {}
    fn op_3xnn(&mut self, x: u8, kk: u8) {}
    fn op_4xnn(&mut self, x: u8, kk: u8) {}
    fn op_5xy0(&mut self, x: u8, y: u8) {}
    fn op_6xnn(&mut self, x: u8, kk: u8) {}
    fn op_7xnn(&mut self, x: u8, kk: u8) {}
    fn op_8xy0(&mut self, x: u8, y: u8) {}
    fn op_8xy1(&mut self, x: u8, y: u8) {}
    fn op_8xy2(&mut self, x: u8, y: u8) {}
    fn op_8xy3(&mut self, x: u8, y: u8) {}
    fn op_8xy4(&mut self, x: u8, y: u8) {}
    fn op_8xy5(&mut self, x: u8, y: u8) {}
    fn op_8xy6(&mut self, x: u8, y: u8) {}
    fn op_8xy7(&mut self, x: u8, y: u8) {}
    fn op_8xye(&mut self, x: u8, y: u8) {}
    fn op_9xy0(&mut self, x: u8, y: u8) {}
    fn op_annn(&mut self, nnn: u16) {}
    fn op_bnnn(&mut self, nnn: u16) {}
    fn op_cxnn(&mut self, x: u8, kk: u8) {}

    /// Draws a sprite at coordinates (`v[x], `v[y]`) with a width of 8 pixels and a height of `n` pixels. The row pixel data are read starting from the memory location at `i`. Since there are `n` such rows, `n` bytes will be read. Each byte is XOR'd onto the corresponding row to determine the final displayed pixels of that row. That is, the displayed pixel is flipped if the corresponding sprite pixel is set, and unchanged if not.
    ///
    /// If any of the displayed pixels are flipped from set to unset, then the carry flag `v[0xF]` is set to `1`. Otherwise, it is set to `0`.
    ///
    /// # Arguments
    /// * `x` - the data register identifier from which the x-coordinate of the sprite will be read
    /// * `y` - the data register identifier from which the y-coordinate of the sprite will be read
    /// * `n` - the height of the sprite
    fn op_dxyn(&mut self, x: u8, y: u8, n: u8) {
        log::trace!("Inputs: x = {}, y = {}, n = {}", x, y, n);
        self.v[0xF] = 0;
        let (vx, vy) = (self.v[x as usize], self.v[y as usize]);
        for oy in 0..n {
            let y = (vy + oy) % HEIGHT;
            // Contains the pixel data for each x-value (bit-coded)
            let pixels = self.memory[(self.i + oy as u16) as usize];
            for ox in 0..8 {
                let x = (vx + ox) % WIDTH;
                let p = pixels >> (7 - ox) & 0x1; // Extract the corresponding bit
                let idx = x as usize + y as usize * WIDTH as usize;
                // VF is set if any of the pixels are flipped from set to unset. Keeping in mind
                // that the sprite pixels are XOR'd onto the corresponding screen pixels, this only
                // happens when both the sprite pixel and screen pixel are set.
                self.v[0xF] |= p & self.display[idx] as u8;
                self.display[idx] ^= p == 1;
                log::trace!("Set pixel at ({}, {}) to {}", x, y, self.display[idx]);
            }
        }
    }

    fn op_ex9e(&mut self, x: u8) {}
    fn op_exa1(&mut self, x: u8) {}
    fn op_fx07(&mut self, x: u8) {}
    fn op_fx0a(&mut self, x: u8) {}
    fn op_fx15(&mut self, x: u8) {}
    fn op_fx18(&mut self, x: u8) {}
    fn op_fx1e(&mut self, x: u8) {}
    fn op_fx29(&mut self, x: u8) {}
    fn op_fx33(&mut self, x: u8) {}
    fn op_fx55(&mut self, x: u8) {}
    fn op_fx65(&mut self, x: u8) {}
}
