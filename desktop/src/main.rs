#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod cli;
mod gui;
mod os;
mod settings;
mod updater;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    let gui_flags = [os::MINIMIZED_FLAG, os::AFTER_UPDATE_FLAG];
    let res = if args.iter().all(|a| gui_flags.contains(&a.as_str())) {
        gui::run(flag(os::MINIMIZED_FLAG), flag(os::AFTER_UPDATE_FLAG))
    } else {
        cli::run(args)
    };
    if let Err(e) = res {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
