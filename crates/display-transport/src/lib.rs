//! Unix serial transport for the EZIO display bridge.

use appliance_display_core::{Button, EzioProtocol, Operation, COMMAND_PREFIX};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

const COMMAND_DELAY: Duration = Duration::from_millis(40);

pub trait DisplayTransport {
    fn execute(&mut self, operations: &[Operation]) -> io::Result<()>;
}

pub struct SerialTransport {
    file: File,
}

impl SerialTransport {
    pub fn open(device: &Path) -> io::Result<Self> {
        reject_console_device(device)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
            .open(device)?;
        let fd = file.as_raw_fd();

        if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::last_os_error());
        }

        configure_2400_8n1(fd)?;
        Ok(Self { file })
    }

    /// Poll the keypad once. `Ok(None)` means no single button is pressed.
    pub fn poll_button(&mut self, timeout: Duration) -> io::Result<Option<Button>> {
        let fd = self.file.as_raw_fd();
        if unsafe { libc::tcflush(fd, libc::TCIFLUSH) } != 0 {
            return Err(io::Error::last_os_error());
        }
        self.write_instruction(0x06)?;

        let milliseconds = timeout.as_millis().min(libc::c_int::MAX as u128) as libc::c_int;
        let mut descriptor = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut descriptor, 1, milliseconds) };
        if ready < 0 {
            return Err(io::Error::last_os_error());
        }
        if ready == 0 {
            return Ok(None);
        }

        let mut response = [0u8; 2];
        let mut received = 0;
        while received < response.len() {
            match self.file.read(&mut response[received..]) {
                Ok(0) => break,
                Ok(count) => received += count,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error),
            }
        }
        if received == 2 && response[0] == 0xfd {
            Ok(EzioProtocol::decode_button(response[1]))
        } else {
            Ok(None)
        }
    }

    fn write_instruction(&mut self, command: u8) -> io::Result<()> {
        self.file.write_all(&[COMMAND_PREFIX])?;
        self.file.flush()?;
        thread::sleep(COMMAND_DELAY);
        self.file.write_all(&[command])?;
        self.file.flush()?;
        thread::sleep(COMMAND_DELAY);
        Ok(())
    }
}

impl DisplayTransport for SerialTransport {
    fn execute(&mut self, operations: &[Operation]) -> io::Result<()> {
        for operation in operations {
            match operation {
                Operation::Instruction(command) => {
                    self.write_instruction(*command)?;
                }
                Operation::Data(data) => {
                    self.file.write_all(data)?;
                    self.file.flush()?;
                }
            }
        }
        Ok(())
    }
}

fn reject_console_device(device: &Path) -> io::Result<()> {
    let canonical = device
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(device));
    if canonical == Path::new("/dev/ttyS0") {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "/dev/ttyS0 is reserved for the recovery console",
        ));
    }
    Ok(())
}

fn configure_2400_8n1(fd: libc::c_int) -> io::Result<()> {
    let mut settings = std::mem::MaybeUninit::<libc::termios>::uninit();
    if unsafe { libc::tcgetattr(fd, settings.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let mut settings = unsafe { settings.assume_init() };
    unsafe { libc::cfmakeraw(&mut settings) };
    settings.c_cflag &= !(libc::CSIZE | libc::PARENB | libc::CSTOPB | libc::CRTSCTS);
    settings.c_cflag |= libc::CS8 | libc::CREAD | libc::CLOCAL;
    if unsafe { libc::cfsetispeed(&mut settings, libc::B2400) } != 0
        || unsafe { libc::cfsetospeed(&mut settings, libc::B2400) } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &settings) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::tcflush(fd, libc::TCIFLUSH) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[derive(Default)]
pub struct RecordingTransport {
    pub operations: Vec<Operation>,
}

impl DisplayTransport for RecordingTransport {
    fn execute(&mut self, operations: &[Operation]) -> io::Result<()> {
        self.operations.extend_from_slice(operations);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_transport_preserves_operations() {
        let expected = vec![Operation::Instruction(1), Operation::Data(b"test".to_vec())];
        let mut transport = RecordingTransport::default();
        transport.execute(&expected).unwrap();
        assert_eq!(transport.operations, expected);
    }

    #[test]
    fn recovery_console_is_rejected_before_open() {
        let error = match SerialTransport::open(Path::new("/dev/ttyS0")) {
            Ok(_) => panic!("recovery console unexpectedly accepted"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
}
