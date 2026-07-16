use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use deskgraph_database::ManifestDatabase;
use deskgraph_domain::{ExtractionJobProgress, ScanJobProgress, collect_health};
use deskgraph_extractors::{
    ExtractionLimits, cancel_extraction_job_at, create_extraction_job_at, extraction_job_at,
    extraction_stats_at, recent_extraction_jobs_at, resume_extraction_job_at,
    run_extraction_job_at,
};
use deskgraph_scanner::{
    authorize_scope, create_scan_job, pause_scan_job, resume_scan_job, run_scan_job_batch,
    run_scan_job_to_terminal, scan_scope,
};
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
    /// Extract bounded local text from an already scanned file.
    Extract {
        #[command(subcommand)]
        command: ExtractCommand,
    },
    /// Generate a bounded synthetic metadata-scan fixture.
    Fixture {
        #[command(subcommand)]
        command: FixtureCommand,
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
    /// Create and complete a scan in the foreground.
    Start {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
    },
    /// Create a durable scan job without starting its runner.
    Create {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
    },
    /// Run a ready or resumed job until it completes or pauses.
    Run {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        job: i64,
    },
    /// Process one bounded batch and persist progress.
    Advance {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        job: i64,
        #[arg(long, default_value_t = 256)]
        batch_size: usize,
    },
    /// Read durable progress for one scan job.
    Status {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        job: i64,
    },
    /// List the 20 most recent durable scan jobs.
    List {
        #[arg(long)]
        database: PathBuf,
    },
    /// Request a safe pause between queue entries.
    Pause {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        job: i64,
    },
    /// Revalidate the authorized scope and make a paused job runnable.
    Resume {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        job: i64,
    },
}

#[derive(Debug, Subcommand)]
enum ExtractCommand {
    /// Create and complete one bounded extraction in the foreground.
    Start {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
        #[arg(long)]
        node: i64,
    },
    /// Create a durable extraction job without opening the source file.
    Create {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
        #[arg(long)]
        node: i64,
    },
    /// Run one queued extraction job to a terminal state.
    Run {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        job: i64,
    },
    /// Read durable progress without returning paths or extracted text.
    Status {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        job: i64,
    },
    /// List the 20 most recent privacy-safe extraction job summaries.
    List {
        #[arg(long)]
        database: PathBuf,
    },
    /// Request durable cancellation; a running provider stops between bounded work units.
    Cancel {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        job: i64,
    },
    /// Revalidate the source and make an interrupted job runnable.
    Resume {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        job: i64,
    },
    /// Read aggregate local extraction counts without paths or text.
    Stats {
        #[arg(long)]
        database: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum FixtureCommand {
    Generate {
        #[arg(long)]
        path: PathBuf,
        #[arg(long, default_value_t = 10_000)]
        files: u32,
        #[arg(long, default_value_t = 100)]
        directories: u32,
    },
}

#[derive(Debug, Serialize)]
struct FixtureReport {
    api_version: &'static str,
    generated_files: u32,
    generated_directories: u32,
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
            ScanCommand::Create { database, scope } => {
                let mut database = open_database(&database)?;
                let progress =
                    create_scan_job(&mut database, scope).map_err(|error| error.code())?;
                emit_scan_progress(&progress, "metadata_scan_created")
            }
            ScanCommand::Run { database, job } => {
                let mut database = open_database(&database)?;
                let progress =
                    run_scan_job_to_terminal(&mut database, job).map_err(|error| error.code())?;
                emit_scan_progress(&progress, "metadata_scan_runner_stopped")
            }
            ScanCommand::Advance {
                database,
                job,
                batch_size,
            } => {
                let mut database = open_database(&database)?;
                let progress = run_scan_job_batch(&mut database, job, batch_size)
                    .map_err(|error| error.code())?;
                emit_scan_progress(&progress, "metadata_scan_batch_stopped")
            }
            ScanCommand::Status { database, job } => {
                let database = open_database(&database)?;
                let progress = database.scan_job(job).map_err(|error| error.code())?;
                emit_scan_progress(&progress, "metadata_scan_status_read")
            }
            ScanCommand::List { database } => {
                let database = open_database(&database)?;
                let jobs = database.recent_scan_jobs().map_err(|error| error.code())?;
                emit_json(&jobs, "metadata_scan_list_read")
            }
            ScanCommand::Pause { database, job } => {
                let mut database = open_database(&database)?;
                let progress = pause_scan_job(&mut database, job).map_err(|error| error.code())?;
                emit_scan_progress(&progress, "metadata_scan_pause_requested")
            }
            ScanCommand::Resume { database, job } => {
                let mut database = open_database(&database)?;
                let progress = resume_scan_job(&mut database, job).map_err(|error| error.code())?;
                emit_scan_progress(&progress, "metadata_scan_resumed")
            }
        },
        Command::Extract { command } => match command {
            ExtractCommand::Start {
                database,
                scope,
                node,
            } => {
                let created = create_extraction_job_at(&database, scope, node)
                    .map_err(|error| error.code())?;
                let progress =
                    run_extraction_job_at(&database, created.job_id, ExtractionLimits::default())
                        .map_err(|error| error.code())?;
                emit_extraction_progress(&progress, "content_extraction_runner_stopped")
            }
            ExtractCommand::Create {
                database,
                scope,
                node,
            } => {
                let progress = create_extraction_job_at(&database, scope, node)
                    .map_err(|error| error.code())?;
                emit_extraction_progress(&progress, "content_extraction_created")
            }
            ExtractCommand::Run { database, job } => {
                let progress = run_extraction_job_at(&database, job, ExtractionLimits::default())
                    .map_err(|error| error.code())?;
                emit_extraction_progress(&progress, "content_extraction_runner_stopped")
            }
            ExtractCommand::Status { database, job } => {
                let progress = extraction_job_at(&database, job).map_err(|error| error.code())?;
                emit_extraction_progress(&progress, "content_extraction_status_read")
            }
            ExtractCommand::List { database } => {
                let jobs = recent_extraction_jobs_at(&database).map_err(|error| error.code())?;
                emit_json(&jobs, "content_extraction_list_read")
            }
            ExtractCommand::Cancel { database, job } => {
                let progress =
                    cancel_extraction_job_at(&database, job).map_err(|error| error.code())?;
                emit_extraction_progress(&progress, "content_extraction_cancel_requested")
            }
            ExtractCommand::Resume { database, job } => {
                let progress =
                    resume_extraction_job_at(&database, job).map_err(|error| error.code())?;
                emit_extraction_progress(&progress, "content_extraction_resumed")
            }
            ExtractCommand::Stats { database } => {
                let stats = extraction_stats_at(&database).map_err(|error| error.code())?;
                emit_json(&stats, "content_extraction_stats_read")
            }
        },
        Command::Fixture { command } => match command {
            FixtureCommand::Generate {
                path,
                files,
                directories,
            } => {
                let report = generate_fixture(&path, files, directories)?;
                emit_json(&report, "scan_fixture_generated")
            }
        },
    }
}

