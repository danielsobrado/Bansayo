//! BMAG v1: 32-byte auxiliary datagram, independent of BIMU v1.
pub struct Packet {
    pub mac: [u8; 6],
    pub sensor: u8,
    pub channel: &'static str,
    pub seq: u32,
    pub time_us: u32,
    pub values: [i16; 3],
}

pub fn parse(b: &[u8]) -> Result<Packet, &'static str> {
    if b.len() != 32 || &b[..4] != b"BMAG" || b[4] != 1 {
        return Err("invalid magnetic packet");
    }
    let channel = match b[6] {
        0 => "mag0",
        1 => "mag1",
        _ => return Err("unknown magnetometer"),
    };
    let values = std::array::from_fn(|i| i16::from_le_bytes([b[24 + 2 * i], b[25 + 2 * i]]));
    if values.contains(&i16::MIN) {
        return Err("magnetic overflow");
    }
    Ok(Packet {
        mac: b[8..14].try_into().map_err(|_| "mac")?,
        sensor: b[5],
        channel,
        seq: u32::from_le_bytes(b[16..20].try_into().map_err(|_| "seq")?),
        time_us: u32::from_le_bytes(b[20..24].try_into().map_err(|_| "time")?),
        values,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnetic_wire_format() {
        let mut b = [0_u8; 32];
        b[..4].copy_from_slice(b"BMAG");
        b[4] = 1;
        b[6] = 1;
        b[8..14].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
        b[16..20].copy_from_slice(&42_u32.to_le_bytes());
        b[24..26].copy_from_slice(&(-100_i16).to_le_bytes());
        let packet = parse(&b).unwrap();
        assert_eq!(packet.channel, "mag1");
        assert_eq!(packet.values, [-100, 0, 0]);
        assert_eq!(packet.seq, 42);
        assert!(parse(&b[..31]).is_err());
        b[6] = 2;
        assert!(parse(&b).is_err());
    }
}
