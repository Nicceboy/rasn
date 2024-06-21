use anyhow::{Context, Result};
use clap::{ArgGroup, Parser};
use fuzz::fuzz_types::{SequenceOptionals, SingleSizeConstrainedBitString};
use log::{error, info, warn, Level, LevelFilter, Metadata, Record};
// use rasn::prelude::*;
use rasn_smi::v2::ObjectSyntax;

static LOGGER: CodecLogger = CodecLogger;

struct CodecLogger;

impl log::Log for CodecLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }
    fn flush(&self) {}
}

/// Run crash cases of fuzzing from a directory or a single file.
#[derive(Parser, Debug)]
#[clap(group(ArgGroup::new("input").required(true).args(&["crash_file", "crash_dir"])))]
struct Cli {
    /// The path fo single crash file, e.g. out/oer/default/crashes/crash-1
    #[clap(long, value_parser)]
    crash_file: Option<String>,
    /// The crash directory, e.g. out/oer/default/crashes/
    #[arg(long, value_parser)]
    crash_dir: Option<String>,
    /// The codec to run the crash file (oer, der) for now, in future requires target type as well
    #[arg(long)]
    codec: String,
}

fn main() -> Result<()> {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(LevelFilter::Debug);
    let args = Cli::parse();
    let fuzz_fn = match &args.codec {
        // codec if codec == "oer" => fuzz::fuzz_oer::<Integer>,
        // codec if codec == "oer" => fuzz::fuzz_oer::<SingleSizeConstrainedBitString>,
        // codec if codec == "oer" => fuzz::fuzz_coer::<ObjectSyntax>,
        codec if codec == "oer" => fuzz::fuzz_oer::<SequenceOptionals>,
        // codec if codec == "der" => fuzz::fuzz_pkix,
        _ => fuzz::fuzz,
    };
    match (&args.crash_file, &args.crash_dir) {
        (Some(file), None) => {
            println!("Using file: {file}");
            run_single_file(file.to_owned().into(), fuzz_fn);
        }
        (None, Some(dir)) => {
            println!("Using directory: {dir}");
            run_from_dir(dir.to_owned().into(), fuzz_fn)?;
        }
        _ => unreachable!(), // clap ensures one of them is provided
    }

    Ok(())
}
fn run_from_dir(dir: std::path::PathBuf, fuzz_fn: fn(&[u8])) -> Result<()> {
    let crash_dir = std::fs::read_dir(dir)
        .with_context(|| "Could not find the crash directory.".to_string())?;

    for file in crash_dir.filter_map(Result::ok).map(|entry| entry.path()) {
        run_single_file(file, fuzz_fn);
    }
    Ok(())
}

fn run_single_file(file: std::path::PathBuf, fuzz_fn: fn(&[u8])) {
    let case = file.file_stem().unwrap().to_str().unwrap().to_owned();
    let result = std::panic::catch_unwind(|| {
        fuzz_fn(&std::fs::read(file).unwrap());
    });
    match result {
        Ok(_) => {
            println!("Testing Crash case: `{case}` - successful without panics");
        }
        Err(err) => {
            println!("Testing Crash case: `{case}` failed with panics: {err:?}");
        }
    }
}
