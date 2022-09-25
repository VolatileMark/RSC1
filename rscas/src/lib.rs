use std::{fs, path::PathBuf, str::FromStr};

const TRAMPOLINE_SIZE: u64 = 4 * 2;

#[macro_export]
macro_rules! critical {
    ($($arg:tt)*) => {
        {
            eprintln!($($arg)*);
            std::process::exit(-1);
        }
    };
}

pub enum Token {
    // Literals
    Label(u64, u16),
    // Assembler directives
    Short(u64, bool),
    Addr(u16),
    // Instructions
    Nop,
    And(u16, u16),
    Not(u16),
    Add(u16, u16),
    Sub(u16, u16),
    Inc(u16),
    Dec(u16),
    Ldb(u16, u16),
    Ldw(u16, u16),
    Mov(u16, u16),
    Ldi(u16, u8),
    Stb(u16, u16),
    Stw(u16, u16),
    Jmp(u16),
    Jnz(u16, u16),
    Shr(u16, u8),
    Shl(u16, u8),
    Test(u8),
    Setf(u8),
    Clrf(u8),
    // Assembler pseudo instructions
    Push(u16),
    Pop(u16),
    Ldl(u16, u64),
}

pub struct Executable {
    bytes: Vec<u8>,
    address: u16,
}

impl Executable {
    pub fn new() -> Self {
        return Self {
            bytes: Vec::new(),
            address: 0
        };
    }

    fn push_byte(&mut self, b: u8) {
        let mut current_address = self.bytes.len() as u16;
        if self.address > current_address {
            for _ in current_address..self.address {
                self.bytes.push(0);
            }
            current_address = self.address;
        }
        if self.address == current_address {
            self.bytes.push(b);
        } else {
            self.bytes.remove(self.address as usize);
            self.bytes.insert(self.address as usize, b);
        }
        self.address += 1;
    }

    pub fn push_short(&mut self, s: u16) {
        self.push_byte((s & 0x00FF) as u8);
        self.push_byte(((s & 0xFF00) >> 8) as u8);
    }

    pub fn size(&self) -> usize {
        return self.bytes.len();
    }

    pub fn set_address(&mut self, a: u16) {
        self.address = a;
    }

    pub fn bytes(&self) -> &Vec<u8> {
        return &self.bytes;
    }
}

pub struct Job {
    files: Vec<String>,
    entry: String,
    output: String,
    trampoline: bool,
    address: u64,
}

impl Job {
    pub fn new() -> Self {
        return Self {
            files: Vec::new(),
            entry: "_start".to_string(),
            output: "a.out".to_string(),
            trampoline: false,
            address: 0,
        };
    }

    pub fn add_file(&mut self, path: String) {
        self.files.push(path);
    }

    pub fn set_entry(&mut self, entry: String) {
        self.entry = entry;
    }

    pub fn set_output(&mut self, path: String) {
        self.output = path;
    }

    pub fn trampoline(&mut self) {
        self.address += TRAMPOLINE_SIZE;
        self.trampoline = true;
    }

    fn get_lines(&self) -> Vec<String> {
        if self.files.is_empty() {
            critical!("No input file provided.");
        }
        let mut code = Vec::new();
        for path in self.files.iter() {
            let content = read_file(path);
            if !content.is_ascii() {
                critical!("File `{}` is not ASCII.", path);
            }
            let lines = content.split("\n");
            for line in lines.into_iter() {
                if !line.trim().is_empty() {
                    code.push(line.trim_start().trim_end().to_string());
                }
            }
        }
        return code;
    }

    pub fn tokenize(&mut self) -> Vec<Token> {
        let lines = self.get_lines();
        let mut tokens = Vec::new();
        for i in 0..lines.len() {
            let line = match lines.get(i) {
                Some(line) => line,
                None => critical!("Failed to fetch line number {}.", i),
            };
            if self.address >= u16::MAX as u64 {
                critical!("Exceeded maximum binary size! Fault line: `{}`.", line);
            }
            tokens.push(self.gen_token(line.trim_end()));
        }
        if self.trampoline {
            let id = calculate_label_id(self.entry.as_str());
            tokens.insert(0, Token::Ldl(0, id));
            tokens.insert(1, Token::Jmp(0));
        }
        return tokens;
    }

    pub fn write_output(&self, exec: Executable) {
        if let Ok(path) = PathBuf::from_str(self.output.as_str()) {
            match fs::write(path, exec.bytes()) {
                Ok(_) => println!("Wrote {} bytes.", exec.size()),
                Err(err) => critical!(
                    "An error occured when writing file `{}`:\n`{}`.",
                    self.output,
                    err.to_string()
                ),
            }
            return;
        }
        critical!("Failed to write output file `{}`.", self.output);
    }

