use std::fs;
use std::io::ErrorKind;
use std::time::Instant;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use num_traits::FromPrimitive;
use num_derive::FromPrimitive;

enum Exception {
    IOP,
    SEG,
}

#[derive(FromPrimitive)]
enum Instruction {
    HLT = 0x0,
    CALL = 0x1,
    RET = 0x2,
    JMP = 0x3,
    JNZ = 0x4,
    MOV = 0x5,
    LDI = 0x6,
    LDA = 0x7,
    STA = 0x8,
    PUSH = 0x9,
    POP = 0xA,
    AND = 0xB,
    NOT = 0xC,
    SHR = 0xD,
    SHL = 0xE,
    ADD = 0xF
}

pub struct Configuration {
    pub cycles_per_second: u128,
    pub initial_pc: u16,
    pub memory_size: u16,
    pub firmware_file: String,
    pub verbose: bool
}

impl Configuration {
    pub fn default() -> Self {
        return Self {
            cycles_per_second: 32,
            initial_pc: 0,
            memory_size: 0x4000,
            firmware_file: String::new(),
            verbose: false
        }
    }

    pub fn dump_to_stdout(&self) {
        println!();
        println!(" ----- VM CFG -----");
        println!(" CPS={}", self.cycles_per_second);
        println!(" iPC={}", self.initial_pc);
        println!(" MEM={}", self.memory_size);
        println!(" FWF={}", self.firmware_file);
    }
}

struct Firmware {
    data: Box<[u8]>,
    size: u16
}

impl Firmware {
    pub fn from_file(path: &String) -> Self {
        match fs::read(path) {
            Ok(bytes) => return Self { 
                size: bytes.len() as u16,
                data: bytes.into_boxed_slice() 
            },
            Err(e) => {
                eprint!("Failed to load firmware: ");
                match e.kind() {
                    ErrorKind::PermissionDenied => eprintln!("permission denied."),
                    ErrorKind::NotFound => eprintln!("file not found."),
                    _ => eprintln!("unknown error.")
                }
                panic!("{}", e);
            }
        }
    }

    pub fn default() -> Self {
        let default = vec![
            // Move 0xDEAD into r0
            0x60, 0xDE,
            0xE0, 0x08,
            0x60, 0xAD,
            // Move 0xDEAD into r1
            0x61, 0xDE,
            0xE1, 0x08,
            0x61, 0xAD,
            // Halt
            0x00, 0x00
        ];
        return Self {
            size: default.len() as u16,
            data: default.into_boxed_slice()
        }
    }
}

struct Memory {
    data: Box<[u8]>,
    size: u16
}

impl Memory {
    pub fn new(alloc_size: u16) -> Self {
        if alloc_size == 0 {
            panic!("Cannot create memory with size of 0");
        }
        let mut vec = Vec::new();
        for _ in 0..alloc_size {
            vec.push(0);
        }
        return Self {
            data: vec.into_boxed_slice(),
            size: alloc_size
        }
    }
}

struct Registers {
    r: [u16; 8],
    c: [u16; 2],
    sp: u16,
    fg: u16,
    pc: u16
}

impl Registers {
    pub fn new() -> Self {
        return Self {
            r: [0; 8],
            c: [0; 2],
            sp: 0,
            fg: 0,
            pc: 0
        }
    }

    pub fn num_to_ptr(&mut self, num: u8) -> Option<&mut u16> {
        return match num {
            0x0A => Some(&mut self.sp),
            0x08 | 0x09 => Some(&mut self.c[(num - 0x08) as usize]),
            0x01..=0x07 => Some(&mut self.r[num as usize]),
            _ => None
        }
    }
}

pub struct VirtualMachine {
    config: Configuration,
    firmware: Firmware,
    mem: Memory,
    regs: Registers,
    pub should_run: Arc<AtomicBool>
}

impl VirtualMachine {
    pub fn new(config: Configuration) -> Self {
        let firmware = if config.firmware_file.is_empty() {
            Firmware::default()
        } else {
            Firmware::from_file(&config.firmware_file)
        };
        let mem = Memory::new(config.memory_size);
        let regs = Registers::new();
        return Self {
            config,
            firmware,
            mem,
            regs,
            should_run: Arc::new(AtomicBool::new(true))
        }
    }

    pub fn dump_to_stdout(&self) {
        println!();
        println!(" ---- VM STATE ----");
        println!(" R0={:0>4X}    R1={:0>4X}", self.regs.r[0], self.regs.r[1]);
        println!(" R2={:0>4X}    R3={:0>4X}", self.regs.r[2], self.regs.r[3]);
        println!(" R4={:0>4X}    R5={:0>4X}", self.regs.r[4], self.regs.r[5]);
        println!(" C0={:0>4X}    C1={:0>4X}", self.regs.c[0], self.regs.c[1]);
        println!(" FG={:0>4X}    SP={:0>4X}", self.regs.fg,   self.regs.sp  );
        println!(" PC={:0>4X}              ", self.regs.pc                  );
    }

