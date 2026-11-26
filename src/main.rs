use clap::Parser;
use noctalia_setup::cli::Cli;

#[cfg(unix)]
fn restore_sigpipe() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn restore_sigpipe() {}

fn main() -> anyhow::Result<()> {
    restore_sigpipe();
    noctalia_setup::commands::run(Cli::parse())
}
