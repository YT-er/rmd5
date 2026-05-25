# rmd5

`rmd5` is a small `md5sum`-compatible file checksum tool written in Rust.

Features:

- md5sum-style output: `<32 hex digest><two spaces><file>`
- `md5sum -c` style verification
- multi-threaded hashing for many files
- streaming reads with a fixed-size buffer
- Linux cache control:
  - default: `posix_fadvise(..., POSIX_FADV_DONTNEED)` after each chunk
  - optional: `--direct` opens files with `O_DIRECT`

## Usage

```sh
rmd5 file1 file2
rmd5 -j 8 file1 file2
rmd5 -c checksums.md5
rmd5 --direct big-file.iso
rmd5 --keep-cache file1
```

## CentOS 7 build

The most portable option is a musl build:

```sh
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

This repository configures the musl target to use Rust's bundled `rust-lld`
linker, so cross-compiling from macOS does not require installing a separate
Linux linker.

The binary will be:

```text
target/x86_64-unknown-linux-musl/release/rmd5
```

If you need a glibc binary, build inside CentOS 7 or a CentOS 7-compatible
container so the produced binary links against an old enough glibc.
