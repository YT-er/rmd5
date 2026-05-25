mod cache;
mod file_hash;
mod format;
mod md5;

use cache::CacheMode;
use file_hash::{DEFAULT_BUFFER_SIZE, hash_path, hash_stdin};
use format::{format_digest_line, parse_check_line};
use md5::{parse_hex, to_hex};
use std::collections::VecDeque;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

#[derive(Debug)]
struct Config {
    check_file: Option<PathBuf>,
    files: Vec<PathBuf>,
    jobs: usize,
    cache_mode: CacheMode,
    binary: bool,
    buffer_size: usize,
}

#[derive(Debug)]
struct HashResult {
    index: usize,
    path: PathBuf,
    result: io::Result<[u8; 16]>,
}

#[derive(Debug)]
struct CheckTask {
    index: usize,
    path: PathBuf,
    expected: [u8; 16],
}

#[derive(Debug)]
struct CheckResult {
    index: usize,
    path: PathBuf,
    status: CheckStatus,
}

#[derive(Debug)]
enum CheckStatus {
    Ok,
    Failed,
    ReadError(io::Error),
}

fn main() {
    let config = match parse_args(env::args().skip(1)) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("rmd5: {message}");
            eprintln!("可执行 'rmd5 --help' 查看帮助。");
            process::exit(2);
        }
    };

    if config.check_file.is_some() {
        process::exit(run_check(&config));
    }

    process::exit(run_hash(&config));
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Config, String> {
    let mut check_file = None;
    let mut files = Vec::new();
    let mut jobs = thread::available_parallelism().map_or(1, usize::from);
    let mut cache_mode = CacheMode::KeepCache;
    let mut binary = false;
    let mut buffer_size = DEFAULT_BUFFER_SIZE;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                process::exit(0);
            }
            "-c" | "--check" => {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("选项 '{arg}' 需要指定校验文件"))?;
                check_file = Some(PathBuf::from(value));
            }
            "-j" | "--jobs" => {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("选项 '{arg}' 需要指定线程数"))?;
                jobs = parse_positive_usize(&value, "线程数")?;
            }
            "-b" | "--binary" => binary = true,
            "--keep-cache" => cache_mode = CacheMode::KeepCache,
            "--drop-cache" | "--no-cache" => cache_mode = CacheMode::DropCache,
            "--direct" => cache_mode = CacheMode::Direct,
            "--buffer-size" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "选项 '--buffer-size' 需要指定字节数".to_string())?;
                buffer_size = parse_positive_usize(&value, "缓冲区大小")?;
            }
            "--" => {
                files.extend(iter.map(PathBuf::from));
                break;
            }
            _ if arg.starts_with("-j") && arg.len() > 2 => {
                jobs = parse_positive_usize(&arg[2..], "线程数")?;
            }
            _ if arg.starts_with("--jobs=") => {
                jobs = parse_positive_usize(&arg["--jobs=".len()..], "线程数")?;
            }
            _ if arg.starts_with("--check=") => {
                check_file = Some(PathBuf::from(&arg["--check=".len()..]));
            }
            _ if arg.starts_with("--buffer-size=") => {
                buffer_size = parse_positive_usize(&arg["--buffer-size=".len()..], "缓冲区大小")?;
            }
            _ if arg.starts_with('-') && arg != "-" => {
                return Err(format!("无法识别的选项 '{arg}'"));
            }
            _ => files.push(PathBuf::from(arg)),
        }
    }

    if check_file.is_some() && !files.is_empty() {
        return Err("--check 模式不支持额外的文件参数".to_string());
    }

    Ok(Config {
        check_file,
        files,
        jobs,
        cache_mode,
        binary,
        buffer_size,
    })
}

fn parse_positive_usize(value: &str, name: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{name} 无效：'{value}'"))?;
    if parsed == 0 {
        return Err(format!("{name} 必须大于 0"));
    }
    Ok(parsed)
}

fn print_help() {
    println!(
        "\
用法:
  rmd5 [选项] [文件]...
  rmd5 -c 校验文件 [选项]

选项:
  -c, --check FILE       从 FILE 读取 md5sum 格式的校验列表并校验
  -j, --jobs N           worker 线程数，默认使用可用 CPU 数
  -b, --binary           输出时在文件名前使用 '*'，兼容 md5sum -b
      --keep-cache       保留 page cache，默认选项，通常最快
      --no-cache         每读完一段后尽量丢弃对应 page cache
      --drop-cache       等同于 --no-cache
      --direct           仅 Linux：使用 O_DIRECT 读取文件
      --buffer-size N    读取缓冲区大小，单位字节，默认 16777216
  -h, --help             显示帮助信息
"
    );
}

fn run_hash(config: &Config) -> i32 {
    if config.files.is_empty() {
        match hash_stdin(config.buffer_size) {
            Ok(digest) => {
                println!("{}  -", to_hex(&digest));
                return 0;
            }
            Err(err) => {
                eprintln!("rmd5: -: {err}");
                return 1;
            }
        }
    }

    let tasks: Vec<_> = config.files.iter().cloned().enumerate().collect();
    let results = hash_many(tasks, config.jobs, config.cache_mode, config.buffer_size);
    let mut exit_code = 0;

    for result in results {
        match result.result {
            Ok(digest) => println!(
                "{}",
                format_digest_line(&to_hex(&digest), &result.path, config.binary)
            ),
            Err(err) => {
                eprintln!("rmd5: {}: {err}", result.path.display());
                exit_code = 1;
            }
        }
    }

    exit_code
}

