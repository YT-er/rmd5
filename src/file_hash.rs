use crate::cache::{self, CacheMode};
use crate::md5::Md5;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

pub const DEFAULT_BUFFER_SIZE: usize = 1024 * 1024;

pub fn hash_path(path: &Path, mode: CacheMode, buffer_size: usize) -> io::Result<[u8; 16]> {
    if path == Path::new("-") {
        return hash_stdin(buffer_size);
    }

    if mode == CacheMode::Direct {
        let mut md5 = Md5::new();
        cache::direct_read(path, buffer_size, |chunk| md5.update(chunk))?;
        return Ok(md5.finalize());
    }

    let mut file = File::open(path)?;
    cache::prepare_file(&file, mode);
    hash_file(&mut file, mode, buffer_size)
}

pub fn hash_stdin(buffer_size: usize) -> io::Result<[u8; 16]> {
    let stdin = io::stdin();
    let mut locked = stdin.lock();
    hash_reader(&mut locked, buffer_size)
}

fn hash_file(file: &mut File, mode: CacheMode, buffer_size: usize) -> io::Result<[u8; 16]> {
    let mut md5 = Md5::new();
    let mut buf = vec![0u8; buffer_size.max(8192)];
    let mut offset = 0u64;

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        md5.update(&buf[..n]);
        cache::drop_cache(file, offset, n, mode);
        offset += n as u64;
    }

    Ok(md5.finalize())
}

fn hash_reader<R: Read>(reader: &mut R, buffer_size: usize) -> io::Result<[u8; 16]> {
    let mut md5 = Md5::new();
    let mut buf = vec![0u8; buffer_size.max(8192)];

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        md5.update(&buf[..n]);
    }

    Ok(md5.finalize())
}
