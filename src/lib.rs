pub const WIDTH: u32 = 64;
pub const HEIGHT: u32 = 32;

pub struct Chip8 {}

impl Chip8 {
    pub fn new() -> Self {
        Self {}
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
        log::info!("Execute opcode: {:#06X}", opcode);
    }

    fn op_0nnn(&mut self, nnn: u16) {}
    fn op_00e0(&mut self) {}
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
    fn op_dxyn(&mut self, x: u8, y: u8, n: u8) {}
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
