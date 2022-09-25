use num_derive::ToPrimitive;
use num_traits::ToPrimitive;
use std::fs;
use std::io::ErrorKind;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

enum Exception {
    IOP,
    SEG,
    UNA,
}

enum Instruction {
    NOP = 0x0000,
    AND = 0x1000,
    NOT = 0x1001,
    ADD = 0x2000,
    SUB = 0x2001,
    INC = 0x2002,
    DEC = 0x2003,
    LDB = 0x3000,
    LDW = 0x3001,
    MOV = 0x3002,
    LDI = 0x4000,
    STB = 0x5000,
    STW = 0x5001,
    JMP = 0x6000,
    JNZ = 0x6001,
    SHR = 0x7000,
    SHL = 0x7001,
    TEST = 0x8000,
    SETF = 0x8001,
    CLRF = 0x8002,
}

#[derive(ToPrimitive)]
enum RegisterId {
    R7 = 0x07,
    SP = 0x08,
    C1 = 0x0A,
}

pub struct Configuration {
    pub cycles_per_second: u128,
    pub initial_pc: u16,
    pub memory_size: u16,
    pub firmware_file: String,
    pub verbose: bool,
}

impl Configuration {
    pub fn default() -> Self {
        return Self {
            cycles_per_second: 32,
            initial_pc: 0,
            memory_size: 0x4000,
            firmware_file: String::new(),
            verbose: false,
        };
    }

    pub fn dump_to_stdout(&self) {
        println!();
        println!(" ----- VM CFG -----");
        println!(" CPS={}", self.cycles_per_second);
        println!(" iPC={}", self.initial_pc);
        println!(" MEM={}", self.memory_size);
        println!(" FWF={}", self.firmware_file);
        println!();
    }
}

struct Firmware {
    data: Box<[u8]>,
    size: u16,
}

impl Firmware {
    pub fn from_file(path: &String) -> Self {
        match fs::read(path) {
            Ok(bytes) => {
                return Self {
                    size: bytes.len() as u16,
                    data: bytes.into_boxed_slice(),
                }
            }
            Err(e) => {
                eprint!("Failed to load firmware: ");
                match e.kind() {
                    ErrorKind::PermissionDenied => eprintln!("permission denied."),
                    ErrorKind::NotFound => eprintln!("file not found."),
                    _ => eprintln!("unknown error."),
                }
                panic!("{}", e);
            }
        }
    }

    pub fn default() -> Self {
        let default = vec![
            // Move 0xDEAD into r0
            0xDE, 0x40, 
            0x81, 0x70, 
            0xAD, 0x40, 
            // Move 0xBEEF into r1
            0xBE, 0x41, 
            0x81, 0x71, 
            0xEF, 0x41, 
            // Load no-op address
            0x00, 0x42, 
            0x81, 0x72, 
            0x12, 0x42, 
            // No-op
            0x00, 0x00, 
            // Jump to no-op
            0x00, 0x62,
        ];
        return Self {
            size: default.len() as u16,
            data: default.into_boxed_slice(),
        };
    }
}

struct Memory {
    data: Box<[u8]>,
    size: u16,
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
            size: alloc_size,
        };
    }
}

struct Registers {
    r: [u16; 8],
    c: [u16; 2],
    sp: u16,
    fg: u16,
    pc: u16,
}

impl Registers {
    pub fn new() -> Self {
        return Self {
            r: [0; 8],
            c: [0; 2],
            sp: 0,
            fg: 0,
            pc: 0,
        };
    }
}

