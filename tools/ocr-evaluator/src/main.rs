use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use deskgraph_ocr_evaluator::evaluate_paths;

#[derive(Debug, Parser)]
#[command(
    name = "deskgraph-ocr-evaluator",
    about = "Score one bounded DeskGraph OCR provider run against a versioned corpus"
)]
struct Args {
    /// Versioned corpus JSON. The evaluator reads metadata and expected text only.
    #[arg(long)]
    corpus: PathBuf,
    /// Provider-run JSON produced by a separate platform harness.
    #[arg(long)]
    run: PathBuf,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match evaluate_paths(&args.corpus, &args.run) {
        Ok(report) => match serde_json::to_string_pretty(&report) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(_) => fail("ocr_evaluation_serialization_failed"),
        },
        Err(code) => fail(code),
    }
}

fn fail(code: &'static str) -> ExitCode {
    eprintln!("OCR evaluation failed: {code}");
    ExitCode::FAILURE
}
