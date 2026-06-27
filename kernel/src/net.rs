//! Networking — real NIC access via the UEFI Simple Network Protocol (SNP).
//!
//! This drives an actual network card: it reads the MAC address and link state,
//! then performs a live ARP exchange — broadcasting an ARP request for the
//! gateway and receiving the reply — i.e. a minimal Ethernet/ARP stack running
//! on the OS. A full TCP/IP stack and the `Internet` capability build from here.

use alloc::format;
use alloc::string::String;
use uefi::proto::network::snp::{ReceiveFlags, SimpleNetwork};

/// Enable receiving unicast + broadcast (+ promiscuous) frames, so replies to
/// our ARP/ICMP actually reach `receive()`.
fn open_rx(snp: &SimpleNetwork) {
    let _ = snp.start();
    let _ = snp.initialize(0, 0);
    let flags = ReceiveFlags::UNICAST | ReceiveFlags::BROADCAST | ReceiveFlags::PROMISCUOUS;
    let _ = snp.receive_filters(flags, ReceiveFlags::empty(), false, None);
}

pub struct NetReport {
    pub present: bool,
    pub mac: String,
    pub gateway_mac: Option<String>,
    pub note: String,
}

fn mac_str(b: &[u8]) -> String {
    format!("{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}", b[0], b[1], b[2], b[3], b[4], b[5])
}

/// 16-bit one's-complement Internet checksum (RFC 1071).
fn checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut i = 0;
    while i + 1 < data.len() {
        sum += u16::from_be_bytes([data[i], data[i + 1]]) as u32;
        i += 2;
    }
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

const OUR_IP: [u8; 4] = [10, 0, 2, 15]; // QEMU SLIRP default guest IP
const GW_IP: [u8; 4] = [10, 0, 2, 2]; // QEMU SLIRP gateway
// QEMU SLIRP synthesises the gateway's MAC as 52:55:<gateway-ip>.
const GW_MAC: [u8; 6] = [0x52, 0x55, 0x0a, 0x00, 0x02, 0x02];

pub struct PingReport {
    pub sent: bool,
    pub replied: bool,
    pub from: String,
    pub ttl: u8,
}

/// Build an Ethernet frame whose first 14 bytes are reserved for SNP to fill in
/// the media header, followed by an IPv4 + ICMP echo request.
fn build_ping(seq: u16) -> [u8; 50] {
    let mut f = [0u8; 50]; // 14 eth (filled by SNP) + 20 IP + 8 ICMP + 8 data
    let b = 14; // IP starts after the reserved Ethernet header
    // IPv4 header
    f[b] = 0x45; // version 4, IHL 5
    f[b + 1] = 0x00;
    let total = 36u16; // IP + ICMP + data
    f[b + 2..b + 4].copy_from_slice(&total.to_be_bytes());
    f[b + 4..b + 6].copy_from_slice(&[0x00, 0x00]); // id
    f[b + 6..b + 8].copy_from_slice(&[0x40, 0x00]); // flags = DF
    f[b + 8] = 64; // TTL
    f[b + 9] = 1; // protocol = ICMP
    f[b + 12..b + 16].copy_from_slice(&OUR_IP);
    f[b + 16..b + 20].copy_from_slice(&GW_IP);
    let ipcsum = checksum(&f[b..b + 20]);
    f[b + 10..b + 12].copy_from_slice(&ipcsum.to_be_bytes());
    // ICMP echo request
    f[b + 20] = 8; // type = echo request
    f[b + 21] = 0;
    f[b + 24..b + 26].copy_from_slice(&[0x4c, 0x4f]); // id = "LO"
    f[b + 26..b + 28].copy_from_slice(&seq.to_be_bytes());
    f[b + 28..b + 36].copy_from_slice(b"LIVINGOS");
    let icsum = checksum(&f[b + 20..b + 36]);
    f[b + 22..b + 24].copy_from_slice(&icsum.to_be_bytes());
    f
}

