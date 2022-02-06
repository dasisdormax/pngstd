use super::args::Args;
use std::fs::File;
use std::io;
use std::path::Path;

pub fn input_file(name: String, _args: &Args) -> io::Result<File> {
    let path = Path::new(&name);
    File::open(&path)
}

pub fn output_file(name: String, args: &Args) -> io::Result<File> {
    let path = Path::new(&name);
    if args.is_noclobber() {
        File::options().create_new(true).open(&path)
    } else {
        File::create(&path)
    }
}