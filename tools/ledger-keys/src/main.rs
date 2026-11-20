use std::process::ExitCode;

use ledger_keys::{ledger_key_base64, Entry, USAGE};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 || args.len() > 3 {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    match Entry::parse(&args[1], args.get(2).map(String::as_str)) {
        Ok(entry) => {
            println!("{}", ledger_key_base64(&args[0], &entry));
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}
