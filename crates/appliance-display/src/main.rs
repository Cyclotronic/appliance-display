use appliance_display_core::{ButtonDebouncer, EzioProtocol, Frame, Operation};
use appliance_display_transport::{DisplayTransport, SerialTransport};
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

const DEFAULT_DEVICE: &str = "/dev/ttyS1";
const DEFAULT_SOCKET: &str = "/run/appliance-display/control.sock";
const MAX_REQUEST_BYTES: u64 = 4096;

#[derive(Debug, Deserialize, Serialize)]
struct Envelope {
    version: u8,
    #[serde(flatten)]
    request: Request,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Write { line1: String, line2: String },
    Clear,
    Status,
    Button { timeout_ms: u64 },
}

#[derive(Debug, Deserialize, Serialize)]
struct Response {
    version: u8,
    ok: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    button: Option<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("appliance-display: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        print_help();
        return Ok(());
    }

    match args[0].as_str() {
        "probe" => probe(parse_device(&args[1..])?),
        "write" => write_command(&args[1..]),
        "clear" => clear_command(&args[1..]),
        "buttons" => buttons_command(&args[1..]),
        "status" => status_command(),
        "serve" => serve_command(&args[1..]),
        unknown => Err(format!("unknown command '{unknown}'; use --help").into()),
    }
}

fn parse_device(args: &[String]) -> Result<PathBuf, Box<dyn Error>> {
    let mut device = PathBuf::from(DEFAULT_DEVICE);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--device" => {
                index += 1;
                device = PathBuf::from(args.get(index).ok_or("--device requires a path")?);
            }
            option => return Err(format!("unexpected option '{option}'").into()),
        }
        index += 1;
    }
    Ok(device)
}

fn probe(device: PathBuf) -> Result<(), Box<dyn Error>> {
    let metadata = std::fs::metadata(&device)?;
    println!("profile: portwell-ezio-300-16x2");
    println!("device: {}", device.display());
    println!("present: true");
    println!(
        "character_device: {}",
        metadata.file_type().is_char_device()
    );
    println!("serial: 2400-N-8-1");
    println!("recovery_console: false");
    Ok(())
}

fn write_command(args: &[String]) -> Result<(), Box<dyn Error>> {
    let mut line1: Option<&str> = None;
    let mut line2: Option<&str> = None;
    let mut device = PathBuf::from(DEFAULT_DEVICE);
    let mut dry_run = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--line1" => {
                index += 1;
                line1 = Some(args.get(index).ok_or("--line1 requires text")?);
            }
            "--line2" => {
                index += 1;
                line2 = Some(args.get(index).ok_or("--line2 requires text")?);
            }
            "--device" => {
                index += 1;
                device = PathBuf::from(args.get(index).ok_or("--device requires a path")?);
            }
            "--dry-run" => dry_run = true,
            option => return Err(format!("unexpected option '{option}'").into()),
        }
        index += 1;
    }

    let line1 = line1.ok_or("--line1 is required")?;
    let line2 = line2.unwrap_or("");
    if daemon_available() && !dry_run {
        let response = daemon_request(&Request::Write {
            line1: line1.to_owned(),
            line2: line2.to_owned(),
        })?;
        require_ok(response)?;
        return Ok(());
    }
    let frame = Frame::new(line1, line2)?;
    let mut operations = EzioProtocol::initialize();
    operations.extend(EzioProtocol::write_frame(&frame));
    execute(&device, dry_run, &operations)
}

fn clear_command(args: &[String]) -> Result<(), Box<dyn Error>> {
    let dry_run = args.iter().any(|arg| arg == "--dry-run");
    let filtered: Vec<String> = args
        .iter()
        .filter(|arg| arg.as_str() != "--dry-run")
        .cloned()
        .collect();
    let device = parse_device(&filtered)?;
    if daemon_available() && !dry_run {
        require_ok(daemon_request(&Request::Clear)?)?;
        return Ok(());
    }
    execute(&device, dry_run, &EzioProtocol::clear())
}