fn hash_many(
    tasks: Vec<(usize, PathBuf)>,
    jobs: usize,
    cache_mode: CacheMode,
    buffer_size: usize,
) -> Vec<HashResult> {
    let task_count = tasks.len();
    let queue = Arc::new(Mutex::new(VecDeque::from(tasks)));
    let (tx, rx) = mpsc::channel();
    let workers = jobs.min(task_count).max(1);

    for _ in 0..workers {
        let queue = Arc::clone(&queue);
        let tx = tx.clone();
        thread::spawn(move || {
            loop {
                let task = queue.lock().expect("task queue poisoned").pop_front();
                let Some((index, path)) = task else {
                    break;
                };
                let result = hash_path(&path, cache_mode, buffer_size);
                if tx
                    .send(HashResult {
                        index,
                        path,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }
    drop(tx);

    let mut results: Vec<Option<HashResult>> = (0..task_count).map(|_| None).collect();
    for result in rx {
        let index = result.index;
        results[index] = Some(result);
    }

    results
        .into_iter()
        .map(|item| item.expect("worker result"))
        .collect()
}

fn run_check(config: &Config) -> i32 {
    let check_file = config.check_file.as_ref().expect("check file");
    let tasks = match read_check_tasks(check_file) {
        Ok(tasks) => tasks,
        Err(err) => {
            eprintln!("rmd5: {}: {err}", check_file.display());
            return 1;
        }
    };

    if tasks.is_empty() {
        eprintln!("rmd5: {}: 没有找到格式正确的校验行", check_file.display());
        return 1;
    }

    let results = check_many(tasks, config.jobs, config.cache_mode, config.buffer_size);
    let mut failed = 0usize;
    let mut read_errors = 0usize;

    for result in results {
        match result.status {
            CheckStatus::Ok => println!("{}: OK", result.path.display()),
            CheckStatus::Failed => {
                println!("{}: FAILED", result.path.display());
                failed += 1;
            }
            CheckStatus::ReadError(err) => {
                eprintln!("rmd5: {}: {err}", result.path.display());
                println!("{}: FAILED open or read", result.path.display());
                failed += 1;
                read_errors += 1;
            }
        }
    }

    if failed > 0 {
        let _ = io::stderr().flush();
        if read_errors > 0 {
            eprintln!("rmd5: 警告：有 {read_errors} 个列出的文件无法读取");
        }
        eprintln!("rmd5: 警告：有 {failed} 个计算出的校验值不匹配");
        1
    } else {
        0
    }
}

fn read_check_tasks(path: &Path) -> io::Result<Vec<CheckTask>> {
    if path == Path::new("-") {
        let stdin = io::stdin();
        let locked = stdin.lock();
        return read_check_tasks_from_reader("-", locked);
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);
    read_check_tasks_from_reader(&path.display().to_string(), reader)
}

fn read_check_tasks_from_reader<R: BufRead>(source: &str, reader: R) -> io::Result<Vec<CheckTask>> {
    let mut tasks = Vec::new();

    for (line_no, line) in reader.lines().enumerate() {
        let line = line?;
        let Some(parsed) = parse_check_line(&line) else {
            eprintln!("rmd5: {}:{}: MD5 校验行格式不正确", source, line_no + 1);
            continue;
        };

        let Some(expected) = parse_hex(&parsed.digest) else {
            eprintln!("rmd5: {}:{}: MD5 校验行格式不正确", source, line_no + 1);
            continue;
        };

        tasks.push(CheckTask {
            index: tasks.len(),
            path: parsed.path,
            expected,
        });
    }

    Ok(tasks)
}

fn check_many(
    tasks: Vec<CheckTask>,
    jobs: usize,
    cache_mode: CacheMode,
    buffer_size: usize,
) -> Vec<CheckResult> {
    let task_count = tasks.len();
    let queue = Arc::new(Mutex::new(VecDeque::from(tasks)));
    let (tx, rx) = mpsc::channel();
    let workers = jobs.min(task_count).max(1);

    for _ in 0..workers {
        let queue = Arc::clone(&queue);
        let tx = tx.clone();
        thread::spawn(move || {
            loop {
                let task = queue.lock().expect("task queue poisoned").pop_front();
                let Some(task) = task else {
                    break;
                };

                let status = match hash_path(&task.path, cache_mode, buffer_size) {
                    Ok(digest) if digest == task.expected => CheckStatus::Ok,
                    Ok(_) => CheckStatus::Failed,
                    Err(err) => CheckStatus::ReadError(err),
                };

                if tx
                    .send(CheckResult {
                        index: task.index,
                        path: task.path,
                        status,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
    }
    drop(tx);

    let mut results: Vec<Option<CheckResult>> = (0..task_count).map(|_| None).collect();
    for result in rx {
        let index = result.index;
        results[index] = Some(result);
    }

    results
        .into_iter()
        .map(|item| item.expect("worker result"))
        .collect()
}
