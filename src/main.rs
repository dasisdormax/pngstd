mod util;

use util::error::ErrorCode;
use std::io::BufReader;
use util::args::Args;
use util::args::CompressionMode;
use util::error::AppResult;
use clap::Parser;
use std::fs::File;
use std::path::Path;
use util::png::*;
use util::files::*;


fn reflate(infile: &File, outfile: &mut File, args: &Args) -> AppResult {
    let mut reader = BufReader::new(infile);
    match args.get_compression_mode() {
        CompressionMode::CompressFile => {
            let mut encoder = zstd::Encoder::new(outfile, args.get_level())?.auto_finish();
            copy_reflate(&mut reader, &mut encoder)
        }
        _ => copy_reflate(&mut reader, outfile)
    }
}

fn run(args: Args) -> AppResult {
    let infile = input_file(args.get_input_file_name(), &args)?;
    let mut outfile = output_file(args.get_output_file_name(), &args)?;

    reflate(&infile, &mut outfile, &args)
}

fn main() {
    let mut result = 0;
    {
        let args = Args::parse();
        if let Err(err) = run(args) {
            eprintln!("Error: {}", err);
            result = err.code();
        }
    }
    std::process::exit(result);
}
