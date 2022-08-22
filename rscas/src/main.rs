use std::env;
use rscas::{Job, Flag, FlagValue};

fn parse_args() -> Job {
    let mut job = Job::new();
    let mut args = env::args().into_iter();

    for _ in 0..args.len() {
        let arg = args.next().unwrap_or_default();
        match arg.as_str() {
            "-o" => {
                let file = args.next().unwrap_or_default();
                job.set_output(file.trim());
            }
            "-e" | "--entry" => {
                let entry = args.next().unwrap_or_default();
                job.add_flag(Flag::ENTRY, FlagValue::String(entry.trim().to_string()));
            }
            _ => {
                if arg.ends_with(".rsc") {
                    job.add_file(arg.as_str().trim());
                }
            }
        }
    }

    return job;
}

fn main() {
    let job = parse_args();
    job.run();
}
