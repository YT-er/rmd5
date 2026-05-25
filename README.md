# rmd5

`rmd5` 是一个用 Rust 编写的 `md5sum` 兼容文件校验工具。

## 功能

- 输出格式兼容 `md5sum`：`<32位十六进制MD5><两个空格><文件名>`
- 支持 `md5sum -c` 风格的校验功能
- 多文件计算时支持多线程
- 默认使用 16 MiB 固定缓冲区流式读取，不会把整个文件读入内存
- MD5 核心使用成熟的 RustCrypto `md-5` 实现
- Linux 缓存控制：
  - 默认保留 page cache，优先速度
  - 可选 `--no-cache`：每读完一段后调用 `posix_fadvise(..., POSIX_FADV_DONTNEED)`
  - 可选 `--direct`：Linux 下使用 `O_DIRECT` 打开文件

## 使用方法

```sh
rmd5 file1 file2
rmd5 -j 8 file1 file2
rmd5 -c checksums.md5
rmd5 --no-cache big-file.iso
rmd5 --direct big-file.iso
rmd5 --buffer-size 33554432 file1
```

## 常用选项

```text
-c, --check FILE       从 FILE 读取 md5sum 格式的校验列表并校验
-j, --jobs N           worker 线程数，默认使用可用 CPU 数
-b, --binary           输出时在文件名前使用 `*`，兼容 md5sum -b
    --keep-cache       保留 page cache，默认选项，通常最快
    --no-cache         每读完一段后尽量丢弃对应 page cache
    --drop-cache       等同于 --no-cache
    --direct           仅 Linux：使用 O_DIRECT 读取文件
    --buffer-size N    读取缓冲区大小，单位字节，默认 16777216
-h, --help             显示帮助信息
```

## CentOS 7 构建

推荐构建 musl 静态版本：

```sh
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

本仓库已经配置 `x86_64-unknown-linux-musl` 使用 Rust 自带的 `rust-lld`
链接器，所以从 macOS 交叉编译时不需要额外安装 Linux linker。

生成的二进制文件在：

```text
target/x86_64-unknown-linux-musl/release/rmd5
```

如果需要 glibc 版本，建议在 CentOS 7 或兼容 CentOS 7 的容器里编译，避免生成的程序依赖过新的 glibc。
