use crate::ConnectorError;
use sha1::Digest;

pub const API_PROTO_VER: u8 = 0;
pub const PROTO_FMT_PROTOBUF: u8 = 0;
pub const HEAD_LEN: usize = 44;

#[derive(Debug, Clone)]
pub struct ProtoHeader {
    pub proto_id: u32,
    pub proto_fmt_type: u8,
    pub proto_ver: u8,
    pub serial_no: u32,
    pub body_len: u32,
    pub sha1: [u8; 20],
}

pub fn pack_message(proto_id: u32, serial_no: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEAD_LEN + body.len());
    out.push(b'F');
    out.push(b'T');
    out.extend_from_slice(&proto_id.to_le_bytes());
    out.push(PROTO_FMT_PROTOBUF);
    out.push(API_PROTO_VER);
    out.extend_from_slice(&serial_no.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());

    let digest: [u8; 20] = sha1::Sha1::digest(body).into();
    out.extend_from_slice(&digest);

    out.extend_from_slice(&[0u8; 8]);
    out.extend_from_slice(body);
    out
}

pub fn unpack_header(buf: &[u8; HEAD_LEN]) -> Result<ProtoHeader, ConnectorError> {
    if buf[0] != b'F' || buf[1] != b'T' {
        return Err(ConnectorError::Protocol("invalid magic".to_string()));
    }
    let proto_id = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]);
    let proto_fmt_type = buf[6];
    let proto_ver = buf[7];
    let serial_no = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let body_len = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
    let mut sha1 = [0u8; 20];
    sha1.copy_from_slice(&buf[16..36]);

    Ok(ProtoHeader {
        proto_id,
        proto_fmt_type,
        proto_ver,
        serial_no,
        body_len,
        sha1,
    })
}
