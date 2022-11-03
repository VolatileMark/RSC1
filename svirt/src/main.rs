use svirt::{Configuration, VirtualMachine};
use std::env;
use std::sync::atomic::Ordering;

fn parse_args() -> Configuration {
    let mut config = Configuration::default();

    for arg in env::args().into_iter() {
        let mut name_val = arg.split("=");
        let name = name_val.next().unwrap_or("");
        let val = name_val.last().unwrap_or("");
        match name {
            "--memory" => {
                if val.is_empty() {
                    panic!("{} requires a value!", name);
                }
                let parsed = val.parse::<u16>();
                match parsed {
                    Ok(size) => config.memory_size = size,
                    Err(e) => panic!("{} has an invalid value.\n{}", name, e),
                }
            }
            "--cps" => {
                if val.is_empty() {
                    panic!("{} requires a value!", name);
                }
                let parsed = val.parse::<u128>();
                match parsed {
                    Ok(cps) => config.cycles_per_second = cps,
                    Err(e) => panic!("{} has an invalid value.\n{}", name, e),
                }
            }
            "--start-address" => {
                if val.is_empty() {
                    panic!("{} requires a value!", name);
                }
                let parsed = val.parse::<u16>();
                match parsed {
                    Ok(address) => config.initial_pc = address,
                    Err(e) => panic!("{} has an invalid value.\n{}", name, e),
                }
            }
            "--firmware" => {
                if val.is_empty() {
                    panic!("{} requires a value!", name);
                }
                config.firmware_file = val.to_string();
            }
            "--verbose" => {
                config.verbose = true;
            }
            _ => {}
        }
    }

    config.dump_to_stdout();
    return config;
}

fn main() {
    let config = parse_args();
    let mut vm = VirtualMachine::new(config);

    let should_run = vm.should_run.clone();
    _ = ctrlc::set_handler(move || {
        should_run.store(false, Ordering::Relaxed);
    });

    vm.reset();
    vm.run();
    vm.dump_to_stdout();
}
