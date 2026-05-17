use std::fmt;
use std::io::{self, Write};

pub fn write_stderr(args: fmt::Arguments<'_>) {
    let mut stderr = io::stderr().lock();
    let _ = write_stderr_to(&mut stderr, args);
}

fn write_stderr_to(writer: &mut impl Write, args: fmt::Arguments<'_>) -> io::Result<()> {
    writer.write_fmt(args)?;
    writer.write_all(b"\n")
}

#[macro_export]
macro_rules! safe_eprintln {
    () => {
        $crate::logging::write_stderr(format_args!(""))
    };
    ($($arg:tt)*) => {
        $crate::logging::write_stderr(format_args!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    struct BrokenPipeWriter;

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed stderr"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed stderr"))
        }
    }

    #[test]
    fn stderr_write_errors_do_not_panic() {
        let result = std::panic::catch_unwind(|| {
            let mut writer = BrokenPipeWriter;
            let _ = write_stderr_to(&mut writer, format_args!("diagnostic {}", 1));
        });

        assert!(result.is_ok());
    }
}
