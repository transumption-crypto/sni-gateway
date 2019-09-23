use futures::future;

use log::Level::{Debug, Info, Warn};
use log::{log, log_enabled};

// See: https://tls.ulfheim.net
use rustls::internal::msgs::codec::{Codec, Reader};
use rustls::internal::msgs::enums::{ContentType, ProtocolVersion};
use rustls::internal::msgs::handshake::{
    HandshakeMessagePayload, HandshakePayload, ServerNamePayload,
};

use std::cell::RefCell;
use std::error::Error;
use std::io::Write;
use std::net::ToSocketAddrs;

use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};

use uuid::Uuid;

const TLS_HANDSHAKE_MAX_LENGTH: usize = 2048;
const TLS_RECORD_HEADER_LENGTH: usize = 5;

macro_rules! err {
    ($($arg:tt)*) => { Err(format!($($arg)*).into()) }
}

async fn peek(stream: &mut TcpStream, size: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut buf = vec![0; size];
    let n = stream.peek(&mut buf).await?;

    if n == size {
        Ok(buf)
    } else {
        err!("Peek size mismatch: {} != {}", n, size)
    }
}

async fn splice(inbound: TcpStream, outbound: TcpStream) -> Result<(), Box<dyn Error>> {
    let (mut ri, mut wi) = inbound.split();
    let (mut ro, mut wo) = outbound.split();

    // TODO: use splice(2) syscall
    let client_to_server = ri.copy(&mut wo);
    let server_to_client = ro.copy(&mut wi);

    future::try_join(client_to_server, server_to_client).await?;

    Ok(())
}

// TODO: figure out the correct way to do this
fn as_str<T: AsRef<str>>(s: T) -> String {
    format!("{}", s.as_ref())
}

async fn process(mut inbound: TcpStream) -> Result<(), Box<dyn Error>> {
    let buf = peek(&mut inbound, TLS_RECORD_HEADER_LENGTH).await?;
    let mut rd = Reader::init(&buf);

    let content_type = ContentType::read(&mut rd).unwrap();
    let protocol_version = ProtocolVersion::read(&mut rd).unwrap();
    let handshake_size = usize::from(u16::read(&mut rd).unwrap());

    log!(
        Debug,
        "Content type: {:?}, protocol version: {:?}, handshake size: {}",
        content_type,
        protocol_version,
        handshake_size
    );

    if content_type != ContentType::Handshake {
        return err!("TLS message is not a handshake");
    }

    if handshake_size > TLS_HANDSHAKE_MAX_LENGTH {
        return err!(
            "TLS handshake size is {} > {}",
            handshake_size,
            TLS_HANDSHAKE_MAX_LENGTH
        );
    }

    let buf = peek(&mut inbound, TLS_RECORD_HEADER_LENGTH + handshake_size).await?;
    let mut rd = Reader::init(&buf);
    rd.take(TLS_RECORD_HEADER_LENGTH);

    let handshake = HandshakeMessagePayload::read_version(&mut rd, protocol_version).unwrap();

    let client_hello = match handshake.payload {
        HandshakePayload::ClientHello(x) => x,
        _ => {
            return err!("TLS handshake is not Client Hello");
        }
    };

    let sni = match client_hello.get_sni_extension() {
        Some(x) => x,
        None => {
            return err!("Missing SNI");
        }
    };

    let host = match &sni[0].payload {
        ServerNamePayload::HostName(x) => x,
        ServerNamePayload::Unknown(_) => {
            return err!("Unknown SNI payload type");
        }
    };

    let host_str = as_str(host);

    log!(Debug, "SNI hostname: {}", host_str);

    let addr = match format!("{}:443", host_str).to_socket_addrs() {
        Ok(mut addrs) => addrs.next().unwrap(),
        Err(_) => {
            return err!("Failed to resolve {}", host_str);
        }
    };

    let outbound = TcpStream::connect(&addr).await?;
    splice(inbound, outbound).await
}

thread_local!(static UUID: RefCell<Uuid> = RefCell::new(Uuid::nil()));

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_default_env()
        .format(|buf, record| {
            UUID.with(|f| {
                writeln!(
                    buf,
                    "[{} {} {:<5} {}] {}",
                    buf.precise_timestamp(),
                    *f.borrow(),
                    buf.default_styled_level(record.level()),
                    record.target(),
                    record.args()
                )
            })
        })
        .init();

    let mut listener = TcpListener::bind("0.0.0.0:443").await?;

    loop {
        let (inbound, inbound_addr) = listener.accept().await?;

        tokio::spawn(async move {
            if log_enabled!(Warn) {
                UUID.with(|f| {
                    *f.borrow_mut() = Uuid::new_v4();
                });
            }

            log!(Info, "Accepted connection from {}", inbound_addr.ip());

            if let Err(e) = process(inbound).await {
                log!(Warn, "{}", e);
            }
        });
    }
}