fn generate_fixture(
    path: &Path,
    files: u32,
    directories: u32,
) -> Result<FixtureReport, &'static str> {
    const MAX_FILES: u32 = 1_000_000;
    const MAX_DIRECTORIES: u32 = 10_000;
    if files > MAX_FILES {
        return Err("fixture_file_count_out_of_range");
    }
    if directories == 0 || directories > MAX_DIRECTORIES {
        return Err("fixture_directory_count_out_of_range");
    }
    if path.try_exists().map_err(|_| "fixture_path_check_failed")? {
        return Err("fixture_path_already_exists");
    }

    std::fs::create_dir_all(path).map_err(|_| "fixture_root_create_failed")?;
    let mut folders = Vec::with_capacity(directories as usize);
    for index in 0..directories {
        let folder = path.join(format!("group-{index:05}"));
        std::fs::create_dir(&folder).map_err(|_| "fixture_directory_create_failed")?;
        folders.push(folder);
    }
    for index in 0..files {
        let folder = &folders[(index % directories) as usize];
        let file = folder.join(format!("document-{index:07}.txt"));
        std::fs::write(file, b"DeskGraph metadata benchmark fixture\n")
            .map_err(|_| "fixture_file_create_failed")?;
    }

    Ok(FixtureReport {
        api_version: "deskgraph.fixture.v1",
        generated_files: files,
        generated_directories: directories,
    })
}

fn open_database(path: &Path) -> Result<ManifestDatabase, &'static str> {
    ManifestDatabase::open(path).map_err(|error| error.code())
}

