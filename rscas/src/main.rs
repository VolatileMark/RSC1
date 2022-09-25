use num_derive::ToPrimitive;
use num_traits::ToPrimitive;
use rscas::{critical, Job, Token};
use std::{collections::HashMap, env, time::Instant};

#[derive(ToPrimitive)]
enum RegisterId {
    R7 = 0x07,
    SP = 0x08,
    C1 = 0x0A,
}

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
            Token::Label(id, address) => {
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

fn gen_bytes(tokens: &Vec<Token>) -> Vec<u8> {
    let mut bytes = Vec::new();
    let labels = collect_labels(tokens);
    let mut tokens_iter = tokens.iter();
    let mut push_short = |v: u16| {
        bytes.push((v & 0x00FF) as u8);
        bytes.push(((v >> 8) & 0x00FF) as u8);
    };
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
                Token::Short(short) => push_short(short),
                Token::Nop => push_short(0x0000),
                Token::And(x, y) => {
                    check_x(x, RegisterId::R7);
                    check_y(y, RegisterId::R7);
                    push_short(0x1000 | (x << 8) | (y << 4));
                }
                Token::Not(x) => {
                    check_x(x, RegisterId::R7);
                    push_short(0x1001 | (x << 8));
                }
                Token::Add(x, y) => {
                    check_x(x, RegisterId::R7);
                    check_y(y, RegisterId::R7);
                    push_short(0x2000 | (x << 8) | (y << 4));
                }
                Token::Sub(x, y) => {
                    check_x(x, RegisterId::R7);
                    check_y(y, RegisterId::R7);
                    push_short(0x2001 | (x << 8) | (y << 4));
                }
                Token::Inc(x) => {
                    check_x(x, RegisterId::SP);
                    push_short(0x2002 | (x << 8));
                }
                Token::Dec(x) => {
                    check_x(x, RegisterId::SP);
                    push_short(0x2003 | (x << 8));
                }
                Token::Ldb(x, y) => {
                    check_x(x, RegisterId::R7);
                    check_y(y, RegisterId::SP);
                    push_short(0x3000 | (x << 8) | (y << 4));
                }
                Token::Ldw(x, y) => {
                    check_x(x, RegisterId::R7);
                    check_y(y, RegisterId::SP);
                    push_short(0x3001 | (x << 8) | (y << 4));
                }
                Token::Mov(x, y) => {
                    check_x(x, RegisterId::C1);
                    check_y(y, RegisterId::C1);
                    push_short(0x3002 | (x << 8) | (y << 4));
                }
                Token::Ldi(x, nn) => {
                    check_x(x, RegisterId::R7);
                    push_short(0x4000 | (x << 8) | (nn as u16));
                }
                Token::Stb(y, x) => {
                    check_y(y, RegisterId::SP);
                    check_x(x, RegisterId::R7);
                    push_short(0x5000 | (y << 8) | (x << 4));
                }
                Token::Stw(y, x) => {
                    check_y(y, RegisterId::SP);
                    check_x(x, RegisterId::R7);
                    push_short(0x5001 | (y << 8) | (x << 4));
                }
                Token::Jmp(x) => {
                    check_x(x, RegisterId::SP);
                    push_short(0x6000 | (x << 8));
                }
                Token::Jnz(x, y) => {
                    check_x(x, RegisterId::SP);
                    check_y(y, RegisterId::R7);
                    push_short(0x6001 | (x << 8) | (y << 4));
                }
                Token::Shr(x, n) => {
                    check_x(x, RegisterId::R7);
                    push_short(0x7000 | (x << 8) | (((n & 0x0F) as u16) << 4));
                }
                Token::Shl(x, n) => {
                    check_x(x, RegisterId::R7);
                    push_short(0x7001 | (x << 8) | (((n & 0x0F) as u16) << 4));
                }
                Token::Test(n) => push_short(0x8000 | (((n & 0x0F) as u16) << 8)),
                Token::Setf(n) => push_short(0x8001 | (((n & 0x0F) as u16) << 8)),
                Token::Clrf(n) => push_short(0x8002 | (((n & 0x0F) as u16) << 8)),
                Token::Push(x) => {
                    check_x(x, RegisterId::R7);
                    push_short(0x2802);
                    push_short(0x5801 | (x << 4));
                }
                Token::Pop(x) => {
                    check_x(x, RegisterId::R7);
                    push_short(0x3081 | (x << 8));
                    push_short(0x2803);
                }
                Token::Ldl(x, k) => {
                    let a = match labels.get(&k) {
                        Some(a) => *a,
                        None => k as u16,
                    };
                    push_short(0x4000 | (x << 8) | ((a & 0xFF00) >> 8));
                    push_short(0x7081 | (x << 8));
                    push_short(0x4000 | (x << 8) | (a & 0x00FF));
                }
                _ => {}
            };
        }
    }
    return bytes;
}

fn main() {
    let start_t = Instant::now();
    let mut job = parse_args();
    let tokens = job.tokenize();
    let bytes = gen_bytes(&tokens);
    job.write_output(&bytes);
    println!("Took {} seconds.", (Instant::now() - start_t).as_secs_f64())
}
