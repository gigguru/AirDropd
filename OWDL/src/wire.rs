//! Safe big-endian read/write helpers for AWDL frames.

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read, Write};

pub struct WireWriter {
    buf: Vec<u8>,
}

impl Default for WireWriter {
    fn default() -> Self {
        Self { buf: Vec::new() }
    }
}

impl WireWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_u8(&mut self, v: u8) -> &mut Self {
        self.buf.push(v);
        self
    }

    pub fn write_u16(&mut self, v: u16) -> &mut Self {
        self.buf.write_u16::<BigEndian>(v).unwrap();
        self
    }

    pub fn write_u32(&mut self, v: u32) -> &mut Self {
        self.buf.write_u32::<BigEndian>(v).unwrap();
        self
    }

    pub fn write_u64(&mut self, v: u64) -> &mut Self {
        self.buf.write_u64::<BigEndian>(v).unwrap();
        self
    }

    pub fn write_bytes(&mut self, data: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(data);
        self
    }

    pub fn write_mac(&mut self, mac: &[u8; 6]) -> &mut Self {
        self.write_bytes(mac)
    }

    pub fn finish(self) -> Vec<u8> {
        self.buf
    }
}

pub struct WireReader<'a> {
    cursor: Cursor<&'a [u8]>,
}

impl<'a> WireReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            cursor: Cursor::new(data),
        }
    }

    pub fn remaining(&self) -> usize {
        (self.cursor.get_ref().len() as u64 - self.cursor.position()) as usize
    }

    pub fn read_u8(&mut self) -> Option<u8> {
        self.cursor.read_u8().ok()
    }

    pub fn read_u16(&mut self) -> Option<u16> {
        self.cursor.read_u16::<BigEndian>().ok()
    }

    pub fn read_u32(&mut self) -> Option<u32> {
        self.cursor.read_u32::<BigEndian>().ok()
    }

    pub fn read_u64(&mut self) -> Option<u64> {
        self.cursor.read_u64::<BigEndian>().ok()
    }

    pub fn read_bytes(&mut self, n: usize) -> Option<Vec<u8>> {
        if self.remaining() < n {
            return None;
        }
        let mut buf = vec![0u8; n];
        self.cursor.read_exact(&mut buf).ok()?;
        Some(buf)
    }

    pub fn read_mac(&mut self) -> Option<[u8; 6]> {
        let b = self.read_bytes(6)?;
        let mut mac = [0u8; 6];
        mac.copy_from_slice(&b);
        Some(mac)
    }
}
