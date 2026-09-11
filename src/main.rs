use std::process::ExitCode;

use thesis_rust_server::{config::Config, server};

fn main() -> ExitCode {
    let config = match Config::from_args(std::env::args()) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(error) = server::run(config) {
        eprintln!("server error: {error}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
