//! Generate offline-valid AirDropd product keys for donors.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use airdropd_core::licensing::key::generate_product_key;
use clap::Parser;
use rand::Rng;

/// Must match `core/src/licensing/mod.rs` KEY_SECRET.
const KEY_SECRET: &[u8] = b"AirDropd-RhythmicRecords-2026-v1";

#[derive(Parser, Debug)]
#[command(name = "airdropd-keygen", about = "Generate AirDropd product keys")]
struct Args {
    /// Number of keys to generate.
    #[arg(short = 'n', long = "count", default_value_t = 1)]
    count: u32,

    /// Append issued keys to a CSV ledger (for donor fulfillment).
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,

    /// Optional note stored in the CSV (donor name, CashApp handle, etc.).
    #[arg(short = 'N', long = "note", default_value = "")]
    note: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut rng = rand::thread_rng();
    let issued_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");

    for _ in 0..args.count {
        let serial: u32 = rng.gen();
        let key = generate_product_key(KEY_SECRET, serial);
        println!("{key}");

        if let Some(path) = &args.output {
            let new_file = !path.exists();
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            if new_file {
                writeln!(file, "issued_at,key,serial,note")?;
            }
            writeln!(file, "{issued_at},{key},{serial},{}", args.note)?;
        }
    }

    Ok(())
}
