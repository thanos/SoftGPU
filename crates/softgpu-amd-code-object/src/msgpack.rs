//! Bounded MessagePack decoder for AMDHSA metadata maps.

use crate::error::{CodeObjectError, Result};
use std::collections::BTreeMap;

const MAX_DEPTH: usize = 32;
const MAX_NODES: usize = 50_000;
const MAX_STRING: usize = 4096;
const MAX_ARRAY: usize = 4096;
const MAX_MAP: usize = 4096;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
    String(String),
    Array(Vec<Value>),
    Map(BTreeMap<String, Value>),
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
    nodes: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            nodes: 0,
        }
    }

    fn remain(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.remain() < n {
            return Err(CodeObjectError::MsgPack {
                detail: "truncated".into(),
            });
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn bump_node(&mut self) -> Result<()> {
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > MAX_NODES {
            return Err(CodeObjectError::LimitExceeded {
                detail: format!("msgpack nodes > {MAX_NODES}"),
            });
        }
        Ok(())
    }

    fn parse_value(&mut self, depth: usize) -> Result<Value> {
        if depth > MAX_DEPTH {
            return Err(CodeObjectError::LimitExceeded {
                detail: format!("msgpack depth > {MAX_DEPTH}"),
            });
        }
        self.bump_node()?;
        let b = self.u8()?;
        match b {
            0xc0 => Ok(Value::Nil),
            0xc2 => Ok(Value::Bool(false)),
            0xc3 => Ok(Value::Bool(true)),
            n if n <= 0x7f => Ok(Value::U64(u64::from(n))),
            n if (0xe0..=0xff).contains(&n) => Ok(Value::I64(i8::from_ne_bytes([n]) as i64)),
            0xcc => Ok(Value::U64(u64::from(self.u8()?))),
            0xcd => {
                let s = self.take(2)?;
                Ok(Value::U64(u64::from(u16::from_be_bytes([s[0], s[1]]))))
            }
            0xce => {
                let s = self.take(4)?;
                Ok(Value::U64(u64::from(u32::from_be_bytes([
                    s[0], s[1], s[2], s[3],
                ]))))
            }
            0xcf => {
                let s = self.take(8)?;
                Ok(Value::U64(u64::from_be_bytes([
                    s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
                ])))
            }
            0xd0 => Ok(Value::I64(i8::from_ne_bytes([self.u8()?]) as i64)),
            0xd1 => {
                let s = self.take(2)?;
                Ok(Value::I64(i16::from_be_bytes([s[0], s[1]]) as i64))
            }
            0xd2 => {
                let s = self.take(4)?;
                Ok(Value::I64(i32::from_be_bytes([s[0], s[1], s[2], s[3]]) as i64))
            }
            0xd3 => {
                let s = self.take(8)?;
                Ok(Value::I64(i64::from_be_bytes([
                    s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
                ])))
            }
            0xca => {
                let s = self.take(4)?;
                Ok(Value::F64(f32::from_bits(u32::from_be_bytes([
                    s[0], s[1], s[2], s[3],
                ])) as f64))
            }
            0xcb => {
                let s = self.take(8)?;
                Ok(Value::F64(f64::from_bits(u64::from_be_bytes([
                    s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
                ]))))
            }
            n if (0xa0..=0xbf).contains(&n) => self.parse_str((n - 0xa0) as usize),
            0xd9 => {
                let len = self.u8()? as usize;
                self.parse_str(len)
            }
            0xda => {
                let s = self.take(2)?;
                let len = u16::from_be_bytes([s[0], s[1]]) as usize;
                self.parse_str(len)
            }
            0xdb => {
                let s = self.take(4)?;
                let len = u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as usize;
                self.parse_str(len)
            }
            n if (0x90..=0x9f).contains(&n) => self.parse_array((n - 0x90) as usize, depth),
            0xdc => {
                let s = self.take(2)?;
                let len = u16::from_be_bytes([s[0], s[1]]) as usize;
                self.parse_array(len, depth)
            }
            0xdd => {
                let s = self.take(4)?;
                let len = u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as usize;
                self.parse_array(len, depth)
            }
            n if (0x80..=0x8f).contains(&n) => self.parse_map((n - 0x80) as usize, depth),
            0xde => {
                let s = self.take(2)?;
                let len = u16::from_be_bytes([s[0], s[1]]) as usize;
                self.parse_map(len, depth)
            }
            0xdf => {
                let s = self.take(4)?;
                let len = u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as usize;
                self.parse_map(len, depth)
            }
            // bin / ext — accept as opaque skip for forward-compat? Fail closed is safer.
            other => Err(CodeObjectError::MsgPack {
                detail: format!("unsupported type byte 0x{other:02x}"),
            }),
        }
    }

    fn parse_str(&mut self, len: usize) -> Result<Value> {
        if len > MAX_STRING {
            return Err(CodeObjectError::LimitExceeded {
                detail: format!("string len {len} > {MAX_STRING}"),
            });
        }
        let bytes = self.take(len)?;
        let s = std::str::from_utf8(bytes).map_err(|_| CodeObjectError::MsgPack {
            detail: "string not utf8".into(),
        })?;
        Ok(Value::String(s.to_string()))
    }

    fn parse_array(&mut self, len: usize, depth: usize) -> Result<Value> {
        if len > MAX_ARRAY {
            return Err(CodeObjectError::LimitExceeded {
                detail: format!("array len {len} > {MAX_ARRAY}"),
            });
        }
        let mut v = Vec::with_capacity(len.min(64));
        for _ in 0..len {
            v.push(self.parse_value(depth + 1)?);
        }
        Ok(Value::Array(v))
    }

    fn parse_map(&mut self, len: usize, depth: usize) -> Result<Value> {
        if len > MAX_MAP {
            return Err(CodeObjectError::LimitExceeded {
                detail: format!("map len {len} > {MAX_MAP}"),
            });
        }
        let mut map = BTreeMap::new();
        for _ in 0..len {
            let key = match self.parse_value(depth + 1)? {
                Value::String(s) => s,
                other => {
                    return Err(CodeObjectError::MsgPack {
                        detail: format!("map key not string: {other:?}"),
                    });
                }
            };
            let val = self.parse_value(depth + 1)?;
            map.insert(key, val);
        }
        Ok(Value::Map(map))
    }
}

/// Parse a single MessagePack value from `data` (must consume the root).
pub fn parse(data: &[u8]) -> Result<Value> {
    let mut c = Cursor::new(data);
    let v = c.parse_value(0)?;
    if c.pos != data.len() {
        // Trailing bytes are unusual for note payloads; fail closed.
        return Err(CodeObjectError::MsgPack {
            detail: format!("trailing {} bytes", data.len() - c.pos),
        });
    }
    Ok(v)
}

/// Encode helpers used by SoftGPU fixtures (not a general msgpack library).
pub mod encode {
    use super::Value;
    use std::collections::BTreeMap;

    pub fn nil() -> Vec<u8> {
        vec![0xc0]
    }

    pub fn bool(v: bool) -> Vec<u8> {
        vec![if v { 0xc3 } else { 0xc2 }]
    }

    pub fn u64(v: u64) -> Vec<u8> {
        if v <= 0x7f {
            vec![v as u8]
        } else if v <= u64::from(u8::MAX) {
            vec![0xcc, v as u8]
        } else if v <= u64::from(u16::MAX) {
            let mut o = vec![0xcd];
            o.extend_from_slice(&(v as u16).to_be_bytes());
            o
        } else if v <= u64::from(u32::MAX) {
            let mut o = vec![0xce];
            o.extend_from_slice(&(v as u32).to_be_bytes());
            o
        } else {
            let mut o = vec![0xcf];
            o.extend_from_slice(&v.to_be_bytes());
            o
        }
    }

    pub fn str(s: &str) -> Vec<u8> {
        let b = s.as_bytes();
        let mut o = Vec::new();
        if b.len() <= 31 {
            o.push(0xa0 | (b.len() as u8));
        } else if b.len() <= 255 {
            o.push(0xd9);
            o.push(b.len() as u8);
        } else {
            o.push(0xda);
            o.extend_from_slice(&(b.len() as u16).to_be_bytes());
        }
        o.extend_from_slice(b);
        o
    }

    pub fn array(items: &[Vec<u8>]) -> Vec<u8> {
        let mut o = Vec::new();
        if items.len() <= 15 {
            o.push(0x90 | (items.len() as u8));
        } else {
            o.push(0xdc);
            o.extend_from_slice(&(items.len() as u16).to_be_bytes());
        }
        for it in items {
            o.extend_from_slice(it);
        }
        o
    }

    pub fn map(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
        let mut o = Vec::new();
        if entries.len() <= 15 {
            o.push(0x80 | (entries.len() as u8));
        } else {
            o.push(0xde);
            o.extend_from_slice(&(entries.len() as u16).to_be_bytes());
        }
        for (k, v) in entries {
            o.extend_from_slice(&str(k));
            o.extend_from_slice(v);
        }
        o
    }

    pub fn value(v: &Value) -> Vec<u8> {
        match v {
            Value::Nil => nil(),
            Value::Bool(b) => bool(*b),
            Value::U64(n) => u64(*n),
            Value::I64(n) if *n >= 0 => u64(*n as u64),
            Value::I64(n) => {
                let mut o = vec![0xd3];
                o.extend_from_slice(&n.to_be_bytes());
                o
            }
            Value::F64(_) => nil(), // fixtures avoid floats
            Value::String(s) => str(s),
            Value::Array(a) => {
                let enc: Vec<_> = a.iter().map(value).collect();
                array(&enc)
            }
            Value::Map(m) => {
                let enc: Vec<_> = m
                    .iter()
                    .map(|(k, v)| (k.clone(), value(v)))
                    .collect::<Vec<_>>();
                map(&enc)
            }
        }
    }

    pub fn map_from_btree(m: &BTreeMap<String, Value>) -> Vec<u8> {
        value(&Value::Map(m.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_small_map() {
        let mut m = BTreeMap::new();
        m.insert("amdhsa.version".into(), Value::Array(vec![Value::U64(1), Value::U64(2)]));
        m.insert("amdhsa.target".into(), Value::String("amdgcn-amd-amdhsa--gfx1201".into()));
        let bytes = encode::map_from_btree(&m);
        let v = parse(&bytes).unwrap();
        match v {
            Value::Map(out) => {
                assert!(out.contains_key("amdhsa.version"));
                assert!(out.contains_key("amdhsa.target"));
            }
            _ => panic!("expected map"),
        }
    }
}
