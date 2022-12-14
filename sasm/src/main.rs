use num_traits::ToPrimitive;
use sasm::{critical, Executable, Job, RegisterId, Token};
use std::{collections::HashMap, env, time::Instant};

fn parse_args() -> Job {
    let mut job = Job::new();
    let mut args = env::args().into_iter();

    for _ in 0..args.len() {
        let arg = args.next().unwrap_or_default();
        match arg.as_str() {
            "-T" | "--trampoline" => job.trampoline(),
            "-o" => {
                let file = args.next().unwrap_or_default().trim().to_string();
                job.set_output(file);
            }
            "-e" | "--entry" => {
                let entry = args.next().unwrap_or_default().trim().to_string();
                job.set_entry(entry);
            }
            _ => {
                if arg.ends_with(".S") || arg.ends_with(".asm") {
                    job.add_file(arg);
                }
            }
        }
    }

    return job;
}

fn collect_labels(tokens: &Vec<Token>) -> HashMap<u64, u16> {
    let mut labels = HashMap::new();
    for token in tokens.iter() {
        match token {
            Token::LABEL(id, address) => {
                if labels.contains_key(id) {
                    critical!("Duplicate label `{:0>16X}`", *id);
                }
                labels.insert(*id, *address);
            }
            _ => {}
        };
    }
    return labels;
}

fn check_register_range(reg: u16, ceil: RegisterId) -> bool {
    match ceil.to_u16() {
        Some(n) => return reg <= n,
        None => return false,
    }
}

