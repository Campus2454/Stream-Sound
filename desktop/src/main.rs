#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod cli;
mod gui;
mod os;
mod settings;
mod setup;
mod updater;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    if args == [os::QUIT_FLAG] {
        // The installers: close the running copy and wait for it to go.
        let gone = os::ask_to_quit(std::time::Duration::from_secs(10));
        std::process::exit(if gone { 0 } else { 1 });
    }
    #[cfg(target_os = "linux")]
    {
        use gui::setup::Mode;
        let mode = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
            [setup::INSTALL_FLAG] => Some(Mode::Install),
            [setup::UNINSTALL_FLAG] => Some(Mode::Uninstall),
            [setup::UPDATE_FLAG, target] => Some(Mode::Update(target.into())),
            // A downloaded file opened for the first time installs itself.
            [] if setup::linux::should_offer_install() => Some(Mode::Install),
            _ => None,
        };
        if let Some(mode) = mode {
            if let Err(e) = gui::setup::run(mode) {
                eprintln!("error: {e:#}");
                std::process::exit(1);
            }
            return;
        }
    }
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
