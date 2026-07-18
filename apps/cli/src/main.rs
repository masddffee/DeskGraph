use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use deskgraph_database::ManifestDatabase;
use deskgraph_domain::{ExtractionJobProgress, ScanJobProgress, collect_health};
use deskgraph_domain::{FileRelationDecisionKind, ProjectDecisionKind};
use deskgraph_extractors::{
    ExtractionLimits, cancel_extraction_job_at, create_extraction_job_at,
    create_screenshot_ocr_job_at, extraction_job_at, extraction_stats_at,
    image_metadata_for_job_at, recent_extraction_jobs_at, resume_extraction_job_at,
    run_extraction_job_at,
};
use deskgraph_projects::{
    check_exact_duplicate_at, decide_exact_duplicate_at, decide_file_version_at,
    decide_project_candidate_at, folder_profile_at, project_candidate_at, propose_project_at,
    recent_file_relation_candidates_at, recent_project_candidates_at, recent_screenshot_groups_at,
    screenshot_group_at, suggest_file_version_at, suggest_screenshot_groups_at,
    verify_exact_duplicate_at, verify_file_version_at,
};
use deskgraph_retrieval::{SearchRequest, SearchSourceFilter, search_at};
use deskgraph_scanner::{
    authorize_scope, comparison_key, create_scan_job, pause_scan_job, resume_scan_job,
    run_scan_job_batch, run_scan_job_to_terminal, scan_scope,
};
use deskgraph_telemetry::{Service, init_privacy_safe_logging};
use deskgraph_transactions::{action_plan_at, create_rename_preview_at, recent_action_plans_at};
use deskgraph_watcher::{
    WatchPolicy, advance_watch_event_at, observe_watch_path_at, recent_watch_events_at,
    watch_event_at,
};
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

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SearchSourceArg {
    All,
    Metadata,
    Content,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ProjectDecisionArg {
    Accept,
    Reject,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RelationDecisionArg {
    Accept,
    Reject,
}

impl From<RelationDecisionArg> for FileRelationDecisionKind {
    fn from(decision: RelationDecisionArg) -> Self {
        match decision {
            RelationDecisionArg::Accept => Self::Accepted,
            RelationDecisionArg::Reject => Self::Rejected,
        }
    }
}

impl From<ProjectDecisionArg> for ProjectDecisionKind {
    fn from(decision: ProjectDecisionArg) -> Self {
        match decision {
            ProjectDecisionArg::Accept => Self::Accepted,
            ProjectDecisionArg::Reject => Self::Rejected,
        }
    }
}

impl From<SearchSourceArg> for SearchSourceFilter {
    fn from(source: SearchSourceArg) -> Self {
        match source {
            SearchSourceArg::All => Self::All,
            SearchSourceArg::Metadata => Self::MetadataPath,
            SearchSourceArg::Content => Self::ExtractedText,
        }
    }
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
    /// Extract bounded local content or screenshot text from an already scanned file.
    Extract {
        #[command(subcommand)]
        command: ExtractCommand,
    },
    /// Search current local metadata and extracted text without embeddings.
    Search {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        query: String,
        #[arg(long)]
        scope: Option<i64>,
        #[arg(long, value_enum, default_value_t = SearchSourceArg::All)]
        source: SearchSourceArg,
        /// Match one ASCII alphanumeric file extension, with or without a leading dot.
        #[arg(long)]
        extension: Option<String>,
        /// Inclusive modified-time lower bound in Unix seconds.
        #[arg(long)]
        modified_since: Option<i64>,
        /// Exclusive modified-time upper bound in Unix seconds.
        #[arg(long)]
        modified_before: Option<i64>,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Ingest and reconcile bounded filesystem-change hints without trusting event paths.
    Watch {
        #[command(subcommand)]
        command: WatchCommand,
    },
    /// Create and inspect durable organization previews. No filesystem action is exposed.
    Organize {
        #[command(subcommand)]
        command: OrganizeCommand,
    },
    /// Read bounded, explainable folder and project facts from the current local manifest.
    Folder {
        #[command(subcommand)]
        command: FolderCommand,
    },
    /// Persist and explicitly correct explainable project-root candidates.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Check and revalidate bounded deterministic file-relation candidates.
    Relation {
        #[command(subcommand)]
        command: RelationCommand,
    },
    /// Inspect suggest-only cleanup review candidates. No filesystem action is exposed.
    Cleanup {
        #[command(subcommand)]
        command: CleanupCommand,
    },
    /// Generate a bounded synthetic metadata-scan fixture.
    Fixture {
        #[command(subcommand)]
        command: FixtureCommand,
    },
}

#[derive(Debug, Subcommand)]
enum WatchCommand {
    /// Persist one adapter/user-observed path after scope validation and debounce it by scope.
    Observe {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
        #[arg(long)]
        path: PathBuf,
    },
    /// Advance one durable event through stability validation and atomic manifest reconciliation.
    Advance {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        event: i64,
    },
    /// Read one path-free durable event status.
    Status {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        event: i64,
    },
    /// List the 20 most recent path-free durable event states.
    List {
        #[arg(long)]
        database: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum OrganizeCommand {
    /// Validate and durably journal a same-folder file rename preview without renaming anything.
    RenamePreview {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        new_name: String,
    },
    /// Read one explicit before/after preview by its durable plan ID.
    Status {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        plan: i64,
    },
    /// List the 20 most recent path-free plan summaries.
    List {
        #[arg(long)]
        database: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum FolderCommand {
    /// Build one read-only profile for an already scanned folder.
    Profile {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
        #[arg(long, conflicts_with = "path", required_unless_present = "path")]
        node: Option<i64>,
        #[arg(long, conflicts_with = "node", required_unless_present = "node")]
        path: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    /// Persist the current deterministic folder suggestion without accepting membership.
    Propose {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
        #[arg(long, conflicts_with = "path", required_unless_present = "path")]
        node: Option<i64>,
        #[arg(long, conflicts_with = "node", required_unless_present = "node")]
        path: Option<PathBuf>,
    },
    /// Append an explicit user accept/reject correction; repeated decisions are idempotent.
    Decide {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        project: i64,
        #[arg(long, value_enum)]
        decision: ProjectDecisionArg,
    },
    /// Read one explicit candidate including its current root path and latest decision.
    Status {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        project: i64,
    },
    /// List the 20 most recent path-free candidate summaries.
    List {
        #[arg(long)]
        database: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum RelationCommand {
    /// Compare two explicit scanned files byte-for-byte and suggest an exact duplicate relation.
    Duplicate {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
        #[arg(long)]
        left: PathBuf,
        #[arg(long)]
        right: PathBuf,
    },
    /// Revalidate one exact duplicate relation against current files and append an observation.
    Verify {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        relation: i64,
    },
    /// Revalidate current bytes, then append an explicit user accept/reject correction.
    Decide {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        relation: i64,
        #[arg(long, value_enum)]
        decision: RelationDecisionArg,
    },
    /// List the 20 most recent path-free relation histories; each requires live verification.
    List {
        #[arg(long)]
        database: PathBuf,
    },
    /// Suggest a directional version relation from two explicit numeric filename suffixes.
    Version {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
        #[arg(long)]
        first: PathBuf,
        #[arg(long)]
        second: PathBuf,
    },
    /// Revalidate one filename-version relation and append an immutable observation.
    VersionVerify {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        relation: i64,
    },
    /// Revalidate current filename evidence, then append an evidence-bound user correction.
    VersionDecide {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        relation: i64,
        #[arg(long, value_enum)]
        decision: RelationDecisionArg,
    },
}

#[derive(Debug, Subcommand)]
enum CleanupCommand {
    /// Find explainable screenshot review groups from current local provenance.
    #[command(name = "screenshot-groups")]
    Groups {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
    },
    /// Read one current screenshot group with explicit member paths.
    #[command(name = "screenshot-group-status")]
    GroupStatus {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        group: i64,
    },
    /// List recent path-free screenshot group histories and currentness.
    #[command(name = "screenshot-group-list")]
    GroupList {
        #[arg(long)]
        database: PathBuf,
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
        #[arg(long, conflicts_with = "path", required_unless_present = "path")]
        node: Option<i64>,
        #[arg(long, conflicts_with = "node", required_unless_present = "node")]
        path: Option<PathBuf>,
    },
    /// Create a durable extraction job without opening the source file.
    Create {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
        #[arg(long, conflicts_with = "path", required_unless_present = "path")]
        node: Option<i64>,
        #[arg(long, conflicts_with = "node", required_unless_present = "node")]
        path: Option<PathBuf>,
    },
    /// Create and complete one bounded screenshot OCR job in the foreground.
    OcrStart {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
        #[arg(long, conflicts_with = "path", required_unless_present = "path")]
        node: Option<i64>,
        #[arg(long, conflicts_with = "node", required_unless_present = "node")]
        path: Option<PathBuf>,
    },
    /// Create a durable screenshot OCR job without opening the source file.
    OcrCreate {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        scope: i64,
        #[arg(long, conflicts_with = "path", required_unless_present = "path")]
        node: Option<i64>,
        #[arg(long, conflicts_with = "node", required_unless_present = "node")]
        path: Option<PathBuf>,
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
    /// Read bounded structured image dimensions for one completed image job.
    ImageMetadata {
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        job: i64,
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
                path,
            } => {
                let node = resolve_extraction_node(&database, scope, node, path.as_deref())?;
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
                path,
            } => {
                let node = resolve_extraction_node(&database, scope, node, path.as_deref())?;
                let progress = create_extraction_job_at(&database, scope, node)
                    .map_err(|error| error.code())?;
                emit_extraction_progress(&progress, "content_extraction_created")
            }
            ExtractCommand::OcrStart {
                database,
                scope,
                node,
                path,
            } => {
                let node = resolve_extraction_node(&database, scope, node, path.as_deref())?;
                let created = create_screenshot_ocr_job_at(&database, scope, node)
                    .map_err(|error| error.code())?;
                let progress =
                    run_extraction_job_at(&database, created.job_id, ExtractionLimits::default())
                        .map_err(|error| error.code())?;
                emit_extraction_progress(&progress, "screenshot_ocr_runner_stopped")
            }
            ExtractCommand::OcrCreate {
                database,
                scope,
                node,
                path,
            } => {
                let node = resolve_extraction_node(&database, scope, node, path.as_deref())?;
                let progress = create_screenshot_ocr_job_at(&database, scope, node)
                    .map_err(|error| error.code())?;
                emit_extraction_progress(&progress, "screenshot_ocr_created")
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
            ExtractCommand::ImageMetadata { database, job } => {
                let metadata =
                    image_metadata_for_job_at(&database, job).map_err(|error| error.code())?;
                emit_json(&metadata, "image_metadata_read")
            }
        },
        Command::Search {
            database,
            query,
            scope,
            source,
            extension,
            modified_since,
            modified_before,
            limit,
        } => {
            let response = search_at(
                &database,
                SearchRequest {
                    query: &query,
                    scope_id: scope,
                    source: source.into(),
                    extension: extension.as_deref(),
                    modified_since_unix_seconds: modified_since,
                    modified_before_unix_seconds: modified_before,
                    limit,
                },
            )
            .map_err(|error| error.code())?;
            print_json(&response)?;
            info!(
                event = "local_search_completed",
                scope_id = scope,
                result_count = response.result_count,
                elapsed_ms = response.elapsed_ms,
                filters_applied = extension.is_some()
                    || modified_since.is_some()
                    || modified_before.is_some()
                    || !matches!(source, SearchSourceArg::All),
                mode = "lexical"
            );
            Ok(())
        }
        Command::Watch { command } => match command {
            WatchCommand::Observe {
                database,
                scope,
                path,
            } => {
                let progress =
                    observe_watch_path_at(&database, scope, &path, WatchPolicy::default())
                        .map_err(|error| error.code())?;
                emit_json(&progress, "watch_event_observed")
            }
            WatchCommand::Advance { database, event } => {
                let progress = advance_watch_event_at(&database, event, WatchPolicy::default())
                    .map_err(|error| error.code())?;
                emit_json(&progress, "watch_event_advanced")
            }
            WatchCommand::Status { database, event } => {
                let progress = watch_event_at(&database, event).map_err(|error| error.code())?;
                emit_json(&progress, "watch_event_status_read")
            }
            WatchCommand::List { database } => {
                let events = recent_watch_events_at(&database).map_err(|error| error.code())?;
                emit_json(&events, "watch_event_list_read")
            }
        },
        Command::Organize { command } => match command {
            OrganizeCommand::RenamePreview {
                database,
                scope,
                source,
                new_name,
            } => {
                let preview = create_rename_preview_at(&database, scope, &source, &new_name)
                    .map_err(|error| error.code())?;
                emit_json(&preview, "rename_preview_created")
            }
            OrganizeCommand::Status { database, plan } => {
                let preview = action_plan_at(&database, plan).map_err(|error| error.code())?;
                emit_json(&preview, "action_plan_status_read")
            }
            OrganizeCommand::List { database } => {
                let plans = recent_action_plans_at(&database).map_err(|error| error.code())?;
                emit_json(&plans, "action_plan_list_read")
            }
        },
        Command::Folder { command } => match command {
            FolderCommand::Profile {
                database,
                scope,
                node,
                path,
            } => {
                let node = resolve_manifest_node(
                    &database,
                    scope,
                    node,
                    path.as_deref(),
                    "folder_profile_source_not_found",
                    "folder_profile_source_not_found",
                    "folder_profile_source_selection_invalid",
                )?;
                let profile =
                    folder_profile_at(&database, scope, node).map_err(|error| error.code())?;
                print_json(&profile)?;
                info!(
                    event = "folder_profile_read",
                    scope_id = profile.scope_id,
                    folder_node_id = profile.folder_node_id,
                    descendant_file_count = profile.descendant_file_count,
                    descendant_folder_count = profile.descendant_folder_count,
                    project_suggested = profile.project_suggestion.is_some()
                );
                Ok(())
            }
        },
        Command::Project { command } => match command {
            ProjectCommand::Propose {
                database,
                scope,
                node,
                path,
            } => {
                let node = resolve_manifest_node(
                    &database,
                    scope,
                    node,
                    path.as_deref(),
                    "project_root_not_found",
                    "project_root_not_found",
                    "project_root_selection_invalid",
                )?;
                let candidate =
                    propose_project_at(&database, scope, node).map_err(|error| error.code())?;
                print_json(&candidate)?;
                info!(
                    event = "project_candidate_proposed",
                    project_id = candidate.project_id,
                    scope_id = candidate.scope_id,
                    root_folder_node_id = candidate.root_folder_node_id,
                    state = ?candidate.state,
                    confidence_basis_points = candidate.suggestion.confidence_basis_points
                );
                Ok(())
            }
            ProjectCommand::Decide {
                database,
                project,
                decision,
            } => {
                let candidate = decide_project_candidate_at(&database, project, decision.into())
                    .map_err(|error| error.code())?;
                print_json(&candidate)?;
                info!(
                    event = "project_candidate_decided",
                    project_id = candidate.project_id,
                    scope_id = candidate.scope_id,
                    root_folder_node_id = candidate.root_folder_node_id,
                    state = ?candidate.state,
                    decision_sequence = candidate
                        .latest_decision
                        .as_ref()
                        .map(|decision| decision.sequence)
                );
                Ok(())
            }
            ProjectCommand::Status { database, project } => {
                let candidate =
                    project_candidate_at(&database, project).map_err(|error| error.code())?;
                print_json(&candidate)?;
                info!(
                    event = "project_candidate_status_read",
                    project_id = candidate.project_id,
                    scope_id = candidate.scope_id,
                    root_folder_node_id = candidate.root_folder_node_id,
                    state = ?candidate.state
                );
                Ok(())
            }
            ProjectCommand::List { database } => {
                let candidates =
                    recent_project_candidates_at(&database).map_err(|error| error.code())?;
                emit_json(&candidates, "project_candidate_list_read")
            }
        },
        Command::Relation { command } => match command {
            RelationCommand::Duplicate {
                database,
                scope,
                left,
                right,
            } => {
                let candidate = check_exact_duplicate_at(&database, scope, &left, &right)
                    .map_err(|error| error.code())?;
                print_json(&candidate)?;
                info!(
                    event = "file_relation_duplicate_checked",
                    relation_id = candidate.relation_id,
                    scope_id = candidate.left.scope_id,
                    left_node_id = candidate.left.node_id,
                    right_node_id = candidate.right.node_id,
                    compared_bytes = candidate.evidence.compared_bytes,
                    state = ?candidate.state
                );
                Ok(())
            }
            RelationCommand::Verify { database, relation } => {
                let candidate =
                    verify_exact_duplicate_at(&database, relation).map_err(|error| error.code())?;
                print_json(&candidate)?;
                info!(
                    event = "file_relation_duplicate_verified",
                    relation_id = candidate.relation_id,
                    scope_id = candidate.left.scope_id,
                    left_node_id = candidate.left.node_id,
                    right_node_id = candidate.right.node_id,
                    compared_bytes = candidate.evidence.compared_bytes,
                    state = ?candidate.state
                );
                Ok(())
            }
            RelationCommand::Decide {
                database,
                relation,
                decision,
            } => {
                let candidate = decide_exact_duplicate_at(&database, relation, decision.into())
                    .map_err(|error| error.code())?;
                print_json(&candidate)?;
                info!(
                    event = "file_relation_candidate_decided",
                    relation_id = candidate.relation_id,
                    scope_id = candidate.left.scope_id,
                    left_node_id = candidate.left.node_id,
                    right_node_id = candidate.right.node_id,
                    state = ?candidate.state,
                    decision_sequence = candidate
                        .latest_decision
                        .as_ref()
                        .map(|decision| decision.sequence)
                );
                Ok(())
            }
            RelationCommand::List { database } => {
                let candidates =
                    recent_file_relation_candidates_at(&database).map_err(|error| error.code())?;
                emit_json(&candidates, "file_relation_candidate_list_read")
            }
            RelationCommand::Version {
                database,
                scope,
                first,
                second,
            } => {
                let candidate = suggest_file_version_at(&database, scope, &first, &second)
                    .map_err(|error| error.code())?;
                print_json(&candidate)?;
                info!(
                    event = "file_version_candidate_suggested",
                    relation_id = candidate.relation_id,
                    scope_id = candidate.older.scope_id,
                    older_node_id = candidate.older.node_id,
                    newer_node_id = candidate.newer.node_id,
                    older_version = candidate.evidence.older_version,
                    newer_version = candidate.evidence.newer_version,
                    state = ?candidate.state
                );
                Ok(())
            }
            RelationCommand::VersionVerify { database, relation } => {
                let candidate =
                    verify_file_version_at(&database, relation).map_err(|error| error.code())?;
                print_json(&candidate)?;
                info!(
                    event = "file_version_candidate_verified",
                    relation_id = candidate.relation_id,
                    scope_id = candidate.older.scope_id,
                    older_node_id = candidate.older.node_id,
                    newer_node_id = candidate.newer.node_id,
                    older_version = candidate.evidence.older_version,
                    newer_version = candidate.evidence.newer_version,
                    state = ?candidate.state
                );
                Ok(())
            }
            RelationCommand::VersionDecide {
                database,
                relation,
                decision,
            } => {
                let candidate = decide_file_version_at(&database, relation, decision.into())
                    .map_err(|error| error.code())?;
                print_json(&candidate)?;
                info!(
                    event = "file_version_candidate_decided",
                    relation_id = candidate.relation_id,
                    scope_id = candidate.older.scope_id,
                    older_node_id = candidate.older.node_id,
                    newer_node_id = candidate.newer.node_id,
                    older_version = candidate.evidence.older_version,
                    newer_version = candidate.evidence.newer_version,
                    state = ?candidate.state,
                    decision_sequence = candidate
                        .latest_decision
                        .as_ref()
                        .map(|decision| decision.sequence),
                    evidence_observation_id = candidate
                        .latest_decision
                        .as_ref()
                        .map(|decision| decision.evidence_observation_id)
                );
                Ok(())
            }
        },
        Command::Cleanup { command } => match command {
            CleanupCommand::Groups { database, scope } => {
                let discovery =
                    suggest_screenshot_groups_at(&database, scope).map_err(|error| error.code())?;
                print_json(&discovery)?;
                info!(
                    event = "screenshot_group_discovery_completed",
                    scope_id = discovery.scope_id,
                    evaluated_image_count = discovery.evaluated_image_count,
                    group_count = discovery.groups.len(),
                    cleanup_authorized = false
                );
                Ok(())
            }
            CleanupCommand::GroupStatus { database, group } => {
                let candidate =
                    screenshot_group_at(&database, group).map_err(|error| error.code())?;
                print_json(&candidate)?;
                info!(
                    event = "screenshot_group_status_read",
                    group_id = candidate.group_id,
                    scope_id = candidate.scope_id,
                    member_count = candidate.members.len(),
                    cleanup_authorized = false
                );
                Ok(())
            }
            CleanupCommand::GroupList { database } => {
                let candidates =
                    recent_screenshot_groups_at(&database).map_err(|error| error.code())?;
                emit_json(&candidates, "screenshot_group_list_read")
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

fn resolve_extraction_node(
    database_path: &Path,
    scope_id: i64,
    node_id: Option<i64>,
    source_path: Option<&Path>,
) -> Result<i64, &'static str> {
    resolve_manifest_node(
        database_path,
        scope_id,
        node_id,
        source_path,
        "extraction_source_not_found",
        "extractable_file_not_found",
        "extraction_source_selection_invalid",
    )
}

fn resolve_manifest_node(
    database_path: &Path,
    scope_id: i64,
    node_id: Option<i64>,
    source_path: Option<&Path>,
    canonical_not_found_code: &'static str,
    manifest_not_found_code: &'static str,
    invalid_selection_code: &'static str,
) -> Result<i64, &'static str> {
    match (node_id, source_path) {
        (Some(node_id), None) => Ok(node_id),
        (None, Some(source_path)) => {
            let canonical =
                std::fs::canonicalize(source_path).map_err(|_| canonical_not_found_code)?;
            open_database(database_path)?
                .node_id_for_path_key(scope_id, &comparison_key(&canonical))
                .map_err(|error| error.code())?
                .ok_or(manifest_not_found_code)
        }
        _ => Err(invalid_selection_code),
    }
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
        operation = ?progress.operation,
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

    fn bounded_png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![0_u8; 32];
        bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes[8..12].copy_from_slice(&13_u32.to_be_bytes());
        bytes[12..16].copy_from_slice(b"IHDR");
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        bytes
    }

    #[test]
    fn clap_schema_is_internally_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn scan_requires_an_explicit_database_and_scope() {
        assert!(Cli::try_parse_from(["deskgraph", "scan", "start"]).is_err());
        assert!(Cli::try_parse_from(["deskgraph", "scan", "run"]).is_err());
        assert!(Cli::try_parse_from(["deskgraph", "extract", "start"]).is_err());
        assert!(Cli::try_parse_from(["deskgraph", "extract", "ocr-start"]).is_err());
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
                    node: Some(node_id),
                    path: None,
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
    fn image_metadata_runs_through_cli_handler() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database = directory.path().join("manifest.sqlite3");
        let scope_path = directory.path().join("authorized");
        std::fs::create_dir(&scope_path).expect("scope should create");
        let source_path = scope_path.join("Screenshot.png");
        let mut png = vec![0_u8; 32];
        png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        png[8..12].copy_from_slice(&13_u32.to_be_bytes());
        png[12..16].copy_from_slice(b"IHDR");
        png[16..20].copy_from_slice(&1920_u32.to_be_bytes());
        png[20..24].copy_from_slice(&1080_u32.to_be_bytes());
        std::fs::write(&source_path, png).expect("image fixture should write");
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
                    node: Some(node_id),
                    path: None,
                },
            },
        })
        .expect("image extraction should pass");
        let manifest = ManifestDatabase::open(&database).expect("database should reopen");
        let job = manifest
            .recent_extraction_jobs()
            .expect("job should load")
            .into_iter()
            .next()
            .expect("job should exist");
        drop(manifest);
        execute(Cli {
            command: Command::Extract {
                command: ExtractCommand::ImageMetadata {
                    database: database.clone(),
                    job: job.job_id,
                },
            },
        })
        .expect("image metadata command should pass");
        let metadata = image_metadata_for_job_at(&database, job.job_id)
            .expect("metadata should remain queryable");
        assert_eq!((metadata.pixel_width, metadata.pixel_height), (1920, 1080));
    }

    #[test]
    fn screenshot_ocr_job_creation_validates_image_without_running_ocr() {
        let directory = tempfile::tempdir().expect("fixture root should exist");
        let database = directory.path().join("manifest.sqlite3");
        let scope_path = directory.path().join("authorized");
        std::fs::create_dir(&scope_path).expect("scope should create");
        let source_path = scope_path.join("Screenshot.png");
        std::fs::write(&source_path, bounded_png_header(640, 480)).expect("fixture should write");
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
                command: ExtractCommand::OcrCreate {
                    database: database.clone(),
                    scope: scope.id,
                    node: Some(node_id),
                    path: None,
                },
            },
        })
        .expect("bounded image validation should create the OCR job");

        let job = ManifestDatabase::open(&database)
            .expect("database should reopen")
            .recent_extraction_jobs()
            .expect("job should load")
            .into_iter()
            .next()
            .expect("job should exist");
        assert_eq!(
            job.operation,
            deskgraph_domain::ExtractionOperation::ScreenshotOcr
        );
        assert_eq!(job.status, deskgraph_domain::ExtractionStatus::Queued);
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
