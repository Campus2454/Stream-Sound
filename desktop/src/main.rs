#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod cli;
mod gui;
mod settings;
mod updater;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let res = if args.is_empty() { gui::run() } else { cli::run(args) };
    if let Err(e) = res {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
