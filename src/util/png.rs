use flate2::Compression;
use flate2::write::ZlibEncoder;
use flate2::read::ZlibDecoder;
use std::io::BufWriter;
use std::ops::{Shl, Shr};
use std::io;
use std::io::Read;
use std::io::Write;
use std::fs::File;
use crc32fast::Hasher;

type ChunkTransform = dyn Fn(&ChunkInfo, Vec<u8>) -> io::Result<Option<Vec<u8>>>;

/** A reader that looks ahead at the PNG chunk metadata. */
pub struct PngReader<'t, R: Read> {
    reader: R,
    info: Option<ChunkInfo>,
    skip: Option<Result<(), io::Error>>,
    transform: &'t ChunkTransform,
}

#[derive(Clone, Debug)]
struct ChunkInfo {
    buf: [u8; 8],
}

fn int_to_bytes(n: u32) -> [u8;4] {
    let mut bytes = [0u8; 4];
    for i in 0..4 {
        bytes[i] = (n.shr(24 - 8 * i) % 256) as u8;
    }
    bytes
}

fn bytes_to_int(bytes: &[u8]) -> u32 {
    let mut n = 0u32;
    for i in 0..4 {
        n = n.shl(8) + bytes[i] as u32;
        //size += (self.buf[i] as u32).shl(8 * i);
    }
    n
}

impl ChunkInfo {
    fn new() -> ChunkInfo {
        ChunkInfo { buf: [0; 8] }
    }

    pub fn size(&self) -> u32 {
        bytes_to_int(&self.buf[..4])
    }

    pub fn set_size(&mut self, size: u32) {
        let bytes = int_to_bytes(size);
        for i in 0..4 {
            self.buf[i] = bytes[i];
        }
    }

    pub fn raw_type(&self) -> &[u8] {
        &self.buf[4..]
    }

    pub fn r#type(&self) -> String {
        let mut t = String::new();
        let raw = self.raw_type();
        for c in raw {
            t.push(*c as char);
        }
        t
    }
}

const PNG_HEADER: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

fn transform_noop(info: &ChunkInfo, buf: Vec<u8>) -> io::Result<Option<Vec<u8>>> {
    Ok(Some(buf))
}

fn transform_reflate(info: &ChunkInfo, buf: Vec<u8>) -> io::Result<Option<Vec<u8>>> {
    if !(info.raw_type()[0] as char).is_ascii_uppercase() {
        // Skip optional chunks. TODO: make this more configurable
        println!("Skipping chunk {}.", info.r#type());
        return Ok(None);
    }

    if info.r#type() != "IDAT" {
        // return chunk without transforming
        return Ok(Some(buf));
    }
    // inflate and dummy-deflate
    let mut inflate = ZlibDecoder::new(&*buf);
    let mut raw = Vec::new();
    inflate.read_to_end(&mut raw)?;

    let mut deflate = ZlibEncoder::new(Vec::new(), Compression::none());
    deflate.write_all(&mut raw)?;
    let out = deflate.finish()?;

    return Ok(Some(out));
}

impl<'t, R> PngReader<'t, R> 
where R: Read {
    fn new(reader: R, transform: &'t ChunkTransform) -> PngReader<'t, R> {
        PngReader { 
            reader,
            info: None,
            skip: None,
            transform
        }
    }

    pub fn check_header<W: Write>(&mut self, writer: &mut W) -> Result<(), io::Error> {
        if let Some(_skip) = &self.skip {
            return Ok(());
        }
        let mut buf = [0u8; 8];
        let bytes = self.reader.read(&mut buf)?;
        let mut mkerr = || {
            let err = Err(io::Error::new(io::ErrorKind::Other, "Invalid PNG signature!"));
            self.skip = Some(err);
            Ok(())
        };
        if bytes < 8 {
            return mkerr();
        }
        for i in 0..8 {
            if buf[i] != PNG_HEADER[i] {
                return mkerr();
            }
        }
        writer.write(&PNG_HEADER)?;
        self.skip = Some(Ok(()));
        Ok(())
    }

    fn look_ahead<W: Write>(&mut self, writer: &mut W) -> Result<(), io::Error> {
        if let None = self.info {
            self.check_header(writer)?;
            let mut info = ChunkInfo::new();
            let bytes = self.reader.read(&mut info.buf)?;
            if bytes < 8 {
                return Ok(());
            }
            self.info = Some(info)
        }
        Ok(())
    }

    fn chunk_data<W: Write>(&mut self, writer: &mut W) -> Result<Vec<u8>, io::Error> {
        let mut data = Vec::new();
        let mut info = match &self.info {
            Some(info) => info,
            None => return Ok(data)
        };
        let chunk_type = info.r#type();
        // Read data of subsequent chunks of same type
        while chunk_type == info.r#type() {
            let size = info.size() as usize;
            let mut buf = vec![0; size];
            let mut pos: usize = 0;
            // Read chunk data
            while pos < size {
                let bytes = self.reader.read(&mut buf[pos..])?;
                if bytes == 0 {
                    return Err(io::Error::new(io::ErrorKind::Other, 
                        format!("Error reading chunk, size is {}", bytes)
                    ));
                }
                pos += bytes;
            }
            // Validate checksum
            let mut hasher = Hasher::new();
            hasher.update(info.raw_type());
            hasher.update(&buf);
            let crc = hasher.finalize();
            {
                let mut crcbuf = [0u8; 4];
                let bytes = self.reader.read(&mut crcbuf)?;
                if bytes < 4 || bytes_to_int(&crcbuf) != crc {
                    return Err(io::Error::new(io::ErrorKind::Other, 
                        format!("Chunk {} checksum failed to validate!", chunk_type)
                    ));
                }
            }

            // Write bytes into total buffer
            data.write_all(&buf)?;

            // Check if next chunk is of the same type
            self.info = None;
            self.look_ahead(writer)?;
            info = match &self.info {
                Some(info) => info,
                None => break
            }
        }
        Ok(data)
    }

    fn write_chunk<W: Write>(&mut self, writer: &mut W) -> Result<(), io::Error> {
        self.look_ahead(writer)?;

        let mut info = match &self.info {
            None => return Ok(()),
            Some(info) => info.clone()
        };

        // read chunk data
        let buf = self.chunk_data(writer)?;
        
        // Transform IDAT chunk. If None is returned, do not output the chunk to the file.
        let buf = match (self.transform)(&info, buf)? {
            Some(buf) => buf,
            None => return Ok(()),
        };

        info.set_size(buf.len() as u32);
        println!("Writing chunk {} ({} bytes).", info.r#type(), info.size());

        writer.write(&info.buf)?;
        writer.write_all(&buf)?;

        // Calculate and Append CRC
        let mut hasher = Hasher::new();
        hasher.update(info.raw_type());
        hasher.update(&buf);
        let crc = hasher.finalize();
        let crc_bytes = int_to_bytes(crc);
        writer.write(&crc_bytes)?;
        
        Ok(())
    }

    pub fn has_data(&self) -> bool {
        if let None = self.skip {
            return true;
        }
        match self.info {
            Some(_) => true,
            _ => false
        }
    }

    pub fn write_all<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        while self.has_data() {
            self.write_chunk(writer)?;
        }
        Ok(())
    }
}

pub fn copy_reflate<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> Result<(), io::Error> {
    let mut png = PngReader::new(reader, &transform_reflate);
    png.write_all(writer)
}

pub fn copy_plain<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> io::Result<()> {
    let mut png = PngReader::new(reader, &transform_noop);
    png.write_all(writer)
}