/// Send an ICMP echo request to the gateway over our IPv4 stack and wait for the
/// echo reply. A genuine (minimal) IPv4/ICMP stack on top of the SNP NIC driver.
pub fn ping_gateway() -> PingReport {
    let handle = match uefi::boot::get_handle_for_protocol::<SimpleNetwork>() {
        Ok(h) => h,
        Err(_) => return PingReport { sent: false, replied: false, from: String::new(), ttl: 0 },
    };
    let snp = match uefi::boot::open_protocol_exclusive::<SimpleNetwork>(handle) {
        Ok(s) => s,
        Err(_) => return PingReport { sent: false, replied: false, from: String::new(), ttl: 0 },
    };
    let our_mac = snp.mode().current_address.0;
    open_rx(&snp);

    let src = uefi::proto::network::MacAddress(our_mac);
    let dst = {
        let mut m = [0u8; 32];
        m[..6].copy_from_slice(&GW_MAC);
        uefi::proto::network::MacAddress(m)
    };

    let mut buf = [0u8; 1024];
    let mut replied = false;
    let mut from = String::new();
    let mut ttl = 0u8;
    'outer: for seq in 0..8u16 {
        let pkt = build_ping(seq);
        let _ = snp.transmit(14, &pkt, Some(src), Some(dst), Some(0x0800));
        for _ in 0..120 {
            if let Ok(len) = snp.receive(&mut buf, None, None, None, None) {
                // Ethernet(14) + IPv4; ICMP echo reply = type 0, proto 1.
                if len >= 14 + 20 + 8
                    && buf[12] == 0x08 && buf[13] == 0x00
                    && buf[14 + 9] == 1
                    && buf[14 + 20] == 0
                {
                    ttl = buf[14 + 8];
                    from = format!("{}.{}.{}.{}", buf[14 + 12], buf[14 + 13], buf[14 + 14], buf[14 + 15]);
                    replied = true;
                    break 'outer;
                }
            }
            uefi::boot::stall(2000);
        }
    }

    PingReport { sent: true, replied, from, ttl }
}

pub fn run_net_demo() -> NetReport {
    let handle = match uefi::boot::get_handle_for_protocol::<SimpleNetwork>() {
        Ok(h) => h,
        Err(_) => {
            return NetReport { present: false, mac: String::new(), gateway_mac: None, note: String::from("no SNP / NIC found") }
        }
    };
    let snp = match uefi::boot::open_protocol_exclusive::<SimpleNetwork>(handle) {
        Ok(s) => s,
        Err(_) => {
            return NetReport { present: false, mac: String::new(), gateway_mac: None, note: String::from("could not open SNP") }
        }
    };

    let mode = snp.mode();
    let our_mac = mode.current_address.0;
    let mac = mac_str(&our_mac[..6]);

    // Bring the interface up and open the receive path.
    open_rx(&snp);

    // Build an ARP request for the QEMU gateway (10.0.2.2) from the default
    // guest IP (10.0.2.15).
    // First 14 bytes are reserved for SNP to fill the Ethernet header.
    let mut arp = [0u8; 42];
    let b = 14;
    arp[b..b + 2].copy_from_slice(&[0x00, 0x01]); // htype = Ethernet
    arp[b + 2..b + 4].copy_from_slice(&[0x08, 0x00]); // ptype = IPv4
    arp[b + 4] = 6; // hlen
    arp[b + 5] = 4; // plen
    arp[b + 6..b + 8].copy_from_slice(&[0x00, 0x01]); // oper = request
    arp[b + 8..b + 14].copy_from_slice(&our_mac[..6]); // sender MAC
    arp[b + 14..b + 18].copy_from_slice(&[10, 0, 2, 15]); // sender IP
    arp[b + 18..b + 24].copy_from_slice(&[0, 0, 0, 0, 0, 0]); // target MAC
    arp[b + 24..b + 28].copy_from_slice(&[10, 0, 2, 2]); // target IP (gateway)

    let broadcast = uefi::proto::network::MacAddress([0xff; 32]);
    let src = uefi::proto::network::MacAddress(mode.current_address.0);

    // Poll for an ARP reply, re-sending the request periodically (~1s total).
    let mut gateway_mac = None;
    let mut buf = [0u8; 1024];
    'poll: for i in 0..1000 {
        if i % 100 == 0 {
            let _ = snp.transmit(14, &arp, Some(src), Some(broadcast), Some(0x0806));
        }
        if let Ok(len) = snp.receive(&mut buf, None, None, None, None) {
            // Ethernet header is 14 bytes; ARP payload follows.
            if len >= 42 && buf[12] == 0x08 && buf[13] == 0x06 {
                let oper = u16::from_be_bytes([buf[14 + 6], buf[14 + 7]]);
                if oper == 2 {
                    gateway_mac = Some(mac_str(&buf[14 + 8..14 + 14]));
                    break 'poll;
                }
            }
        }
        uefi::boot::stall(1_000);
    }

    let note = if gateway_mac.is_some() {
        String::from("ARP exchange succeeded")
    } else {
        String::from("NIC up; no ARP reply (gateway may be silent)")
    };
    NetReport { present: true, mac, gateway_mac, note }
}