fn emit_json<T: Serialize>(value: &T, event: &'static str) -> Result<(), &'static str> {
    print_json(value)?;
    info!(event = event);
    Ok(())
}

fn emit_scan_progress(progress: &ScanJobProgress, event: &'static str) -> Result<(), &'static str> {
    print_json(progress)?;
    info!(
        event = event,
        scope_id = progress.scope_id,
        job_id = progress.job_id,
        status = ?progress.status,
        queued_entries = progress.queued_entries,
        processed_entries = progress.processed_entries,
        discovered_files = progress.discovered_files,
        discovered_folders = progress.discovered_folders,
        skipped_entries = progress.skipped_entries,
        issue_count = progress.issue_count,
        elapsed_ms = progress.elapsed_ms
    );
    Ok(())
}

fn emit_extraction_progress(
    progress: &ExtractionJobProgress,
    event: &'static str,
) -> Result<(), &'static str> {
    print_json(progress)?;
    info!(
        event = event,
        scope_id = progress.scope_id,
        node_id = progress.node_id,
        job_id = progress.job_id,
        status = ?progress.status,
        source_bytes = progress.source_bytes,
        output_bytes = progress.output_bytes,
        chunk_count = progress.chunk_count,
        elapsed_ms = progress.elapsed_ms,
        cancel_requested = progress.cancel_requested
    );
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
        assert!(Cli::try_parse_from(["deskgraph", "scan", "run"]).is_err());
        assert!(Cli::try_parse_from(["deskgraph", "extract", "start"]).is_err());
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

    #[test]
    fn resumable_scan_controls_run_through_cli_handler() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database = directory.path().join("manifest.sqlite3");
        let scope_path = directory.path().join("authorized");
        std::fs::create_dir(&scope_path).expect("scope should create");
        std::fs::write(scope_path.join("note.md"), "metadata fixture")
            .expect("fixture should write");
        let mut manifest = ManifestDatabase::open(&database).expect("database should initialize");
        let scope = authorize_scope(&manifest, &scope_path).expect("scope should authorize");
        let job = create_scan_job(&mut manifest, scope.id).expect("job should create");
        drop(manifest);

        execute(Cli {
            command: Command::Scan {
                command: ScanCommand::Pause {
                    database: database.clone(),
                    job: job.job_id,
                },
            },
        })
        .expect("pause should pass");
        execute(Cli {
            command: Command::Scan {
                command: ScanCommand::Resume {
                    database: database.clone(),
                    job: job.job_id,
                },
            },
        })
        .expect("resume should pass");
        execute(Cli {
            command: Command::Scan {
                command: ScanCommand::Run {
                    database: database.clone(),
                    job: job.job_id,
                },
            },
        })
        .expect("run should pass");

        let manifest = ManifestDatabase::open(&database).expect("database should reopen");
        assert_eq!(
            manifest
                .scan_job(job.job_id)
                .expect("job should load")
                .status,
            deskgraph_domain::ScanStatus::Completed
        );
    }

    #[test]
    fn bounded_extraction_runs_through_cli_handler() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database = directory.path().join("manifest.sqlite3");
        let scope_path = directory.path().join("authorized");
        std::fs::create_dir(&scope_path).expect("scope should create");
        let source_path = scope_path.join("notes.md");
        std::fs::write(&source_path, "# DeskGraph\n本機 local-first context\n")
            .expect("fixture should write");
        let mut manifest = ManifestDatabase::open(&database).expect("database should initialize");
        let scope = authorize_scope(&manifest, &scope_path).expect("scope should authorize");
        scan_scope(&mut manifest, scope.id).expect("scope should scan");
        let node_id = manifest
            .node_id_for_path_key(
                scope.id,
                &deskgraph_scanner::comparison_key(
                    &std::fs::canonicalize(&source_path).expect("source should canonicalize"),
                ),
            )
            .expect("node lookup should pass")
            .expect("source node should exist");
        drop(manifest);

        execute(Cli {
            command: Command::Extract {
                command: ExtractCommand::Start {
                    database: database.clone(),
                    scope: scope.id,
                    node: node_id,
                },
            },
        })
        .expect("extraction should pass");

        let manifest = ManifestDatabase::open(&database).expect("database should reopen");
        let stats = manifest.extraction_stats().expect("stats should load");
        assert_eq!(stats.extracted_file_count, 1);
        assert!(stats.active_chunk_count > 0);
    }

    #[test]
    fn fixture_generator_is_bounded_and_never_overwrites() {
        let directory = tempfile::tempdir().expect("fixture parent should exist");
        let root = directory.path().join("generated");
        let report = generate_fixture(&root, 12, 3).expect("fixture should generate");

        assert_eq!(report.generated_files, 12);
        assert_eq!(report.generated_directories, 3);
        assert_eq!(
            generate_fixture(&root, 1, 1).expect_err("existing path must be preserved"),
            "fixture_path_already_exists"
        );
        assert_eq!(
            generate_fixture(&directory.path().join("too-many"), 1_000_001, 1)
                .expect_err("unbounded fixture must fail"),
            "fixture_file_count_out_of_range"
        );
    }
}
