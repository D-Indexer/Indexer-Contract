//! Reference event replayer: rebuilds contract state from `getEvents` output
//! and diffs it against live reads. See `tools/replay/README.md`.

mod decode;
mod live;
mod state;

use std::process::ExitCode;

use state::{diff, Event, State};

const USAGE: &str = "\
usage: replay --events <FILE|-> [--live <FILE>] [--contract <ID>] [--rpc-url <URL>]

  --events    recorded getEvents JSON (one object per line or an array), '-' for stdin
  --live      live state JSON (see tools/replay/README.md); omit to skip the diff
  --contract  contract ID to read live state from (requires --rpc-url)
  --rpc-url   Soroban RPC endpoint, e.g. https://soroban-testnet.stellar.org

exit codes: 0 no diffs, 1 diffs found, 2 usage or input error";

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> Result<ExitCode, String> {
    let mut events_path: Option<String> = None;
    let mut live_path: Option<String> = None;
    let mut contract: Option<String> = None;
    let mut rpc_url: Option<String> = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| iter.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--events" => events_path = Some(value("--events")?),
            "--live" => live_path = Some(value("--live")?),
            "--contract" => contract = Some(value("--contract")?),
            "--rpc-url" => rpc_url = Some(value("--rpc-url")?),
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(ExitCode::SUCCESS);
            }
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }

    let events_path = events_path.ok_or_else(|| format!("--events is required\n{USAGE}"))?;
    let raw = read_input(&events_path)?;
    let events: Vec<Event> = decode::decode_events(&raw)?;
    let rebuilt = State::replay(&events)?;
    println!(
        "replayed {} events: {} admins, {} identities, {} templates, {} portfolios",
        events.len(),
        rebuilt.admins.len(),
        rebuilt.owner_folder.len(),
        rebuilt.templates.len(),
        rebuilt.portfolios.len()
    );

    let live = match (live_path, contract, rpc_url) {
        (Some(path), _, _) => {
            let raw = read_input(&path)?;
            live::parse_live_state(&raw)?
        }
        (None, Some(contract), Some(rpc_url)) => live::fetch_live_state(&rpc_url, &contract)?,
        (None, Some(_), None) => return Err("--contract requires --rpc-url".to_string()),
        (None, None, _) => {
            println!("no --live or --contract given; skipping diff");
            return Ok(ExitCode::SUCCESS);
        }
    };

    let diffs = diff(&rebuilt, &live);
    if diffs.is_empty() {
        println!("no diffs: rebuilt state matches live reads");
        return Ok(ExitCode::SUCCESS);
    }
    for d in &diffs {
        println!(
            "diff {}: rebuilt={:?} live={:?}",
            d.key, d.rebuilt, d.live
        );
    }
    println!("{} diff(s) found", diffs.len());
    Ok(ExitCode::from(1))
}

fn read_input(path: &str) -> Result<String, String> {
    if path == "-" {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
            .map_err(|e| format!("reading stdin: {e}"))?;
        Ok(buf)
    } else {
        std::fs::read_to_string(path).map_err(|e| format!("reading {path}: {e}"))
    }
}