pub struct VirtualMachine {
    config: Configuration,
    firmware: Firmware,
    mem: Memory,
    regs: Registers,
    pub should_run: Arc<AtomicBool>,
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
            should_run: Arc::new(AtomicBool::new(true)),
        };
    }

    pub fn dump_to_stdout(&self) {
        println!();
        println!(" ---- VM STATE ----");
        println!(" R0={:0>4X}    R1={:0>4X}", self.regs.r[0], self.regs.r[1]);
        println!(" R2={:0>4X}    R3={:0>4X}", self.regs.r[2], self.regs.r[3]);
        println!(" R4={:0>4X}    R5={:0>4X}", self.regs.r[4], self.regs.r[5]);
        println!(" C0={:0>4X}    C1={:0>4X}", self.regs.c[0], self.regs.c[1]);
        println!(" FG={:0>4X}    SP={:0>4X}", self.regs.fg, self.regs.sp);
        println!(" PC={:0>4X}              ", self.regs.pc);
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
                    Err(e) => match e {
                        Exception::IOP => self.regs.fg |= 1 << 15,
                        Exception::SEG => self.regs.fg |= 1 << 14,
                        Exception::UNA => self.regs.fg |= 1 << 13,
                    }
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
        let opcode_lo = self.mem.data[self.regs.pc as usize] as u16;
        let opcode_hi = self.mem.data[(self.regs.pc + 1) as usize] as u16;
        return (opcode_hi << 8) | opcode_lo;
    }

    fn step(&mut self) -> Result<u16, Exception> {
        let opcode = self.fetch();
        if self.config.verbose {
            println!(
                " [PC={:0>4X}] Executing opcode ({:0>4X})",
                self.regs.pc, opcode
            );
        }
        match decode_opcode(opcode) {
            Some(i) => {
                let x = (opcode & 0x0F00) >> 8;
                let y = (opcode & 0x00F0) >> 4;
                let nn = opcode & 0x00FF;
                match i {
                    Instruction::NOP => {}
                    Instruction::AND => {
                        if !check_register_range(x, RegisterId::R7)
                            || !check_register_range(y, RegisterId::R7)
                        {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] &= self.regs.r[y as usize];
                    }
                    Instruction::NOT => {
                        if !check_register_range(x, RegisterId::R7) {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] = !self.regs.r[x as usize];
                    }
                    Instruction::ADD => {
                        if !check_register_range(x, RegisterId::R7)
                            || !check_register_range(y, RegisterId::R7)
                        {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] += self.regs.r[y as usize];
                    }
                    Instruction::SUB => {
                        if !check_register_range(x, RegisterId::R7)
                            || !check_register_range(y, RegisterId::R7)
                        {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] -= self.regs.r[y as usize];
                    }
                    Instruction::INC => {
                        if !check_register_range(x, RegisterId::SP) {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] += 1;
                    }
                    Instruction::DEC => {
                        if !check_register_range(x, RegisterId::SP) {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] -= 1;
                    }
                    Instruction::LDB => {
                        if !check_register_range(x, RegisterId::R7)
                            || !check_register_range(y, RegisterId::SP)
                        {
                            return Err(Exception::IOP);
                        }
                        let address = self.regs.r[y as usize];
                        if address >= self.mem.size {
                            return Err(Exception::SEG);
                        }
                        let xh = self.regs.r[x as usize] & 0xFF00;
                        self.regs.r[x as usize] = xh | self.mem.data[address as usize] as u16;
                    }
                    Instruction::LDW => {
                        if !check_register_range(x, RegisterId::R7)
                            || !check_register_range(y, RegisterId::SP)
                        {
                            return Err(Exception::IOP);
                        }
                        let address = self.regs.r[y as usize];
                        if address >= self.mem.size - 1 {
                            return Err(Exception::SEG);
                        }
                        self.regs.r[x as usize] = ((self.mem.data[address as usize + 1] as u16)
                            << 8)
                            | (self.mem.data[address as usize] as u16);
                    }
                    Instruction::MOV => {
                        if !check_register_range(x, RegisterId::C1)
                            || !check_register_range(y, RegisterId::C1)
                        {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] = self.regs.r[y as usize];
                    }
                    Instruction::LDI => {
                        if !check_register_range(x, RegisterId::R7) {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] = (self.regs.r[x as usize] & 0xFF00) | nn;
                    }
                    Instruction::STB => {
                        if !check_register_range(x, RegisterId::SP)
                            || !check_register_range(y, RegisterId::R7)
                        {
                            return Err(Exception::IOP);
                        }
                        let address = self.regs.r[x as usize];
                        if address >= self.mem.size {
                            return Err(Exception::SEG);
                        }
                        self.mem.data[address as usize] = (self.regs.r[y as usize] & 0x00FF) as u8;
                    }
                    Instruction::STW => {
                        if !check_register_range(x, RegisterId::SP)
                            || !check_register_range(y, RegisterId::R7)
                        {
                            return Err(Exception::IOP);
                        }
                        let address = self.regs.r[x as usize];
                        if address >= self.mem.size - 1 {
                            return Err(Exception::SEG);
                        }
                        self.mem.data[address as usize + 1] = (self.regs.r[y as usize] >> 8) as u8;
                        self.mem.data[address as usize] = (self.regs.r[y as usize] & 0x00FF) as u8;
                    }
                    Instruction::JMP => {
                        if !check_register_range(x, RegisterId::SP) {
                            return Err(Exception::IOP);
                        }
                        let address = self.regs.r[x as usize];
                        if address % 2 != 0 {
                            return Err(Exception::UNA);
                        }
                        self.regs.pc = address;
                    }
                    Instruction::JNZ => {
                        if !check_register_range(x, RegisterId::SP)
                            || !check_register_range(y, RegisterId::R7)
                        {
                            return Err(Exception::IOP);
                        }
                        let address = self.regs.r[x as usize];
                        if address % 2 != 0 {
                            return Err(Exception::UNA);
                        }
                        if self.regs.r[y as usize] == 0 {
                            self.regs.pc = address;
                        }
                    }
                    Instruction::SHR => {
                        if !check_register_range(x, RegisterId::R7) {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] >>= y;
                    }
                    Instruction::SHL => {
                        if !check_register_range(x, RegisterId::R7) {
                            return Err(Exception::IOP);
                        }
                        self.regs.r[x as usize] <<= y;
                    }
                    Instruction::TEST => {
                        if self.regs.fg & (1 << x) != 0 {
                            self.regs.pc += 2;
                        }
                    }
                    Instruction::SETF => {
                        self.regs.fg |= 1 << x;
                    }
                    Instruction::CLRF => {
                        self.regs.fg &= !(1 << x);
                    }
                }
            }
            None => panic!("Failed to fetch next instruction"),
        }
        return Ok(2);
    }
}