fn buttons_command(args: &[String]) -> Result<(), Box<dyn Error>> {
    let watch = args.iter().any(|arg| arg == "--watch");
    let filtered: Vec<String> = args
        .iter()
        .filter(|arg| arg.as_str() != "--watch")
        .cloned()
        .collect();
    let device = parse_device(&filtered)?;
    if daemon_available() {
        let mut last_button: Option<String> = None;
        loop {
            let response = daemon_request(&Request::Button { timeout_ms: 250 })?;
            if !response.ok {
                return Err(response.message.into());
            }
            if watch {
                if response.button != last_button {
                    if let Some(button) = &response.button {
                        println!("{button}");
                    }
                    last_button = response.button;
                }
            } else {
                println!("{}", response.message);
            }
            if !watch {
                break;
            }
            thread::sleep(Duration::from_millis(75));
        }
        return Ok(());
    }
    let mut transport = SerialTransport::open(&device)?;
    let mut debouncer = ButtonDebouncer::default();

    loop {
        let current = transport.poll_button(Duration::from_millis(250))?;
        if let Some(button) = debouncer.update(current) {
            println!("{button}");
        } else if !watch {
            println!("none");
        }
        if !watch {
            break;
        }
        thread::sleep(Duration::from_millis(75));
    }
    Ok(())
}

fn status_command() -> Result<(), Box<dyn Error>> {
    if !daemon_available() {
        return Err("display daemon is not running".into());
    }
    require_ok(daemon_request(&Request::Status)?)
}

fn serve_command(args: &[String]) -> Result<(), Box<dyn Error>> {
    let mut device = PathBuf::from(DEFAULT_DEVICE);
    let mut socket = daemon_socket();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--device" => {
                index += 1;
                device = PathBuf::from(args.get(index).ok_or("--device requires a path")?);
            }
            "--socket" => {
                index += 1;
                socket = PathBuf::from(args.get(index).ok_or("--socket requires a path")?);
            }
            option => return Err(format!("unexpected option '{option}'").into()),
        }
        index += 1;
    }

    if let Some(parent) = socket.parent() {
        fs::create_dir_all(parent)?;
    }
    remove_stale_socket(&socket)?;
    let listener = UnixListener::bind(&socket)?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o660))?;
    let _socket_guard = SocketGuard(socket);
    let mut transport = SerialTransport::open(&device)?;
    transport.execute(&EzioProtocol::initialize())?;
    eprintln!(
        "appliance-display: serving on {}",
        _socket_guard.0.display()
    );

    for connection in listener.incoming() {
        match connection {
            Ok(mut stream) => {
                if let Err(error) = stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .and_then(|_| stream.set_write_timeout(Some(Duration::from_secs(2))))
                {
                    eprintln!("appliance-display: client timeout setup failed: {error}");
                    continue;
                }
                let response = handle_request(&mut stream, &mut transport)
                    .unwrap_or_else(|error| Response::error(error.to_string()));
                if let Err(error) = write_response(&mut stream, &response) {
                    eprintln!("appliance-display: response failed: {error}");
                }
            }
            Err(error) => eprintln!("appliance-display: connection failed: {error}"),
        }
    }
    Ok(())
}

fn write_response(stream: &mut UnixStream, response: &Response) -> Result<(), Box<dyn Error>> {
    serde_json::to_writer(&mut *stream, response)?;
    stream.write_all(b"\n")?;
    Ok(())
}