    fn gen_token(&mut self, raw_line: &str) -> Token {
        let line = match raw_line.split_once(' ') {
            Some(parts) => (parts.0.to_string(), parts.1.replace(" ", "")),
            None => (raw_line.to_string(), "".to_string()),
        };
        if line.0.starts_with('.') && !line.1.is_empty() {
            return self.gen_directive_token(&line.0, &line.1);
        }
        if line.0.ends_with(':') && line.1.is_empty() {
            return self.gen_label_token(&line.0);
        }
        return self.gen_instruction_token(&line.0, &line.1);
    }

    fn gen_instruction_token(&mut self, instruction: &String, arguments: &String) -> Token {
        let arguments = arguments.split(',').collect::<Vec<&str>>();
        let assert_args_len_eq = |len| -> () {
            let arglen = arguments.len();
            if (len == 0 && (arglen > 1 || !arguments[0].is_empty())) || (len > 0 && arglen > len) {
                critical!(
                    "Too many arguments for assembler instruction `{}`.",
                    instruction
                );
            } else if arglen < len {
                critical!(
                    "Not enough arguments for assembler instruction `{}`.",
                    instruction
                );
            }
        };
        self.address += 2;
        return match instruction.as_str() {
            "nop" => {
                assert_args_len_eq(0);
                Token::Nop
            }
            "and" => {
                assert_args_len_eq(2);
                Token::And(reg_name_to_num(arguments[0]), reg_name_to_num(arguments[1]))
            }
            "not" => {
                assert_args_len_eq(1);
                Token::Not(reg_name_to_num(arguments[0]))
            }
            "add" => {
                assert_args_len_eq(2);
                Token::Add(reg_name_to_num(arguments[0]), reg_name_to_num(arguments[1]))
            }
            "sub" => {
                assert_args_len_eq(2);
                Token::Sub(reg_name_to_num(arguments[0]), reg_name_to_num(arguments[1]))
            }
            "inc" => {
                assert_args_len_eq(1);
                Token::Inc(reg_name_to_num(arguments[0]))
            }
            "dec" => {
                assert_args_len_eq(1);
                Token::Dec(reg_name_to_num(arguments[0]))
            }
            "ldb" => {
                assert_args_len_eq(2);
                Token::Ldb(reg_name_to_num(arguments[0]), reg_name_to_num(arguments[1]))
            }
            "ldw" => {
                assert_args_len_eq(2);
                Token::Ldw(reg_name_to_num(arguments[0]), reg_name_to_num(arguments[1]))
            }
            "mov" => {
                assert_args_len_eq(2);
                Token::Mov(reg_name_to_num(arguments[0]), reg_name_to_num(arguments[1]))
            }
            "ldi" => {
                assert_args_len_eq(2);
                Token::Ldi(
                    reg_name_to_num(arguments[0]),
                    parse_int_from_string(arguments[1]),
                )
            }
            "stb" => {
                assert_args_len_eq(2);
                Token::Stb(reg_name_to_num(arguments[0]), reg_name_to_num(arguments[1]))
            }
            "stw" => {
                assert_args_len_eq(2);
                Token::Stw(reg_name_to_num(arguments[0]), reg_name_to_num(arguments[1]))
            }
            "jmp" => {
                assert_args_len_eq(1);
                Token::Jmp(reg_name_to_num(arguments[0]))
            }
            "jnz" => {
                assert_args_len_eq(2);
                Token::Jnz(reg_name_to_num(arguments[0]), reg_name_to_num(arguments[1]))
            }
            "shr" => {
                assert_args_len_eq(2);
                Token::Shr(
                    reg_name_to_num(arguments[0]),
                    parse_int_from_string(arguments[1]),
                )
            }
            "shl" => {
                assert_args_len_eq(2);
                Token::Shl(
                    reg_name_to_num(arguments[0]),
                    parse_int_from_string(arguments[1]),
                )
            }
            "test" => {
                assert_args_len_eq(1);
                Token::Test(parse_int_from_string(arguments[0]))
            }
            "setf" => {
                assert_args_len_eq(1);
                Token::Setf(parse_int_from_string(arguments[0]))
            }
            "clrf" => {
                assert_args_len_eq(1);
                Token::Clrf(parse_int_from_string(arguments[0]))
            }
            _ => self.gen_pseudo_instruction_token(instruction, &arguments),
        };
    }

