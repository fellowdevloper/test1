use std::env;
use std::fs::File;
use std::io;
use std::process;

use test1::{Engine, Transaction};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <input.csv>", args[0]);
        process::exit(1);
    }

    let input_path = &args[1];

    let file = File::open(input_path).unwrap_or_else(|err| {
        eprintln!("Error opening {}: {}", input_path, err);
        process::exit(1);
    });

    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(file);

    let transactions = rdr.deserialize::<Transaction>().filter_map(|result| {
        result
            .map_err(|err| eprintln!("Error reading record: {}", err))
            .ok()
    });

    let mut engine = Engine::new();
    engine.process_all(transactions);

    let mut wtr = csv::Writer::from_writer(io::stdout());

    wtr.write_record(["client", "available", "held", "total", "locked"])
        .unwrap();

    for (client_id, account) in &engine.accounts {
        wtr.write_record(&[
            client_id.to_string(),
            format!("{:.4}", account.available),
            format!("{:.4}", account.held),
            format!("{:.4}", account.total()),
            account.locked.to_string(),
        ])
        .unwrap();
    }

    wtr.flush().unwrap();
}