fn gen_executable(tokens: &Vec<Token>) -> Executable {
    let mut exec = Executable::new();
    let labels = collect_labels(tokens);
    let mut tokens_iter = tokens.iter();
    for line in 0..tokens.len() {
        let check_x = |x: u16, r: RegisterId| {
            if !check_register_range(x, r) {
                critical!("Error @ line {}: X register out of range.", line);
            }
        };
        let check_y = |y: u16, r: RegisterId| {
            if !check_register_range(y, r) {
                critical!("Error @ line {}: Y register out of range.", line);
            }
        };
        if let Some(token) = tokens_iter.next() {
            match *token {
                Token::SHORT(s, l) => {
                    if l {
                        match labels.get(&s) {
                            Some(v) => exec.push_short(*v),
                            None => critical!("Label with id {:0>16X} not found", s),
                        }
                    } else {
                        exec.push_short(s as u16);
                    }
                }
                Token::ADDR(a) => exec.set_address(a),
                Token::NOP => exec.push_short(0x0000),
                Token::AND(x, y) => {
                    check_x(x, RegisterId::R7);
                    check_y(y, RegisterId::R7);
                    exec.push_short(0x1000 | (x << 8) | (y << 4));
                }
                Token::NOT(x) => {
                    check_x(x, RegisterId::R7);
                    exec.push_short(0x1001 | (x << 8));
                }
                Token::ADD(x, y) => {
                    check_x(x, RegisterId::R7);
                    check_y(y, RegisterId::R7);
                    exec.push_short(0x2000 | (x << 8) | (y << 4));
                }
                Token::SUB(x, y) => {
                    check_x(x, RegisterId::R7);
                    check_y(y, RegisterId::R7);
                    exec.push_short(0x2001 | (x << 8) | (y << 4));
                }
                Token::INC(x) => {
                    check_x(x, RegisterId::SP);
                    exec.push_short(0x2002 | (x << 8));
                }
                Token::DEC(x) => {
                    check_x(x, RegisterId::SP);
                    exec.push_short(0x2003 | (x << 8));
                }
                Token::LDB(x, y) => {
                    check_x(x, RegisterId::R7);
                    check_y(y, RegisterId::SP);
                    exec.push_short(0x3000 | (x << 8) | (y << 4));
                }
                Token::LDW(x, y) => {
                    check_x(x, RegisterId::R7);
                    check_y(y, RegisterId::SP);
                    exec.push_short(0x3001 | (x << 8) | (y << 4));
                }
                Token::MOV(x, y) => {
                    check_x(x, RegisterId::C1);
                    check_y(y, RegisterId::C1);
                    exec.push_short(0x3002 | (x << 8) | (y << 4));
                }
                Token::LDI(x, nn) => {
                    check_x(x, RegisterId::R7);
                    exec.push_short(0x4000 | (x << 8) | (nn as u16));
                }
                Token::STB(y, x) => {
                    check_y(y, RegisterId::SP);
                    check_x(x, RegisterId::R7);
                    exec.push_short(0x5000 | (y << 8) | (x << 4));
                }
                Token::STW(y, x) => {
                    check_y(y, RegisterId::SP);
                    check_x(x, RegisterId::R7);
                    exec.push_short(0x5001 | (y << 8) | (x << 4));
                }
                Token::JMP(x) => {
                    check_x(x, RegisterId::SP);
                    exec.push_short(0x6000 | (x << 8));
                }
                Token::JNZ(x, y) => {
                    check_x(x, RegisterId::SP);
                    check_y(y, RegisterId::R7);
                    exec.push_short(0x6001 | (x << 8) | (y << 4));
                }
                Token::SHR(x, n) => {
                    check_x(x, RegisterId::R7);
                    exec.push_short(0x7000 | (x << 8) | (((n & 0x0F) as u16) << 4));
                }
                Token::SHL(x, n) => {
                    check_x(x, RegisterId::R7);
                    exec.push_short(0x7001 | (x << 8) | (((n & 0x0F) as u16) << 4));
                }
                Token::TEST(n) => exec.push_short(0x8000 | (((n & 0x0F) as u16) << 8)),
                Token::SETF(n) => exec.push_short(0x8001 | (((n & 0x0F) as u16) << 8)),
                Token::CLRF(n) => exec.push_short(0x8002 | (((n & 0x0F) as u16) << 8)),
                Token::PUSH(x) => {
                    check_x(x, RegisterId::R7);
                    exec.push_short(0x2803);
                    exec.push_short(0x2803);
                    exec.push_short(0x5801 | (x << 4));
                }
                Token::POP(x) => {
                    check_x(x, RegisterId::R7);
                    exec.push_short(0x3081 | (x << 8));
                    exec.push_short(0x2802);
                    exec.push_short(0x2802);
                }
                Token::LDL(x, k) => {
                    let a = match labels.get(&k) {
                        Some(a) => *a,
                        None => k as u16,
                    };
                    exec.push_short(0x4000 | (x << 8) | ((a & 0xFF00) >> 8));
                    exec.push_short(0x7081 | (x << 8));
                    exec.push_short(0x4000 | (x << 8) | (a & 0x00FF));
                }
                Token::CALL(x, a) => {
                    check_x(x, RegisterId::R7);
                    exec.push_short(0x2803);
                    exec.push_short(0x2803);
                    exec.push_short(0x2803);
                    exec.push_short(0x2803);
                    exec.push_short(0x5801 | (x << 4));
                    exec.push_short(0x2802);
                    exec.push_short(0x2802);
                    exec.push_short(0x4000 | (x << 8) | ((a & 0xFF00) >> 8));
                    exec.push_short(0x7081 | (x << 8));
                    exec.push_short(0x4000 | (x << 8) | (a & 0x00FF));
                    exec.push_short(0x5801 | (x << 4));
                    exec.push_short(0x2803);
                    exec.push_short(0x2803);
                    exec.push_short(0x3081 | (x << 8));
                    exec.push_short(0x2802);
                    exec.push_short(0x2802);
                    exec.push_short(0x6000 | (x << 8));
                }
                Token::CALLF(x, y, a) => {
                    check_x(x, RegisterId::SP);
                    check_y(y, RegisterId::R7);
                    if x == y {
                        critical!("CALLF: register X cannot be the same as register Y");
                    }
                    // LDL
                    exec.push_short(0x4000 | (y << 8) | ((a & 0xFF00) >> 8));
                    exec.push_short(0x7081 | (y << 8));
                    exec.push_short(0x4000 | (y << 8) | (a & 0x00FF));
                    // PUSH Y
                    exec.push_short(0x2803);
                    exec.push_short(0x2803);
                    exec.push_short(0x5801 | (y << 4));
                    // JMP X
                    exec.push_short(0x6000 | (x << 8));
                }
                Token::RET(x) => {
                    check_x(x, RegisterId::R7);
                    exec.push_short(0x3081 | (x << 8));
                    exec.push_short(0x2802);
                    exec.push_short(0x2802);
                    exec.push_short(0x6000 | (x << 8));
                }
                Token::LABEL(_, _) => {}
            };
        }
    }
    return exec;
}

fn main() {
    let start_t = Instant::now();
    let mut job = parse_args();
    let tokens = job.tokenize();
    let executable = gen_executable(&tokens);
    job.write_output(executable);
    println!("Took {} seconds.", (Instant::now() - start_t).as_secs_f64())
}