fn check_register_range(reg: u16, ceil: RegisterId) -> bool {
    match ceil.to_u16() {
        Some(n) => return reg <= n,
        None => return false,
    }
}

fn decode_opcode(opcode: u16) -> Option<Instruction> {
    return match opcode & 0xF003 {
        0x0000 => Some(Instruction::NOP),
        0x1000 => Some(Instruction::AND),
        0x1001 => Some(Instruction::NOT),
        0x2000 => Some(Instruction::ADD),
        0x2001 => Some(Instruction::SUB),
        0x2002 => Some(Instruction::INC),
        0x2003 => Some(Instruction::DEC),
        0x3000 => Some(Instruction::LDB),
        0x3001 => Some(Instruction::LDW),
        0x3002 => Some(Instruction::MOV),
        0x4000..=0x4003 => Some(Instruction::LDI),
        0x5000 => Some(Instruction::STB),
        0x5001 => Some(Instruction::STW),
        0x6000 => Some(Instruction::JMP),
        0x6001 => Some(Instruction::JNZ),
        0x7000 => Some(Instruction::SHR),
        0x7001 => Some(Instruction::SHL),
        0x8000 => Some(Instruction::TEST),
        0x8001 => Some(Instruction::SETF),
        0x8002 => Some(Instruction::CLRF),
        _ => None,
    };
}
