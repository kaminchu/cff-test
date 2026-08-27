mod app;
mod assertion;
mod checker;
mod cli;
mod error;
mod event;
mod runtime;

fn main() {
    let cli = cli::Cli::parse_args();
    match cli.and_then(app::run) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(error.exit_code());
        }
    }
}
