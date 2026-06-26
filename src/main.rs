mod cli;

use clap::Parser;

fn main() {
    let args = cli::Args::parse();
    println!("whichllm — top {} results", args.top);
}
