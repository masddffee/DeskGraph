use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use deskgraph_ocr_evaluator::{NativeRunnerConfig, run_macos_vision};

#[derive(Debug, Parser)]
#[command(
    name = "deskgraph-macos-vision-runner",
    about = "Run bounded Apple Vision OCR over one explicit private DeskGraph corpus"
)]
struct Args {
    /// Versioned OCR ground-truth corpus JSON.
    #[arg(long)]
    corpus: PathBuf,
    /// Private case-to-image manifest. Do not commit this file.
    #[arg(long)]
    asset_manifest: PathBuf,
    /// Absolute, explicit root containing only manifest-addressed corpus images.
    #[arg(long)]
    images_root: PathBuf,
    /// New sensitive run JSON path. Existing files are never overwritten.
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    run_id: String,
    #[arg(long)]
    os_version: String,
    #[arg(long)]
    cpu_model: String,
    #[arg(long)]
    ram_bytes: u64,
    #[arg(long)]
    rust_toolchain: String,
    #[arg(long)]
    deskgraph_commit: String,
    #[arg(long)]
    runtime_revision: String,
    #[arg(long, default_value_t = 10_000)]
    case_timeout_ms: u64,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let config = NativeRunnerConfig {
        corpus_path: args.corpus,
        asset_manifest_path: args.asset_manifest,
        images_root: args.images_root,
        output_path: args.output,
        run_id: args.run_id,
        os_version: args.os_version,
        cpu_model: args.cpu_model,
        ram_bytes: args.ram_bytes,
        rust_toolchain: args.rust_toolchain,
        deskgraph_commit: args.deskgraph_commit,
        runtime_revision: args.runtime_revision,
        case_timeout_ms: args.case_timeout_ms,
    };
    match run_macos_vision(config) {
        Ok(()) => {
            println!("OCR native run completed");
            ExitCode::SUCCESS
        }
        Err(code) => {
            eprintln!("OCR native run failed: {code}");
            ExitCode::FAILURE
        }
    }
}
