use std::process::ExitCode;

fn main() -> ExitCode {
    // Initialize tracing for structured logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
        )
        .with_writer(std::io::stderr)
        .init();

    match axe_cli::run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("axe: {e}");
            ExitCode::FAILURE
        }
    }
}
