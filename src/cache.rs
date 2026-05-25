use std::fs::File;
use std::io;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheMode {
    DropCache,
    KeepCache,
    Direct,
}

#[cfg(unix)]
pub fn prepare_file(file: &File, mode: CacheMode) {
    #[cfg(target_os = "linux")]
    linux::prepare_file(file, mode);

    #[cfg(target_os = "macos")]
    macos::prepare_file(file, mode);

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (file, mode);
    }
}

#[cfg(not(unix))]
pub fn prepare_file(_file: &File, _mode: CacheMode) {}

#[cfg(unix)]
pub fn drop_cache(file: &File, offset: u64, len: usize, mode: CacheMode) {
    #[cfg(target_os = "linux")]
    linux::drop_cache(file, offset, len, mode);

    #[cfg(target_os = "macos")]
    macos::drop_cache(file, offset, len, mode);

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (file, offset, len, mode);
    }
}

#[cfg(not(unix))]
pub fn drop_cache(_file: &File, _offset: u64, _len: usize, _mode: CacheMode) {}

#[cfg(target_os = "linux")]
pub fn direct_read(
    path: &std::path::Path,
    buf_size: usize,
    mut on_chunk: impl FnMut(&[u8]),
) -> io::Result<()> {
    linux::direct_read(path, buf_size, &mut on_chunk)
}

#[cfg(not(target_os = "linux"))]
pub fn direct_read(
    _path: &std::path::Path,
    _buf_size: usize,
    _on_chunk: impl FnMut(&[u8]),
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "--direct is only supported on Linux",
    ))
}

#[cfg(target_os = "linux")]
mod linux {
    use super::CacheMode;
    use std::ffi::{CString, c_char, c_int, c_void};
    use std::fs::File;
    use std::io;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr;

    const O_RDONLY: c_int = 0;
    const O_DIRECT: c_int = 0o40000;
    const POSIX_FADV_DONTNEED: c_int = 4;
    const POSIX_FADV_SEQUENTIAL: c_int = 2;

    unsafe extern "C" {
        fn open(pathname: *const c_char, flags: c_int, mode: c_int) -> c_int;
        fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
        fn close(fd: c_int) -> c_int;
        fn posix_fadvise(fd: c_int, offset: i64, len: i64, advice: c_int) -> c_int;
        fn posix_memalign(memptr: *mut *mut c_void, alignment: usize, size: usize) -> c_int;
        fn free(ptr: *mut c_void);
    }

    pub fn prepare_file(file: &File, mode: CacheMode) {
        if mode != CacheMode::KeepCache {
            let fd = file.as_raw_fd();
            unsafe {
                let _ = posix_fadvise(fd, 0, 0, POSIX_FADV_SEQUENTIAL);
            }
        }
    }

    pub fn drop_cache(file: &File, offset: u64, len: usize, mode: CacheMode) {
        if mode == CacheMode::DropCache {
            let fd = file.as_raw_fd();
            unsafe {
                let _ = posix_fadvise(fd, offset as i64, len as i64, POSIX_FADV_DONTNEED);
            }
        }
    }

    pub fn direct_read(
        path: &Path,
        buf_size: usize,
        on_chunk: &mut impl FnMut(&[u8]),
    ) -> io::Result<()> {
        let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "path contains an interior NUL byte",
            )
        })?;

        let fd = unsafe { open(c_path.as_ptr(), O_RDONLY | O_DIRECT, 0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        let mut ptr: *mut c_void = ptr::null_mut();
        let alignment = 4096;
        let alloc_size = round_up(buf_size.max(alignment), alignment);
        let rc = unsafe { posix_memalign(&mut ptr, alignment, alloc_size) };
        if rc != 0 {
            unsafe {
                close(fd);
            }
            return Err(io::Error::from_raw_os_error(rc));
        }

        let result = loop {
            let n = unsafe { read(fd, ptr, alloc_size) };
            if n < 0 {
                break Err(io::Error::last_os_error());
            }
            if n == 0 {
                break Ok(());
            }
            let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), n as usize) };
            on_chunk(bytes);
        };

        unsafe {
            free(ptr);
            let _ = close(fd);
        }

        result
    }

    fn round_up(value: usize, alignment: usize) -> usize {
        value.div_ceil(alignment) * alignment
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::CacheMode;
    use std::ffi::c_int;
    use std::fs::File;
    use std::os::fd::AsRawFd;

    const F_NOCACHE: c_int = 48;

    unsafe extern "C" {
        fn fcntl(fd: c_int, cmd: c_int, arg: c_int) -> c_int;
    }

    pub fn prepare_file(file: &File, mode: CacheMode) {
        if mode == CacheMode::DropCache {
            unsafe {
                let _ = fcntl(file.as_raw_fd(), F_NOCACHE, 1);
            }
        }
    }

    pub fn drop_cache(_file: &File, _offset: u64, _len: usize, _mode: CacheMode) {}
}
