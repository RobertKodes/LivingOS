//! Networking — real NIC access via the UEFI Simple Network Protocol (SNP).
//!
//! This drives an actual network card: it reads the MAC address and link state,
//! then performs a live ARP exchange — broadcasting an ARP request for the
//! gateway and receiving the reply — i.e. a minimal Ethernet/ARP stack running
//! on the OS. A full TCP/IP stack and the `Internet` capability build from here.

use alloc::format;
use alloc::string::String;
use uefi::proto::network::snp::SimpleNetwork;

pub struct NetReport {
    pub present: bool,
    pub mac: String,
    pub gateway_mac: Option<String>,
    pub note: String,
}

fn mac_str(b: &[u8]) -> String {
    format!("{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}", b[0], b[1], b[2], b[3], b[4], b[5])
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

    // Bring the interface up (ignore "already started/initialized").
    let _ = snp.start();
    let _ = snp.initialize(0, 0);

    // Build an ARP request for the QEMU gateway (10.0.2.2) from the default
    // guest IP (10.0.2.15).
    let mut arp = [0u8; 28];
    arp[0..2].copy_from_slice(&[0x00, 0x01]); // htype = Ethernet
    arp[2..4].copy_from_slice(&[0x08, 0x00]); // ptype = IPv4
    arp[4] = 6; // hlen
    arp[5] = 4; // plen
    arp[6..8].copy_from_slice(&[0x00, 0x01]); // oper = request
    arp[8..14].copy_from_slice(&our_mac[..6]); // sender MAC
    arp[14..18].copy_from_slice(&[10, 0, 2, 15]); // sender IP
    arp[18..24].copy_from_slice(&[0, 0, 0, 0, 0, 0]); // target MAC (unknown)
    arp[24..28].copy_from_slice(&[10, 0, 2, 2]); // target IP (gateway)

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