fn handle_request(
    stream: &mut UnixStream,
    transport: &mut SerialTransport,
) -> Result<Response, Box<dyn Error>> {
    let mut payload = Vec::new();
    stream
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut payload)?;
    if payload.len() as u64 > MAX_REQUEST_BYTES {
        return Err("request exceeds 4096 bytes".into());
    }
    let envelope: Envelope = serde_json::from_slice(&payload)?;
    if envelope.version != 1 {
        return Err(format!("unsupported protocol version {}", envelope.version).into());
    }
    match envelope.request {
        Request::Write { line1, line2 } => {
            let frame = Frame::new(&line1, &line2)?;
            transport.execute(&EzioProtocol::write_frame(&frame))?;
            Ok(Response::ok("frame written"))
        }
        Request::Clear => {
            transport.execute(&EzioProtocol::clear())?;
            Ok(Response::ok("display cleared"))
        }
        Request::Status => Ok(Response::ok("portwell-ezio-300-16x2 ready")),
        Request::Button { timeout_ms } => {
            let timeout = Duration::from_millis(timeout_ms.min(1000));
            let button = transport
                .poll_button(timeout)?
                .map(|value| value.to_string());
            Ok(Response {
                version: 1,
                ok: true,
                message: button.clone().unwrap_or_else(|| "none".to_owned()),
                button,
            })
        }
    }
}

fn daemon_available() -> bool {
    daemon_socket().exists()
}

fn daemon_request(request: &Request) -> Result<Response, Box<dyn Error>> {
    let mut stream = UnixStream::connect(daemon_socket())?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    serde_json::to_writer(
        &mut stream,
        &Envelope {
            version: 1,
            request: match request {
                Request::Write { line1, line2 } => Request::Write {
                    line1: line1.clone(),
                    line2: line2.clone(),
                },
                Request::Clear => Request::Clear,
                Request::Status => Request::Status,
                Request::Button { timeout_ms } => Request::Button {
                    timeout_ms: *timeout_ms,
                },
            },
        },
    )?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut payload = Vec::new();
    stream
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut payload)?;
    if payload.len() as u64 > MAX_REQUEST_BYTES {
        return Err("daemon response exceeds 4096 bytes".into());
    }
    Ok(serde_json::from_slice(&payload)?)
}

fn require_ok(response: Response) -> Result<(), Box<dyn Error>> {
    if response.ok {
        println!("{}", response.message);
        Ok(())
    } else {
        Err(response.message.into())
    }
}

impl Response {
    fn ok(message: impl Into<String>) -> Self {
        Self {
            version: 1,
            ok: true,
            message: message.into(),
            button: None,
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            version: 1,
            ok: false,
            message: message.into(),
            button: None,
        }
    }
}

fn remove_stale_socket(path: &Path) -> Result<(), Box<dyn Error>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => {
            if UnixStream::connect(path).is_ok() {
                return Err(
                    format!("display daemon is already listening on {}", path.display()).into(),
                );
            }
            fs::remove_file(path)?;
            Ok(())
        }
        Ok(_) => Err(format!("refusing to replace non-socket path {}", path.display()).into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn daemon_socket() -> PathBuf {
    env::var_os("APPLIANCE_DISPLAY_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SOCKET))
}

struct SocketGuard(PathBuf);

impl Drop for SocketGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn execute(device: &Path, dry_run: bool, operations: &[Operation]) -> Result<(), Box<dyn Error>> {
    if dry_run {
        for operation in operations {
            match operation {
                Operation::Instruction(value) => println!("instruction fe {value:02x}"),
                Operation::Data(value) => println!("data {}", String::from_utf8_lossy(value)),
            }
        }
        return Ok(());
    }
    let mut transport = SerialTransport::open(device)?;
    transport.execute(operations)?;
    Ok(())
}

fn print_help() {
    println!(
        "appliance-display 0.1.0\n\n\
         Usage:\n  \
         appliance-display probe [--device PATH]\n  \
         appliance-display write --line1 TEXT [--line2 TEXT] [--device PATH] [--dry-run]\n  \
         appliance-display clear [--device PATH] [--dry-run]\n\n\
         appliance-display buttons [--watch] [--device PATH]\n\n\
         appliance-display status\n  \
         appliance-display serve [--device PATH] [--socket PATH]\n\n\
         Default device: {DEFAULT_DEVICE}\n\
         Default socket: {DEFAULT_SOCKET}"
    );
}
