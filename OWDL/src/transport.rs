//! AWDL transport layer — UDP infrastructure mode (Windows-compatible).

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;

use bytes::Bytes;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket as TokioUdp;
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, error, info, warn};

use crate::constants::{INFRA_MULTICAST, INFRA_PORT};
use crate::frame::{DataFrame, PsfFrame};

/// Wire message types for infrastructure AWDL transport.
#[repr(u8)]
enum WireMsg {
    Psf = 1,
    Data = 2,
}

pub struct InfraTransport {
    socket: Arc<TokioUdp>,
    local_mac: [u8; 6],
    device_name: String,
    local_ipv4: Ipv4Addr,
    sequence: Arc<Mutex<u64>>,
    inbound: broadcast::Sender<TransportEvent>,
}

#[derive(Debug, Clone)]
pub enum TransportEvent {
    PeerDiscovered {
        mac: [u8; 6],
        name: String,
        ipv4: Option<Ipv4Addr>,
        sequence: u64,
    },
    DataReceived {
        src_mac: [u8; 6],
        payload: Bytes,
    },
}

impl InfraTransport {
    pub async fn new(device_name: String, local_mac: [u8; 6], local_ipv4: Ipv4Addr) -> anyhow::Result<Self> {
        let socket = Self::bind_multicast()?;
        let (inbound, _) = broadcast::channel(256);

        info!(
            "AWDL infrastructure transport on {}:{} (local {})",
            INFRA_MULTICAST, INFRA_PORT, local_ipv4
        );

        Ok(Self {
            socket: Arc::new(socket),
            local_mac,
            device_name,
            local_ipv4,
            sequence: Arc::new(Mutex::new(0)),
            inbound,
        })
    }

    fn bind_multicast() -> anyhow::Result<TokioUdp> {
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        socket.set_reuse_address(true)?;

        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let yes: libc::c_int = 1;
            unsafe {
                libc::setsockopt(
                    socket.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_REUSEPORT,
                    &yes as *const _ as *const libc::c_void,
                    std::mem::size_of_val(&yes) as libc::socklen_t,
                );
            }
        }

        let addr: SocketAddr = format!("0.0.0.0:{}", INFRA_PORT).parse()?;
        socket.bind(&addr.into())?;

        let multicast: Ipv4Addr = INFRA_MULTICAST.parse()?;
        let interfaces = local_ip_address::list_afinet_netifas()?;
        for (_, ip) in interfaces {
            if let std::net::IpAddr::V4(v4) = ip {
                if !v4.is_loopback() && !v4.is_link_local() {
                    let _ = socket.join_multicast_v4(&multicast, &v4);
                }
            }
        }

        socket.set_nonblocking(true)?;
        Ok(TokioUdp::from_std(UdpSocket::from(socket))?)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TransportEvent> {
        self.inbound.subscribe()
    }

    pub async fn run_tx(self: Arc<Self>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let multicast: SocketAddr = format!("{}:{}", INFRA_MULTICAST, INFRA_PORT).parse().unwrap();

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() { break; }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(crate::constants::PSF_INTERVAL_MS)) => {
                    let seq = {
                        let mut s = self.sequence.lock().await;
                        *s += 1;
                        *s
                    };
                    let psf = PsfFrame::build(
                        self.local_mac,
                        &self.device_name,
                        Some(self.local_ipv4),
                        seq,
                    );
                    let mut packet = vec![WireMsg::Psf as u8];
                    packet.extend_from_slice(&psf);
                    if let Err(e) = self.socket.send_to(&packet, multicast).await {
                        warn!("AWDL PSF send failed: {}", e);
                    }
                }
            }
        }
    }

    pub async fn run_rx(self: Arc<Self>, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut buf = vec![0u8; 4096];

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() { break; }
                }
                result = self.socket.recv_from(&mut buf) => {
                    match result {
                        Ok((n, _from)) => {
                            if n < 2 { continue; }
                            match buf[0] {
                                x if x == WireMsg::Psf as u8 => {
                                    if let Some(psf) = PsfFrame::parse(&buf[1..n]) {
                                        if psf.src_mac == self.local_mac { continue; }
                                        let _ = self.inbound.send(TransportEvent::PeerDiscovered {
                                            mac: psf.src_mac,
                                            name: psf.device_name.clone(),
                                            ipv4: psf.ipv4,
                                            sequence: psf.sequence,
                                        });
                                    }
                                }
                                x if x == WireMsg::Data as u8 => {
                                    if let Some(df) = DataFrame::parse(&buf[1..n]) {
                                        if df.dst_mac != self.local_mac && df.dst_mac != [0xff; 6] {
                                            continue;
                                        }
                                        let _ = self.inbound.send(TransportEvent::DataReceived {
                                            src_mac: df.src_mac,
                                            payload: df.payload.clone(),
                                        });
                                    }
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            error!("AWDL transport recv error: {}", e);
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        }
                    }
                }
            }
        }
    }

    pub async fn send_data(&self, dst_mac: [u8; 6], payload: Bytes) -> anyhow::Result<()> {
        let frame = DataFrame {
            src_mac: self.local_mac,
            dst_mac,
            ethertype: 0x0800,
            payload,
        };
        let mut packet = vec![WireMsg::Data as u8];
        packet.extend_from_slice(&frame.encode());

        let multicast: SocketAddr = format!("{}:{}", INFRA_MULTICAST, INFRA_PORT).parse()?;
        self.socket.send_to(&packet, multicast).await?;
        debug!("Sent AWDL data frame ({} bytes) to {:02x?}", packet.len(), dst_mac);
        Ok(())
    }
}

/// Generate a locally-unique AWDL MAC from device name.
pub fn derive_local_mac(seed: &str) -> [u8; 6] {
    let mut mac = [0u8; 6];
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in seed.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    mac[0] = 0x02;
    mac[1] = ((hash >> 40) & 0xff) as u8;
    mac[2] = ((hash >> 32) & 0xff) as u8;
    mac[3] = ((hash >> 24) & 0xff) as u8;
    mac[4] = ((hash >> 16) & 0xff) as u8;
    mac[5] = ((hash >> 8) & 0xff) as u8;
    mac
}

pub fn local_ipv4() -> anyhow::Result<Ipv4Addr> {
    let interfaces = local_ip_address::list_afinet_netifas()?;
    for (_, ip) in interfaces {
        if let std::net::IpAddr::V4(v4) = ip {
            if !v4.is_loopback() && !v4.is_link_local() {
                return Ok(v4);
            }
        }
    }
    Err(anyhow::anyhow!("no suitable IPv4 address"))
}
