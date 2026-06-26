mod cli;
mod hardware;

use clap::Parser;

fn main() {
    let args = cli::Args::parse();
    let hw = hardware::detect(args.gpu.as_deref());

    if hw.gpus.is_empty() {
        println!("No GPU detected — CPU-only mode");
    } else {
        for gpu in &hw.gpus {
            println!("{} — {} MB VRAM, {:.0} GB/s", gpu.name, gpu.vram_mb, gpu.bandwidth_gbps);
        }
    }
    println!("CPU: {} ({} cores)", hw.cpu.name, hw.cpu.cores);
    println!("RAM: {:.1} GB", hw.ram_gb);
}
