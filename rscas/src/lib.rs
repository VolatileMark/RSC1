use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::num::ParseIntError;
use std::path::PathBuf;
use std::fs;

#[derive(Eq, Hash, PartialEq)]
pub enum Flag {
    ENTRY
}

pub enum FlagValue {
    String(String),
    Integer(u64),
    Boolean(bool)
}

enum Error {
    IOP,
    UOP,
    EOL
}

struct File {
    bytes: Vec<u8>,
    main: bool
}

pub struct Job {
    files: Vec<PathBuf>,
    output: String,
    flags: HashMap<Flag, FlagValue>
}

impl Job {
    pub fn new() -> Self {
        return Self {
            files: Vec::new(),
            output: "".to_string(),
            flags: HashMap::new()
        }
    }

    pub fn add_file(&mut self, path: &str) {
        let abs_path = fs::canonicalize(path)
            .expect(format!("Failed to convert {} to absolute path", path).as_str());
        self.files.push(abs_path);
    }

    pub fn add_flag(&mut self, flag: Flag, value: FlagValue) {
        self.flags.insert(flag, value);
    }

    pub fn set_output(&mut self, output: &str) {
        if output.is_empty() {
            panic!("Not output file provided");
        }
        if !self.output.is_empty() {
            panic!("Output file was provided more than one time!");
        }
        self.output = output.to_string();
    }

    pub fn run(&self) {
        self.ok();
        let mut files = Vec::new();
        for path in self.files.iter() {
            let contents = read_file(path);
            if !contents.is_ascii() {
                panic!("{} contents are not in ASCII format.", path.display());
            }
            let lines = contents.split("\n");
            let mut bytes = Vec::new();
            for line in lines.into_iter() {
                let opcode = self.gen_opcode(line.trim());
                match opcode {
                    Ok(data) => {
                        bytes.push(data.0);
                        bytes.push(data.1);
                    }
                    Err(reason) => {
                        match reason {
                            Error::IOP => panic!("Invalid instruction '{}'", line),
                            Error::UOP => panic!("Unknown instruction '{}'", line),
                            Error::EOL => {}
                        }
                    }
                }
            }
            files.push(File {
                bytes,
                main: false
            });
        }
        write_file(self.output.as_str(), &mut files);
    }

    fn ok(&self) {
        if self.files.len() == 0 {
            panic!("No source files specified.");
        }
        
        if self.output.is_empty() {
            panic!("No output file specified.");
        }
    }

