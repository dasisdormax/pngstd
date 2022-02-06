use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /** The input file name */
    input_file_name: String,

    /** The output file name. If unset, an appropriate suffix is added to the input file name. */
    output_file_name: Option<String>,
    
    /** The compression level. */
    #[clap(short, long, default_value_t = 3)]
    level: i32,

    /** Do not overwrite an existing output file */
    #[clap(short, long)]
    noclobber: bool,

    /** Skip zstd compression */
    #[clap(short, long = "nocompress")]
    skip_compression: bool,
}

/** The compression mode for the file */
pub enum CompressionMode {
    /** 
     * Removes any internal PNG compression.
     * 
     * The png image data (IDAT chunks) is normally compressed using zlib.
     * This mode re-encodes the PNG without compression done.
     */
    NoCompress,
    /**
     * Compresses the PNG file with zstd.
     * 
     * In order to achieve the best compression, the png internal zlib
     * compression is removed first. Configure the compression strength
     * with the --level option.
     */
    CompressFile,
    _CompressIDAT
}

impl Args {
    pub fn get_input_file_name(&self) -> String {
        self.input_file_name.clone()
    }

    pub fn get_output_file_name(&self) -> String {
        self.output_file_name.clone().unwrap_or_else(|| {
            let noext = self.input_file_name.trim_end_matches(".png");
            match self.get_compression_mode() {
                CompressionMode::NoCompress => format!("{}.plain.png", noext),
                CompressionMode::CompressFile => format!("{}.png.zst", noext),
                CompressionMode::_CompressIDAT => format!("{}.sng", noext),
            }
            
        })
    }

    pub fn get_level(&self) -> i32 {
        self.level
    }

    pub fn is_noclobber(&self) -> bool {
        self.noclobber
    }

    pub fn get_compression_mode(&self) -> CompressionMode {
        if self.skip_compression {
            CompressionMode::NoCompress
        } else {
            CompressionMode::CompressFile
        }
    }
}