    pub fn reset(&mut self) {
        self.regs.pc = self.config.initial_pc;
        for i in 0..self.firmware.size {
            self.mem.data[(self.regs.pc + i) as usize] = self.firmware.data[i as usize];
        }
    }

    pub fn run(&mut self) {
        let delta_ceil = 1_000_000_000 / self.config.cycles_per_second;
        let mut before = Instant::now();
        let mut delta = 0;
        while self.should_run.load(Ordering::Relaxed) {
            let now = Instant::now();
            delta += (now - before).as_nanos();
            if delta >= delta_ceil {
                match self.step() {
                    Ok(s) => self.regs.pc += s,
                    Err(_) => {}
                }
                delta -= delta_ceil;
                if delta >= delta_ceil {
                    println!(" [WARN] Running late by {}ns", delta);
                }
            }
            before = now;
        }
    }

    fn fetch(&self) -> u16 {
        if self.regs.pc > self.mem.size - 2 {
            return 0;    
        }
        let opcode_hi = self.mem.data[self.regs.pc as usize] as u16;
        let opcode_lo = self.mem.data[(self.regs.pc + 1) as usize] as u16;
        return (opcode_hi << 8) | opcode_lo;
    }

    fn step(&mut self) -> Result<u16, Exception> {
        let opcode = self.fetch();
        if self.config.verbose {
            println!(" [PC={:0>4X}] Executing opcode ({:0>4X})", self.regs.pc, opcode);
        }
        match FromPrimitive::from_u8(((opcode & 0xF000) >> 12) as u8) {
            Some(i) => {
                match i {
                    Instruction::HLT => { return Ok(0); }
                    Instruction::CALL => {
                        if (opcode & 0x00FF) != 0x00 {
                            return Err(Exception::SEG);
                        }
                        let x = (opcode & 0x0F00) >> 8;
                        if x >= 0x08 {
                            return Err(Exception::IOP);
                        } else if self.regs.sp < 2 {
                            return Err(Exception::SEG);
                        }
                        let tmp = self.regs.pc + 2;
                        self.regs.pc += 2;
                        self.mem.data[self.regs.pc as usize] = ((tmp & 0xFF00) >> 8) as u8;
                        self.mem.data[(self.regs.pc + 1) as usize] = (tmp & 0x00FF) as u8;
                        return Ok(0);
                    }
                    Instruction::RET => {
                        if (opcode & 0x0FFF) != 0x00 {
                            return Err(Exception::SEG);
                        }
                        if self.regs.sp > self.mem.size - 2 {
                            return Err(Exception::SEG);
                        }
                        let pc_hi = (self.mem.data[self.regs.sp as usize] as u16) << 8;
                        let pc_lo = self.mem.data[(self.regs.pc + 1) as usize] as u16;
                        self.regs.pc = pc_hi | pc_lo;
                        self.regs.sp += 2;
                        return Ok(0);
                    }
                    Instruction::JMP => {
                        if (opcode & 0x00FF) != 0x00 {
                            return Err(Exception::SEG);
                        }
                        let x = (opcode & 0x0F00) >> 8;
                        if x >= 0x08 {
                            return Err(Exception::IOP);
                        }
                        self.regs.pc = self.regs.r[x as usize];
                        return Ok(0);
                    }
                    Instruction::JNZ => {
                        if (opcode & 0x000F) != 0x00 {
                            return Err(Exception::SEG);
                        }
                        let x = (opcode & 0x0F00) >> 8;
                        let y = (opcode & 0x00F0) >> 4;
                        if x >= 0x08 || y >= 0x08 {
                            return Err(Exception::IOP);
                        }
                        if self.regs.r[y as usize] != 0 {
                            self.regs.pc = self.regs.r[x as usize];
                            return Ok(0);
                        }
                    }
                    Instruction::MOV => {
                        if (opcode & 0x000F) != 0x00 {
                            return Err(Exception::SEG);
                        }
                        let x = (opcode & 0x0F00) >> 8;
                        let y = (opcode & 0x00F0) >> 4;
                        if x > 0x0A || y > 0x0A {
                            return Err(Exception::IOP);
                        }
                        let ry = *self.regs.num_to_ptr(y as u8).unwrap();
                        let rx = self.regs.num_to_ptr(x as u8).unwrap();
                        *rx = ry;
                    }
                    Instruction::LDI => {
                        let x = (opcode & 0x0F00) >> 8;
                        if x >= 0x08 {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] = (self.regs.r[x as usize] & 0xFF00) | (opcode & 0x00FF);
                    }
                    Instruction::LDA => {
                        let x = (opcode & 0x0F00) >> 8;
                        let y = (opcode & 0x00F0) >> 4;
                        if x >= 0x08 || y >= 0x08 {
                            return Err(Exception::IOP);
                        }
                        let addr = self.regs.r[y as usize];
                        if addr > self.mem.size - 2 {
                            return Err(Exception::SEG);
                        }
                        let value_lo;
                        let value_hi;
                        match opcode & 0x000F {
                            0x0 => {
                                value_hi = self.regs.r[x as usize] & 0xFF00;
                                value_lo = self.mem.data[addr as usize] as u16;
                                
                            }
                            0x1 => {
                                value_hi = self.mem.data[addr as usize] as u16;
                                value_lo = self.mem.data[(addr + 1) as usize] as u16;
                            }
                            _ => return Err(Exception::IOP)
                        }
                        self.regs.r[x as usize] = (value_hi << 8) | value_lo;
                    }
                    Instruction::STA => {
                        let y = (opcode & 0x0F00) >> 8;
                        let x = (opcode & 0x00F0) >> 4;
                        if x >= 0x08 || y >= 0x08 {
                            return Err(Exception::IOP);
                        }
                        let addr = self.regs.r[y as usize];
                        if addr > self.mem.size - 2 {
                            return Err(Exception::SEG);
                        }
                        match opcode & 0x000F {
                            0x0 => self.mem.data[addr as usize] = (self.regs.r[x as usize] & 0x00FF) as u8,
                            0x1 => {
                                self.mem.data[addr as usize] = ((self.regs.r[x as usize] & 0xFF00) >> 8) as u8;
                                self.mem.data[(addr + 1) as usize] = (self.regs.r[x as usize] & 0x00FF) as u8;
                            }
                            _ => return Err(Exception::IOP)
                        }
                    }
                    Instruction::PUSH => {
                        if self.regs.sp < 2 {
                            return Err(Exception::SEG);
                        }
                        let value;
                        match opcode & 0x00FF {
                            0x00 => {
                                let x = (opcode & 0x0F00) >> 8;
                                if x > 0x0A {
                                    return Err(Exception::IOP);
                                }
                                value = *self.regs.num_to_ptr(x as u8).unwrap();
                            }
                            0x01 => {
                                if (opcode & 0x0FF0) != 0x00 {
                                    return Err(Exception::IOP);
                                }
                                value = self.regs.fg;
                            }
                            _ => return Err(Exception::IOP)
                        }
                        self.regs.sp -= 2;
                        self.mem.data[self.regs.sp as usize] = ((value & 0xFF00) >> 8) as u8;
                        self.mem.data[(self.regs.sp + 1) as usize] = (value & 0x00FF) as u8;
                    }
                    Instruction::POP => {
                        if self.regs.sp >= self.mem.size {
                            return Err(Exception::SEG);
                        }
                        let value_hi = self.mem.data[self.regs.sp as usize] as u16;
                        let value_lo = self.mem.data[(self.regs.sp + 1) as usize] as u16;
                        let value = (value_hi << 8) | value_lo;
                        match opcode & 0x00FF {
                            0x00 => {
                                let x = (opcode & 0x0F00) >> 8;
                                if x > 0x0A {
                                    return Err(Exception::IOP);
                                }
                                *self.regs.num_to_ptr(x as u8).unwrap() = value;
                            }
                            0x01 => {
                                if (opcode & 0x0FF0) != 0x00 {
                                    return Err(Exception::IOP);
                                }
                                self.regs.fg = value;
                            }
                            _ => return Err(Exception::IOP)
                        }
                        self.regs.sp += 2;
                    }
                    Instruction::AND => {
                        if (opcode & 0x000F) != 0 {
                            return Err(Exception::IOP);
                        }
                        let x = (opcode & 0x0F00) >> 8;
                        let y = (opcode & 0x00F0) >> 4;
                        if x >= 0x08 || y >= 0x08 {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] &= self.regs.r[y as usize];
                    }
                    Instruction::NOT => {
                        if (opcode & 0x00FF) != 0 {
                            return Err(Exception::IOP);
                        }
                        let x = (opcode & 0x0F00) >> 8;
                        if x >= 0x08 {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] = !self.regs.r[x as usize];
                    }
                    Instruction::SHR => {
                        let x = (opcode & 0x0F00) >> 8;
                        if x >= 0x08 {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] >>= opcode & 0x00FF;
                    }
                    Instruction::SHL => {
                        let x = (opcode & 0x0F00) >> 8;
                        if x >= 0x08 {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] <<= opcode & 0x00FF;
                    }
                    Instruction::ADD => {
                        if (opcode & 0x000F) != 0 {
                            return Err(Exception::IOP);
                        }
                        let x = (opcode & 0x0F00) >> 8;
                        let y = (opcode & 0x00F0) >> 4;
                        if x >= 0x08 || y >= 0x08 {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] += self.regs.r[y as usize];
                    }
                }
            }
            None => panic!("Failed to fetch next instruction")
        }
        return Ok(2);
    }
}