    fn gen_opcode(&self, line: &str) -> Result<(u8, u8), Error> {
        let inst_and_params = line.split_once(" ")
            .unwrap_or((line, ""));
        let inst = inst_and_params.0.to_ascii_lowercase();
        if inst.is_empty() {
            return Err(Error::EOL);
        }
        let mut params = inst_and_params.1.split(",");
        match inst.as_str() {
            "hlt" => return Ok((0x00, 0x00)),
            "call" => {
                let x = get_num_reg(params.next());
                if x >= 0x08 {
                    return Err(Error::IOP);
                }
                return Ok((0x10 | x, 0x00));
            }
            "ret" => return Ok((0x20, 0x00)),
            "jmp" => {
                let x = get_num_reg(params.next());
                if x >= 0x08 {
                    return Err(Error::IOP);
                }
                return Ok((0x30 | x, 0x00));
            }
            "jnz" => {
                let x = get_num_reg(params.next());
                if x >= 0x08 {
                    return Err(Error::IOP);
                }
                return Ok((0x40 | x, 0x00));
            }
            "mov" => {
                let x = get_num_reg(params.next());
                let y = get_num_reg(params.next());
                if x > 0x0A || y > 0x0A {
                    return Err(Error::IOP);
                }
                return Ok((0x50 | x, 0x00 | (y << 4)));
            }
            "ldi" => {
                let x = get_num_reg(params.next());
                let i = params.next().unwrap_or_default().trim();
                if i.is_empty() || x >= 0x08 {
                    return Err(Error::IOP);
                }
                let n = to_unsigned_byte(i);
                match n {
                    Ok(n) => return Ok((0x60 | x, n)),
                    Err(e) => {
                        eprintln!("{}", e);
                        return Err(Error::IOP);
                    }
                }
            }
            "ldb" => {
                let x = get_num_reg(params.next());
                let y = get_num_reg(params.next());
                if x > 0x0A || y > 0x0A {
                    return Err(Error::IOP);
                }
                return Ok((0x70 | x, 0x00 | (y << 4)));
            }
            "ldw" => {
                let x = get_num_reg(params.next());
                let y = get_num_reg(params.next());
                if x > 0x0A || y > 0x0A {
                    return Err(Error::IOP);
                }
                return Ok((0x70 | x, 0x01 | (y << 4)));
            }
            "stb" => {
                let y = get_num_reg(params.next());
                let x = get_num_reg(params.next());
                if x > 0x0A || y > 0x0A {
                    return Err(Error::IOP);
                }
                return Ok((0x80 | y, 0x00 | (x << 4)));
            }
            "stw" => {
                let y = get_num_reg(params.next());
                let x = get_num_reg(params.next());
                if x > 0x0A || y > 0x0A {
                    return Err(Error::IOP);
                }
                return Ok((0x80 | y, 0x01 | (x << 4)));
            }
            "pushr" => {
                let x = get_num_reg(params.next());
                if x > 0x0A {
                    return Err(Error::IOP);
                }
                return Ok((0x90 | x, 0x00));
            }
            "pushf" => return Ok((0x90, 01)),
            "popr" => {
                let x = get_num_reg(params.next());
                if x > 0x0A {
                    return Err(Error::IOP);
                }
                return Ok((0xA0 | x, 0x00));
            }
            "popf" => return Ok((0xA0, 01)),
            "and" => {
                let x = get_num_reg(params.next());
                let y = get_num_reg(params.next());
                if x > 0x0A || y > 0x0A {
                    return Err(Error::IOP);
                }
                return Ok((0xB0 | x, 0x00 | (y << 4)));
            }
            "not" => {
                let x = get_num_reg(params.next());
                if x > 0x0A {
                    return Err(Error::IOP);
                }
                return Ok((0xC0 | x, 0x00));
            }
            "shr" => {
                let x = get_num_reg(params.next());
                let i = params.next().unwrap_or_default().trim();
                if i.is_empty() || x >= 0x08 {
                    return Err(Error::IOP);
                }
                let n = to_unsigned_byte(i);
                match n {
                    Ok(n) => return Ok((0xD0 | x, n)),
                    Err(e) => {
                        eprintln!("{}", e);
                        return Err(Error::IOP);
                    }
                }
            }
            "shl" => {
                let x = get_num_reg(params.next());
                let i = params.next().unwrap_or_default().trim();
                if i.is_empty() || x >= 0x08 {
                    return Err(Error::IOP);
                }
                let n = to_unsigned_byte(i);
                match n {
                    Ok(n) => return Ok((0xE0 | x, n)),
                    Err(e) => {
                        eprintln!("{}", e);
                        return Err(Error::IOP);
                    }
                }
            }
            "add" => {
                let x = get_num_reg(params.next());
                let y = get_num_reg(params.next());
                if x > 0x0A || y > 0x0A {
                    return Err(Error::IOP);
                }
                return Ok((0xF0 | x, 0x00 | (y << 4)));
            }
            _ => return Err(Error::UOP)
        }
    }
}

fn to_unsigned_byte(i: &str) -> Result<u8, ParseIntError> {
    if i.starts_with("0x") {
        return u8::from_str_radix(i.replace("0x", "").as_str(), 16);
    } else {
        return i.parse::<u8>()
    }
}

fn get_num_reg(string: Option<&str>) -> u8 {
    let x_str = string.unwrap_or_default().trim();
    match x_str {
        "c0" => return 0x08,
        "c1" => return 0x09,
        "sp" => return 0x0A,
        _ => {
            let x_bytes = x_str.as_bytes();
            if x_bytes.len() < 2 {
                return 0xFF;
            }
            if x_bytes[0] != ('r' as u8) || x_bytes[1] < ('0' as u8) || x_bytes[1] > ('7' as u8) {
                return 0xFF;
            }
            return x_bytes[1] - ('0' as u8);
        }
    }
}

fn write_file(output: &str, files: &mut Vec<File>) {
    files.sort_by(|this, to| {
        if this.main && !to.main {
            return Ordering::Greater;
        } else if !this.main && to.main {
            return Ordering::Less;
        } else if !this.main && !to.main {
            return Ordering::Equal;
        } else {
            panic!("Found two entry points when only one was expected");
        }
    });
    let mut contents = Vec::new();
    for file in files {
        contents.append(&mut file.bytes);
    }
    let bytes = contents.len();
    match fs::write(output, contents) {
        Ok(_) => println!("Compilation finished. Wrote {} bytes", bytes),
        Err(_) => panic!("Failed to write output file"),
    }
}

fn read_file(file: &PathBuf) -> String {
    match fs::read_to_string(file) {
        Ok(string) => {
            return string;
        },
        Err(e) => {
            eprint!("Failed to open file {}: ", file.display());
            match e.kind() {
                ErrorKind::PermissionDenied => eprintln!("permission denied."),
                ErrorKind::NotFound => eprintln!("file not found."),
                _ => eprintln!("unknown error.")
            }
            panic!("{}", e);
        }
    }
}
