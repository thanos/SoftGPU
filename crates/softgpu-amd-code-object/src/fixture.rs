//! SoftGPU-owned minimal ELF64 + AMDGPU metadata fixtures (synthetic provenance).

use crate::msgpack::{encode, Value};
use crate::note::{NOTE_OWNER_AMDGPU, NT_AMDGPU_METADATA};
use std::collections::BTreeMap;

/// Build a minimal little-endian ELF64 ET_DYN with one `SHT_NOTE` metadata payload.
pub fn build_code_object_with_metadata(msgpack_desc: &[u8]) -> Vec<u8> {
    build_code_object_with_metadata_and_text(msgpack_desc, None)
}

/// SoftGPU llvm-mc `tiny_add` `.text` (duplicated for fixture independence).
const TINY_ADD_TEXT_FIXTURE: &[u8] = &[
    0x02, 0x20, 0x00, 0xf4, 0x00, 0x00, 0x00, 0xf8, 0x82, 0x20, 0x00, 0xf4, 0x08, 0x00, 0x00, 0xf8,
    0x00, 0x00, 0x89, 0xbf, 0x82, 0x00, 0x02, 0x30, 0x00, 0x00, 0x05, 0xee, 0x02, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x89, 0xbf, 0x81, 0x04, 0x04, 0x4a, 0x02, 0x80, 0x06, 0xee,
    0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xb0, 0xbf,
];