    fn gen_pseudo_instruction_token(&mut self, instruction: &String, arguments: &Vec<&str>) -> Token {
        let assert_args_len_eq = |len| -> () {
            let arglen = arguments.len();
            if arglen > len {
                critical!(
                    "Too many arguments for assembler instruction `{}`.",
                    instruction
                );
            } else if arglen < len {
                critical!(
                    "Not enough arguments for assembler instruction `{}`.",
                    instruction
                );
            }
        };
        return match instruction.as_str() {
            "push" => {
                assert_args_len_eq(1);
                self.address += 2;
                Token::Push(reg_name_to_num(arguments[0]))
            }
            "pop" => {
                assert_args_len_eq(1);
                self.address += 2;
                Token::Pop(reg_name_to_num(arguments[0]))
            }
            "ldl" => {
                assert_args_len_eq(2);
                self.address += 4;
                let value = match arguments[0].parse::<u16>() {
                    Ok(v) => v as u64,
                    Err(_) => {
                        let trimmed = arguments[1].trim_start_matches("0x");
                        match u16::from_str_radix(trimmed, 16) {
                            Ok(v) => v as u64,
                            Err(_) => calculate_label_id(trimmed),
                        }
                    },
                };
                Token::Ldl(reg_name_to_num(arguments[0]), value)
            }
            _ => critical!("Invalid instruction `{}`.", instruction),
        };
    }

    fn gen_directive_token(&mut self, directive: &String, arguments: &String) -> Token {
        let directive = match directive.strip_prefix('.') {
            Some(str) => str,
            None => critical!("Failed to remove semicolon from label (`{}`).", directive),
        };
        let arguments = arguments.split(',').collect::<Vec<&str>>();
        let assert_args_len_eq = |len| -> () {
            let arglen = arguments.len();
            if arglen > len {
                critical!(
                    "Too many arguments for assembler directive `.{}`.",
                    directive
                );
            } else if arglen < len {
                critical!(
                    "Not enough arguments for assembler instruction `.{}`.",
                    directive
                );
            }
        };
        return match directive {
            "short" => {
                assert_args_len_eq(1);
                let mut is_label = false;
                let trimmed = arguments[0].trim_start_matches("0x");
                let short = match trimmed.parse::<u16>() {
                    Ok(v) => v as u64,
                    Err(_) => match u16::from_str_radix(trimmed, 16) {
                        Ok(v) => v as u64,
                        Err(_) => {
                            is_label = true;
                            calculate_label_id(trimmed)
                        },
                    },
                };
                self.address += 2;
                Token::Short(short, is_label)
            }
            "addr" => {
                assert_args_len_eq(1);
                let address = match arguments[0].parse::<u16>() {
                    Ok(v) => v as u64,
                    Err(_) => {
                        let trimmed = arguments[0].trim_start_matches("0x");
                        match u16::from_str_radix(trimmed, 16) {
                            Ok(v) => v as u64,
                            Err(_) => critical!("Invalid address `{}`", trimmed),
                        }
                    }
                };
                if address % 2 != 0 {
                    critical!("Address {:0>4X} is not 2 byte aligned.", address);
                } else if address > u16::MAX as u64 {
                    critical!("Address {:0>4X} is higher than the maximum allowed.", address);
                }
                self.address = address;
                Token::Addr(address as u16)
            }
            _ => critical!("Invalid directive `.{}`.", directive),
        };
    }

    fn gen_label_token(&mut self, line: &String) -> Token {
        let label = match line.strip_suffix(':') {
            Some(str) => str,
            None => critical!("Failed to remove semicolon from label (`{}`).", line),
        };
        if self.trampoline && self.entry.as_str() == label && self.address == TRAMPOLINE_SIZE {
            self.address -= TRAMPOLINE_SIZE;
            self.trampoline = false;
        }
        return Token::Label(calculate_label_id(label), self.address as u16);
    }
}

fn calculate_label_id(label: &str) -> u64 {
    let label = label.as_bytes();
    let mut hash = 0;
    for j in (0..label.len()).step_by(8) {
        let mut mask = 0;
        for i in 0..8 {
            let index = i + j;
            let value = if index < label.len() { label[index] } else { 0 };
            mask |= (value as u64) << i * 8;
        }
        hash ^= mask;
    }
    return hash;
}

fn parse_int_from_string<F: std::str::FromStr>(string: &str) -> F {
    return match string.parse::<F>() {
        Ok(val) => val,
        Err(_) => critical!("Error parsing `{}` into unsigned integer.", string),
    };
}

fn reg_name_to_num(name: &str) -> u16 {
    let name = name.trim();
    if name == "sp" {
        return 0x0A;
    } else if name.starts_with("r") || name.starts_with('c') {
        return match name.get(1..2) {
            Some(num) => parse_int_from_string(num),
            None => critical!(
                "Failed to obtain register number (input string was `{}`).",
                name
            ),
        };
    }
    critical!("Invalid register `{}`.", name)
}

fn read_file(file: &String) -> String {
    if let Ok(path) = PathBuf::from_str(file.as_str()) {
        match fs::read_to_string(path) {
            Ok(str) => return str,
            Err(err) => critical!(
                "An error occured when reading file `{}`:\n`{}`.",
                file,
                err.to_string()
            ),
        }
    }
    critical!("Failed to create file path from string `{}`.", file);
}
