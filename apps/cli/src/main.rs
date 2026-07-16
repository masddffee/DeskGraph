use std::process::ExitCode;

use deskgraph_domain::collect_health;
use deskgraph_telemetry::{Service, init_privacy_safe_logging};
use tracing::{error, info};

fn main() -> ExitCode {
    let _logger_installed = init_privacy_safe_logging(Service::Cli);
    let mut arguments = std::env::args().skip(1);

    match (arguments.next().as_deref(), arguments.next()) {
        (Some("health"), None) => emit_health(),
        (Some("--help" | "-h"), None) => {
            print_usage();
            ExitCode::SUCCESS
        }
        _ => {
            error!(event = "invalid_cli_arguments");
            eprintln!("Invalid command. Run `deskgraph --help` for usage.");
            ExitCode::from(2)
        }
    }
}

fn emit_health() -> ExitCode {
    let report = collect_health();

    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            info!(
                event = "health_check_completed",
                status = report.status,
                database_state = ?report.database.state
            );
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(_) => {
            error!(event = "health_serialization_failed");
            eprintln!("Unable to generate the health report.");
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    println!("DeskGraph CLI (pre-release)\n\nUSAGE:\n    deskgraph health");
}
