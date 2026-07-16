use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use deskgraph_database::ManifestDatabase;
use deskgraph_domain::collect_health;
use deskgraph_scanner::{authorize_scope, scan_scope};
use deskgraph_telemetry::{Service, init_privacy_safe_logging};
use serde::Serialize;
use tracing::{error, info};

#[derive(Debug, Parser)]
#[command(
    name = "deskgraph",
    version,
    about = "Local-first computer context graph"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the privacy-safe runtime health contract.
    Health,
    /// Initialize or inspect the local manifest database.
    Manifest {
        #[command(subcommand)]
        command: ManifestCommand,
    },
    /// Manage explicitly authorized scan scopes.
    Scope {
        #[command(subcommand)]
        command: ScopeCommand,
    },
    /// Run a metadata-only manifest scan.
    Scan {
        #[command(subcommand)]
        command: ScanCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ManifestCommand {
    Init {
        #[arg(long)]
        database: PathBuf,
    },
    Stats {
        #[arg(long)]
        database: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum ScopeCommand {
    Add {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        path: PathBuf,
    },
    List {
        #[arg(long)]
        database: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum ScanCommand {
    Start {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
    },
}

fn main() -> ExitCode {
    let _logger_installed = init_privacy_safe_logging(Service::Cli);
    let cli = Cli::parse();

    match execute(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            error!(event = "cli_command_failed", error_code = code);
            eprintln!("Command failed: {code}");
            ExitCode::FAILURE
        }
    }
}

fn execute(cli: Cli) -> Result<(), &'static str> {
    match cli.command {
        Command::Health => emit_json(&collect_health(), "health_check_completed"),
        Command::Manifest { command } => match command {
            ManifestCommand::Init { database } => {
                let database = open_database(&database)?;
                emit_json(
                    &database.stats().map_err(|error| error.code())?,
                    "manifest_initialized",
                )
            }
            ManifestCommand::Stats { database } => {
                let database = open_database(&database)?;
                emit_json(
                    &database.stats().map_err(|error| error.code())?,
                    "manifest_stats_read",
                )
            }
        },
        Command::Scope { command } => match command {
            ScopeCommand::Add { database, path } => {
                let database = open_database(&database)?;
                let scope = authorize_scope(&database, &path).map_err(|error| error.code())?;
                emit_json(&scope, "scope_authorized")
            }
            ScopeCommand::List { database } => {
                let database = open_database(&database)?;
                let scopes = database.list_scopes().map_err(|error| error.code())?;
                emit_json(&scopes, "scope_list_read")
            }
        },
        Command::Scan { command } => match command {
            ScanCommand::Start { database, scope } => {
                let mut database = open_database(&database)?;
                let report = scan_scope(&mut database, scope).map_err(|error| error.code())?;
                info!(
                    event = "metadata_scan_completed",
                    scope_id = report.scope_id,
                    job_id = report.job_id,
                    discovered_files = report.discovered_files,
                    discovered_folders = report.discovered_folders,
                    skipped_entries = report.skipped_entries,
                    issue_count = report.issue_count,
                    elapsed_ms = report.elapsed_ms
                );
                print_json(&report)
            }
        },
    }
}

fn open_database(path: &Path) -> Result<ManifestDatabase, &'static str> {
    ManifestDatabase::open(path).map_err(|error| error.code())
}

fn emit_json<T: Serialize>(value: &T, event: &'static str) -> Result<(), &'static str> {
    print_json(value)?;
    info!(event = event);
    Ok(())
}

fn print_json<T: Serialize>(value: &T) -> Result<(), &'static str> {
    let json = serde_json::to_string_pretty(value).map_err(|_| "response_serialization_failed")?;
    println!("{json}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn clap_schema_is_internally_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn scan_requires_an_explicit_database_and_scope() {
        assert!(Cli::try_parse_from(["deskgraph", "scan", "start"]).is_err());
    }

    #[test]
    fn manifest_slice_runs_through_cli_handler() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database = directory.path().join("manifest.sqlite3");

        execute(Cli {
            command: Command::Manifest {
                command: ManifestCommand::Init {
                    database: database.clone(),
                },
            },
        })
        .expect("manifest init should pass");
        execute(Cli {
            command: Command::Scope {
                command: ScopeCommand::Add {
                    database: database.clone(),
                    path: directory.path().to_path_buf(),
                },
            },
        })
        .expect("scope add should pass");
        execute(Cli {
            command: Command::Scan {
                command: ScanCommand::Start { database, scope: 1 },
            },
        })
        .expect("scan should pass");
    }
}