/// Build ELF64 with metadata note and optional `.text` bytes (SoftGPU executable fixtures).
pub fn build_code_object_with_metadata_and_text(
    msgpack_desc: &[u8],
    text: Option<&[u8]>,
) -> Vec<u8> {
    let note_payload = encode_note(NOTE_OWNER_AMDGPU, NT_AMDGPU_METADATA, msgpack_desc);
    let has_text = text.map(|t| !t.is_empty()).unwrap_or(false);
    let shstrtab: &[u8] = if has_text {
        b"\0.note\0.text\0.shstrtab\0"
    } else {
        b"\0.note\0.shstrtab\0"
    };

    let ehdr_size = 64usize;
    let note_off = ehdr_size;
    let note_size = note_payload.len();
    let text_off = align8(note_off + note_size);
    let text_size = text.map(|t| t.len()).unwrap_or(0);
    let shstr_off = if has_text {
        align8(text_off + text_size)
    } else {
        text_off
    };
    let shstr_size = shstrtab.len();
    let shoff = align8(shstr_off + shstr_size);
    let shentsize = 64usize;
    let shnum = if has_text { 4u16 } else { 3u16 };
    let shstrndx = if has_text { 3u16 } else { 2u16 };

    let mut buf = vec![0u8; shoff + shentsize * shnum as usize];

    buf[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    buf[4] = 2;
    buf[5] = 1;
    buf[6] = 1;
    write_u16(&mut buf, 16, 3);
    write_u16(&mut buf, 18, 0xe0);
    write_u32(&mut buf, 20, 1);
    write_u64(&mut buf, 40, shoff as u64);
    write_u16(&mut buf, 54, ehdr_size as u16);
    write_u16(&mut buf, 58, shentsize as u16);
    write_u16(&mut buf, 60, shnum);
    write_u16(&mut buf, 62, shstrndx);

    buf[note_off..note_off + note_size].copy_from_slice(&note_payload);
    if has_text {
        let t = text.unwrap();
        buf[text_off..text_off + text_size].copy_from_slice(t);
    }
    buf[shstr_off..shstr_off + shstr_size].copy_from_slice(shstrtab);

    write_section(
        &mut buf,
        shoff + shentsize,
        1,
        7,
        note_off as u64,
        note_size as u64,
    );
    if has_text {
        write_section(
            &mut buf,
            shoff + shentsize * 2,
            7,
            1,
            text_off as u64,
            text_size as u64,
        );
        write_section(
            &mut buf,
            shoff + shentsize * 3,
            13,
            3,
            shstr_off as u64,
            shstr_size as u64,
        );
    } else {
        write_section(
            &mut buf,
            shoff + shentsize * 2,
            7,
            3,
            shstr_off as u64,
            shstr_size as u64,
        );
    }

    buf
}

/// Tiny single-kernel gfx1201 fixture (scalar add style metadata).
pub fn fixture_tiny_add_gfx1201() -> Vec<u8> {
    let meta = amdhsa_metadata(
        (1, 2),
        "amdgcn-amd-amdhsa--gfx1201",
        vec![kernel(
            "tiny_add",
            "tiny_add.kd",
            16,
            8,
            0,
            0,
            vec![
                arg(
                    Some("a"),
                    Some("float*"),
                    0,
                    8,
                    "global_buffer",
                    Some("global"),
                ),
                arg(
                    Some("b"),
                    Some("float*"),
                    8,
                    8,
                    "global_buffer",
                    Some("global"),
                ),
            ],
        )],
    );
    build_code_object_with_metadata(&meta)
}

/// SoftGPU executable fixture: metadata + llvm-mc `tiny_add` `.text`.
pub fn fixture_tiny_add_with_text() -> Vec<u8> {
    let meta = amdhsa_metadata(
        (1, 2),
        "amdgcn-amd-amdhsa--gfx1201",
        vec![kernel(
            "tiny_add",
            "tiny_add.kd",
            16,
            8,
            0,
            0,
            vec![
                arg(
                    Some("a"),
                    Some("float*"),
                    0,
                    8,
                    "global_buffer",
                    Some("global"),
                ),
                arg(
                    Some("b"),
                    Some("float*"),
                    8,
                    8,
                    "global_buffer",
                    Some("global"),
                ),
            ],
        )],
    );
    build_code_object_with_metadata_and_text(&meta, Some(TINY_ADD_TEXT_FIXTURE))
}

/// Multiple kernels + group/private segment sizes.
pub fn fixture_multi_kernel_gfx1201() -> Vec<u8> {
    let meta = amdhsa_metadata(
        (1, 1),
        "amdgcn-amd-amdhsa--gfx1201",
        vec![
            kernel(
                "copy",
                "copy.kd",
                16,
                8,
                0,
                0,
                vec![
                    arg(
                        Some("src"),
                        Some("int*"),
                        0,
                        8,
                        "global_buffer",
                        Some("global"),
                    ),
                    arg(
                        Some("dst"),
                        Some("int*"),
                        8,
                        8,
                        "global_buffer",
                        Some("global"),
                    ),
                ],
            ),
            kernel(
                "reduce",
                "reduce.kd",
                8,
                8,
                256,
                64,
                vec![arg(
                    Some("buf"),
                    Some("int*"),
                    0,
                    8,
                    "global_buffer",
                    Some("global"),
                )],
            ),
        ],
    );
    build_code_object_with_metadata(&meta)
}

/// Unsupported target (fail-closed).
pub fn fixture_unsupported_target() -> Vec<u8> {
    let meta = amdhsa_metadata(
        (1, 0),
        "amdgcn-amd-amdhsa--gfx900",
        vec![kernel("k", "k.kd", 0, 1, 0, 0, vec![])],
    );
    build_code_object_with_metadata(&meta)
}

/// Unsupported metadata version.
pub fn fixture_unsupported_version() -> Vec<u8> {
    let meta = amdhsa_metadata(
        (2, 0),
        "amdgcn-amd-amdhsa--gfx1201",
        vec![kernel("k", "k.kd", 0, 1, 0, 0, vec![])],
    );
    build_code_object_with_metadata(&meta)
}

fn amdhsa_metadata(
    version: (u32, u32),
    target: &str,
    kernels: Vec<BTreeMap<String, Value>>,
) -> Vec<u8> {
    let mut root = BTreeMap::new();
    root.insert(
        "amdhsa.version".into(),
        Value::Array(vec![
            Value::U64(u64::from(version.0)),
            Value::U64(u64::from(version.1)),
        ]),
    );
    root.insert("amdhsa.target".into(), Value::String(target.into()));
    root.insert(
        "amdhsa.kernels".into(),
        Value::Array(kernels.into_iter().map(Value::Map).collect()),
    );
    encode::map_from_btree(&root)
}

fn kernel(
    name: &str,
    symbol: &str,
    kernarg_size: u32,
    kernarg_align: u32,
    group: u32,
    private: u32,
    args: Vec<BTreeMap<String, Value>>,
) -> BTreeMap<String, Value> {
    let mut m = BTreeMap::new();
    m.insert(".name".into(), Value::String(name.into()));
    m.insert(".symbol".into(), Value::String(symbol.into()));
    m.insert(
        ".kernarg_segment_size".into(),
        Value::U64(u64::from(kernarg_size)),
    );
    m.insert(
        ".kernarg_segment_align".into(),
        Value::U64(u64::from(kernarg_align)),
    );
    m.insert(
        ".group_segment_fixed_size".into(),
        Value::U64(u64::from(group)),
    );
    m.insert(
        ".private_segment_fixed_size".into(),
        Value::U64(u64::from(private)),
    );
    m.insert(
        ".args".into(),
        Value::Array(args.into_iter().map(Value::Map).collect()),
    );
    m
}

fn arg(
    name: Option<&str>,
    type_name: Option<&str>,
    offset: u32,
    size: u32,
    value_kind: &str,
    address_space: Option<&str>,
) -> BTreeMap<String, Value> {
    let mut m = BTreeMap::new();
    if let Some(n) = name {
        m.insert(".name".into(), Value::String(n.into()));
    }
    if let Some(t) = type_name {
        m.insert(".type_name".into(), Value::String(t.into()));
    }
    m.insert(".offset".into(), Value::U64(u64::from(offset)));
    m.insert(".size".into(), Value::U64(u64::from(size)));
    m.insert(".value_kind".into(), Value::String(value_kind.into()));
    if let Some(a) = address_space {
        m.insert(".address_space".into(), Value::String(a.into()));
    }
    m
}

fn encode_note(name: &str, ntype: u32, desc: &[u8]) -> Vec<u8> {
    let mut name_bytes = name.as_bytes().to_vec();
    name_bytes.push(0);
    let namesz = name_bytes.len();
    let descsz = desc.len();
    let mut out = Vec::new();
    out.extend_from_slice(&(namesz as u32).to_le_bytes());
    out.extend_from_slice(&(descsz as u32).to_le_bytes());
    out.extend_from_slice(&ntype.to_le_bytes());
    out.extend_from_slice(&name_bytes);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out.extend_from_slice(desc);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out
}

fn write_section(buf: &mut [u8], at: usize, name_off: u32, sh_type: u32, off: u64, size: u64) {
    write_u32(buf, at, name_off);
    write_u32(buf, at + 4, sh_type);
    write_u64(buf, at + 24, off);
    write_u64(buf, at + 32, size);
}

fn write_u16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}
fn write_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}
fn write_u64(buf: &mut [u8], off: usize, v: u64) {
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}
fn align8(n: usize) -> usize {
    (n + 7) & !7
}
