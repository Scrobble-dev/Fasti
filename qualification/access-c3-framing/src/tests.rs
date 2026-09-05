use super::*;
use fasti_store::archive::{validate_archive, ArchiveError, ArchiveLimits, ArchiveWriter};
use std::{
    cell::{Cell, RefCell},
    io::Cursor,
    rc::Rc,
};

const BACKUP: &[u8] = b"opaque-fixture-backup-v1";
const RECORD: &[u8] = b"opaque-fixture-record-v1";
#[test]
fn linked_native_identity_is_the_pinned_nonminimal_build() {
    use libsodium_rs::version;
    assert_eq!(version::version_string(), "1.0.22");
    assert_eq!(version::library_version_major(), 26);
    assert_eq!(version::library_version_minor(), 4);
    assert!(!version::library_minimal());
}
fn key() -> SecretKey {
    SecretKey::generate().unwrap()
}
fn limits(p: u64) -> Limits {
    Limits::new(Purpose::Backup, p).unwrap()
}
fn encrypt(bytes: &[u8], key: &SecretKey, envelope: &[u8], limits: Limits) -> Vec<u8> {
    let mut writer = FrameWriter::new(Vec::new(), key, envelope, limits).unwrap();
    writer.write_all(bytes).unwrap();
    writer.finalise().unwrap();
    writer.into_inner().unwrap()
}
fn decrypt(bytes: &[u8], key: &SecretKey, envelope: &[u8], limits: Limits) -> io::Result<Vec<u8>> {
    let mut reader = FrameReader::new(Cursor::new(bytes), key, envelope, limits)?;
    let mut out = Vec::new();
    reader.read_to_end(&mut out)?;
    reader.finish()?;
    Ok(out)
}
fn prefix_len(bytes: &[u8]) -> usize {
    36 + u32::from_be_bytes(bytes[8..12].try_into().unwrap()) as usize
}
fn frames(bytes: &[u8]) -> Vec<std::ops::Range<usize>> {
    let mut at = prefix_len(bytes);
    let mut result = Vec::new();
    while at < bytes.len() {
        let n = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        result.push(at..at + 4 + n);
        at += 4 + n;
    }
    result
}

#[test]
fn backup_plaintext_alignment_matrix_and_exact_wire_equation() {
    let key = key();
    for size in [0, 1, 65535, 65536, 65537, 131072] {
        let data = vec![0x5a; size];
        let bytes = encrypt(&data, &key, BACKUP, limits(size as u64));
        assert_eq!(
            decrypt(&bytes, &key, BACKUP, limits(size as u64)).unwrap(),
            data
        );
        let f = frames(&bytes).len();
        assert_eq!(bytes.len(), 36 + BACKUP.len() + size + 21 * f);
        println!("backup plain={size} frames={f} ciphertext={}", bytes.len());
    }
}

#[test]
fn record_size_single_final_and_provider_owner_limit() {
    let key = key();
    let policy = Limits::new(Purpose::Record, 65536).unwrap();
    for size in [1, 65536] {
        let data = vec![0x5a; size];
        let bytes = encrypt(&data, &key, RECORD, policy);
        assert_eq!(frames(&bytes).len(), 1);
        assert_eq!(decrypt(&bytes, &key, RECORD, policy).unwrap(), data);
    }
    let mut empty = FrameWriter::new(Vec::new(), &key, RECORD, policy).unwrap();
    assert!(empty.finalise().is_err());
    let mut oversized = FrameWriter::new(Vec::new(), &key, RECORD, policy).unwrap();
    assert!(oversized.write_all(&vec![0; 65537]).is_err());
    let provider = Limits::new(Purpose::Record, 4096).unwrap();
    let data = vec![0; 4096];
    assert_eq!(
        decrypt(
            &encrypt(&data, &key, RECORD, provider),
            &key,
            RECORD,
            provider
        )
        .unwrap(),
        data
    );
    let mut too_large = FrameWriter::new(Vec::new(), &key, RECORD, provider).unwrap();
    assert!(too_large.write_all(&vec![0; 4097]).is_err());
    let mut record_flush = FrameWriter::new(Vec::new(), &key, RECORD, policy).unwrap();
    record_flush.write_all(b"x").unwrap();
    assert!(record_flush.flush().is_err());
    assert!(record_flush.finalise().is_err());
}

#[test]
fn partial_and_repeated_flush_reserve_final_at_exact_frame_cap() {
    let key = key();
    let policy = limits(65536);
    assert_eq!(policy.fmax, 3);
    let mut writer = FrameWriter::new(Vec::new(), &key, BACKUP, policy).unwrap();
    writer.write_all(b"a").unwrap();
    writer.flush().unwrap();
    assert_eq!(writer.budget.frames, 1);
    assert!(writer.state.is_some());
    assert!(!writer.complete);
    writer.flush().unwrap();
    assert_eq!(writer.budget.frames, 1);
    writer.write_all(b"b").unwrap();
    writer.flush().unwrap();
    assert_eq!(writer.budget.frames, 2);
    writer.finalise().unwrap();
    assert_eq!(writer.budget.frames, 3);
    assert_eq!(
        decrypt(&writer.into_inner().unwrap(), &key, BACKUP, policy).unwrap(),
        b"ab"
    );
    let mut excess = FrameWriter::new(Vec::new(), &key, BACKUP, policy).unwrap();
    for _ in 0..2 {
        excess.write_all(b"x").unwrap();
        excess.flush().unwrap();
    }
    excess.write_all(b"x").unwrap();
    assert!(excess.flush().is_err());
    assert_eq!(excess.seals, 2);
    assert!(excess.finalise().is_err());
    assert!(excess.into_inner().is_err());
}

// Non-cryptographic deterministic fixture generator; never used for keys/nonces.
fn fixture(size: usize) -> Vec<u8> {
    let mut state = 0x123456789abcdef0_u64;
    (0..size)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 32) as u8
        })
        .collect()
}
fn archive_limits(compressed: u64) -> ArchiveLimits {
    ArchiveLimits::new(compressed, 2, 1_000_000, 4_000_000).unwrap()
}
fn append_fixture<W: Write>(
    writer: &mut ArchiveWriter<W>,
    data: &[u8],
) -> Result<(), ArchiveError> {
    writer.append("data.bin", data.len() as u64, Cursor::new(data))?;
    writer.append("manifest.json", 2, Cursor::new(b"{}"))
}

#[test]
fn real_archive_finish_flush_and_exact_compressed_limits() {
    let key = key();
    for payload_len in [0, 1, 65535, 65536, 65537, 131072] {
        let data = fixture(payload_len);
        let mut baseline = ArchiveWriter::new(Vec::new(), archive_limits(1_000_000)).unwrap();
        append_fixture(&mut baseline, &data).unwrap();
        let expected = baseline.finish().unwrap();
        let compressed = expected.len() as u64;
        let cap = archive_limits(compressed);
        let sink =
            FrameWriter::new(Vec::new(), &key, BACKUP, limits(cap.max_compressed_bytes)).unwrap();
        let mut archive = ArchiveWriter::new(sink, cap).unwrap();
        append_fixture(&mut archive, &data).unwrap();
        let mut encryption = archive.finish().unwrap();
        assert!(encryption.pending.is_empty());
        assert!(!encryption.complete);
        assert!(encryption.budget.frames > 0);
        assert_eq!(encryption.accepted, compressed);
        encryption.finalise().unwrap();
        let bytes = encryption.into_inner().unwrap();
        let mut reader =
            FrameReader::new(Cursor::new(&bytes), &key, BACKUP, limits(compressed)).unwrap();
        let summary = validate_archive(&mut reader, cap).unwrap();
        assert_eq!(summary.entries, 2);
        reader.finish().unwrap();
        assert_eq!(
            decrypt(&bytes, &key, BACKUP, limits(compressed)).unwrap(),
            expected
        );
        assert!(decrypt(&bytes, &key, BACKUP, limits(compressed - 1)).is_err());
        let sink = FrameWriter::new(Vec::new(), &key, BACKUP, limits(compressed - 1)).unwrap();
        let mut low = ArchiveWriter::new(sink, archive_limits(compressed - 1)).unwrap();
        let low_result = append_fixture(&mut low, &data).and_then(|_| low.finish().map(|_| ()));
        assert!(low_result.is_err());
        println!(
            "real archive payload={payload_len} exact_compressed={compressed} frames={} fmax={}",
            frames(&bytes).len(),
            limits(compressed).fmax
        );
    }
}

#[test]
fn inner_archive_success_does_not_hide_physical_trailing_ciphertext() {
    let key = key();
    let cap = archive_limits(1_000_000);
    let mut archive = ArchiveWriter::new(Vec::new(), cap).unwrap();
    append_fixture(&mut archive, b"fixture").unwrap();
    let plain = archive.finish().unwrap();
    let mut bytes = encrypt(&plain, &key, BACKUP, limits(plain.len() as u64));
    let size = bytes.len();
    bytes.push(0x7f);
    let mut bound = limits(plain.len() as u64);
    bound.cmax = size as u64;
    let mut reader = FrameReader::new(Cursor::new(&bytes), &key, BACKUP, bound).unwrap();
    validate_archive(&mut reader, cap).unwrap();
    assert!(reader.finish().is_err());
}

#[test]
fn invalid_inner_archive_is_not_publishable_even_when_encryption_is_valid() {
    let key = key();
    let bytes = encrypt(b"not an archive", &key, BACKUP, limits(64));
    let mut reader = FrameReader::new(Cursor::new(&bytes), &key, BACKUP, limits(64)).unwrap();
    assert!(validate_archive(&mut reader, archive_limits(64)).is_err());
}

#[test]
fn wrong_key_envelope_header_ciphertext_aad_and_purpose_are_rejected() {
    let key = key();
    let bytes = encrypt(b"fixture", &key, BACKUP, limits(64));
    assert!(decrypt(&bytes, &self::key(), BACKUP, limits(64)).is_err());
    assert!(decrypt(
        &bytes,
        &key,
        RECORD,
        Limits::new(Purpose::Record, 64).unwrap()
    )
    .is_err());
    for position in [12, prefix_len(&bytes) - 1, prefix_len(&bytes) + 4] {
        let mut changed = bytes.clone();
        changed[position] ^= 1;
        assert!(decrypt(&changed, &key, BACKUP, limits(64)).is_err());
    }
    let mut changed = bytes.clone();
    changed[12] ^= 1;
    let mut changed_envelope = BACKUP.to_vec();
    changed_envelope[0] ^= 1;
    assert!(
        decrypt(&changed, &key, &changed_envelope, limits(64)).is_err(),
        "exact prefix AAD must fail even when fixture envelope is accepted"
    );
    let mut changed_len = bytes.clone();
    let at = prefix_len(&bytes);
    let len = u32::from_be_bytes(changed_len[at..at + 4].try_into().unwrap());
    changed_len[at..at + 4].copy_from_slice(&(len + 1).to_be_bytes());
    changed_len.push(0);
    assert!(decrypt(&changed_len, &key, BACKUP, limits(64)).is_err());
}

#[test]
fn missing_truncated_final_trailing_reordered_duplicate_and_cross_stream_frames() {
    let key = key();
    let data = fixture(65537);
    let policy = limits(131072);
    let bytes = encrypt(&data, &key, BACKUP, policy);
    let spans = frames(&bytes);
    assert_eq!(spans.len(), 2);
    assert!(decrypt(&bytes[..spans[1].start], &key, BACKUP, policy).is_err());
    for cut in [1, 16, spans[1].len() - 1] {
        assert!(decrypt(&bytes[..bytes.len() - cut], &key, BACKUP, policy).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decrypt(&trailing, &key, BACKUP, policy).is_err());
    let prefix = &bytes[..prefix_len(&bytes)];
    let reordered = [prefix, &bytes[spans[1].clone()], &bytes[spans[0].clone()]].concat();
    assert!(decrypt(&reordered, &key, BACKUP, policy).is_err());
    let duplicate = [
        prefix,
        &bytes[spans[0].clone()],
        &bytes[spans[0].clone()],
        &bytes[spans[1].clone()],
    ]
    .concat();
    assert!(decrypt(&duplicate, &key, BACKUP, policy).is_err());
    let other = encrypt(&data, &key, BACKUP, policy);
    let other_spans = frames(&other);
    let mixed = [
        prefix,
        &bytes[spans[0].clone()],
        &other[other_spans[1].clone()],
    ]
    .concat();
    assert!(decrypt(&mixed, &key, BACKUP, policy).is_err());
}

// Public native API fixture only. This permits valid authentication of tags the
// safe wrapper cannot produce. It is not an alternate crypto implementation.
fn native_frame(key: &SecretKey, envelope: &[u8], tag: u8, plain: &[u8]) -> Vec<u8> {
    use libsodium_sys::*;
    let mut state = std::mem::MaybeUninit::<crypto_secretstream_xchacha20poly1305_state>::uninit();
    let mut header = [0; 24];
    assert_eq!(
        unsafe {
            crypto_secretstream_xchacha20poly1305_init_push(
                state.as_mut_ptr(),
                header.as_mut_ptr(),
                key.0.as_bytes().as_ptr(),
            )
        },
        0
    );
    let mut state = unsafe { state.assume_init() };
    let mut prefix = Vec::new();
    prefix.extend_from_slice(MAGIC);
    prefix.extend_from_slice(&(envelope.len() as u32).to_be_bytes());
    prefix.extend_from_slice(envelope);
    prefix.extend_from_slice(&header);
    let len = (plain.len() + OVERHEAD) as u32;
    let mut aad = prefix.clone();
    aad.extend_from_slice(&len.to_be_bytes());
    let mut cipher = vec![0; len as usize];
    let mut actual = 0;
    let result = unsafe {
        crypto_secretstream_xchacha20poly1305_push(
            &mut state,
            cipher.as_mut_ptr(),
            &mut actual,
            plain.as_ptr(),
            plain.len() as u64,
            aad.as_ptr(),
            aad.len() as u64,
            tag,
        )
    };
    unsafe {
        sodium_memzero(
            (&mut state as *mut crypto_secretstream_xchacha20poly1305_state).cast(),
            std::mem::size_of_val(&state),
        )
    };
    assert_eq!(result, 0);
    assert_eq!(actual, len as u64);
    prefix.extend_from_slice(&len.to_be_bytes());
    prefix.extend_from_slice(&cipher);
    prefix
}

#[test]
fn unsupported_defined_tags_empty_ordinary_and_record_tag_mismatch_fail() {
    let key = key();
    for tag in [1, 2] {
        assert!(decrypt(
            &native_frame(&key, BACKUP, tag, b"x"),
            &key,
            BACKUP,
            limits(64)
        )
        .is_err());
    }
    assert!(decrypt(
        &native_frame(&key, BACKUP, 0, b""),
        &key,
        BACKUP,
        limits(64)
    )
    .is_err());
    assert!(decrypt(
        &native_frame(&key, RECORD, 0, b"x"),
        &key,
        RECORD,
        Limits::new(Purpose::Record, 64).unwrap()
    )
    .is_err());
    assert!(decrypt(
        &native_frame(&key, RECORD, 3, b""),
        &key,
        RECORD,
        Limits::new(Purpose::Record, 64).unwrap()
    )
    .is_err());
}

#[test]
fn unknown_authenticated_native_tag_must_return_error_not_panic() {
    let key = key();
    let bytes = native_frame(&key, BACKUP, 0x7f, b"x");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        decrypt(&bytes, &key, BACKUP, limits(64))
    }));
    assert!(
        result.is_ok(),
        "binding panics on an authenticated unknown native tag; not qualified"
    );
    assert!(result.unwrap().is_err());
}

struct CountRead {
    data: Cursor<Vec<u8>>,
    reads: Rc<Cell<usize>>,
}
impl Read for CountRead {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = self.data.read(out)?;
        self.reads.set(self.reads.get() + n);
        Ok(n)
    }
}

#[test]
fn malformed_envelope_frame_lengths_and_overflows_fail_before_body_reads() {
    let key = key();
    for len in [0_u32, 16385, u32::MAX] {
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes.extend_from_slice(&[0; 100]);
        let reads = Rc::new(Cell::new(0));
        assert!(FrameReader::new(
            CountRead {
                data: Cursor::new(bytes),
                reads: reads.clone()
            },
            &key,
            BACKUP,
            limits(64)
        )
        .is_err());
        assert_eq!(reads.get(), 12);
    }
    let valid = encrypt(b"x", &key, BACKUP, limits(64));
    let prefix = prefix_len(&valid);
    for len in [0_u32, 16, 65554, u32::MAX] {
        let mut bytes = valid[..prefix].to_vec();
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes.extend_from_slice(&[0; 100]);
        let reads = Rc::new(Cell::new(0));
        let mut reader = FrameReader::new(
            CountRead {
                data: Cursor::new(bytes),
                reads: reads.clone(),
            },
            &key,
            BACKUP,
            limits(64),
        )
        .unwrap();
        assert!(reader.read(&mut [0; 1]).is_err());
        assert_eq!(reads.get(), prefix + 4);
    }
    assert!(Limits::new(Purpose::Backup, u64::MAX).is_err());
    let policy = limits(64);
    for budget in [
        Budget {
            plain: u64::MAX,
            frames: 0,
            cipher: 0,
        },
        Budget {
            plain: 0,
            frames: u64::MAX,
            cipher: 0,
        },
        Budget {
            plain: 0,
            frames: 0,
            cipher: u64::MAX,
        },
    ] {
        assert!(budget.next(policy, 1, true).is_err());
    }
}

#[test]
fn decoder_exact_frame_plaintext_and_ciphertext_caps() {
    let key = key();
    let bytes = encrypt(b"x", &key, BACKUP, limits(64));
    let mut cap = limits(64);
    cap.cmax = bytes.len() as u64;
    assert_eq!(decrypt(&bytes, &key, BACKUP, cap).unwrap(), b"x");
    cap.cmax -= 1;
    assert!(decrypt(&bytes, &key, BACKUP, cap).is_err());
    let mut cap = limits(64);
    cap.fmax = 0;
    assert!(decrypt(&bytes, &key, BACKUP, cap).is_err());
    let mut writer = FrameWriter::new(Vec::new(), &key, BACKUP, limits(65536)).unwrap();
    writer.write_all(b"a").unwrap();
    writer.flush().unwrap();
    writer.write_all(b"b").unwrap();
    writer.flush().unwrap();
    writer.finalise().unwrap();
    let bytes = writer.into_inner().unwrap();
    let mut cap = limits(65536);
    cap.fmax = 3;
    assert_eq!(decrypt(&bytes, &key, BACKUP, cap).unwrap(), b"ab");
    cap.fmax = 2;
    assert!(decrypt(&bytes, &key, BACKUP, cap).is_err());
    assert!(decrypt(&bytes, &key, BACKUP, limits(1)).is_err());
}

struct Choppy<T> {
    inner: T,
    turn: usize,
}
impl<T: Read> Read for Choppy<T> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        self.turn += 1;
        if self.turn % 5 == 1 {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let n = out.len().min(3);
        self.inner.read(&mut out[..n])
    }
}
impl<T: Write> Write for Choppy<T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.turn += 1;
        if self.turn % 5 == 1 {
            return Err(io::ErrorKind::Interrupted.into());
        }
        self.inner.write(&bytes[..bytes.len().min(3)])
    }
    fn flush(&mut self) -> io::Result<()> {
        self.turn += 1;
        if self.turn % 5 == 1 {
            Err(io::ErrorKind::Interrupted.into())
        } else {
            self.inner.flush()
        }
    }
}
#[test]
fn short_and_interrupted_io_preserves_identity() {
    let key = key();
    let data = fixture(65537);
    let policy = limits(data.len() as u64);
    let mut writer = FrameWriter::new(
        Choppy {
            inner: Vec::new(),
            turn: 0,
        },
        &key,
        BACKUP,
        policy,
    )
    .unwrap();
    writer.write_all(&data).unwrap();
    writer.flush().unwrap();
    writer.finalise().unwrap();
    let bytes = writer.into_inner().unwrap().inner;
    let mut reader = FrameReader::new(
        Choppy {
            inner: Cursor::new(bytes),
            turn: 0,
        },
        &key,
        BACKUP,
        policy,
    )
    .unwrap();
    let mut out = Vec::new();
    reader.read_to_end(&mut out).unwrap();
    reader.finish().unwrap();
    assert_eq!(out, data);
}

#[derive(Clone)]
struct FaultSink {
    bytes: Rc<RefCell<Vec<u8>>>,
    fail_after: usize,
    flush_error: Rc<Cell<bool>>,
}
impl Write for FaultSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let n = bytes
            .len()
            .min(self.fail_after.saturating_sub(self.bytes.borrow().len()));
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected sink error",
            ));
        }
        self.bytes.borrow_mut().extend_from_slice(&bytes[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.flush_error.get() {
            Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "injected flush error",
            ))
        } else {
            Ok(())
        }
    }
}
#[test]
fn partial_sink_error_poison_preserves_error_and_never_reseals() {
    let key = key();
    let bytes = Rc::new(RefCell::new(Vec::new()));
    let sink = FaultSink {
        bytes: bytes.clone(),
        fail_after: 36 + BACKUP.len() + 7,
        flush_error: Rc::new(Cell::new(false)),
    };
    let mut writer = FrameWriter::new(sink, &key, BACKUP, limits(64)).unwrap();
    writer.write_all(b"fixture").unwrap();
    assert_eq!(
        writer.flush().unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    let size = bytes.borrow().len();
    assert_eq!(writer.seals, 1);
    assert!(writer.state.is_none());
    assert_eq!(writer.pending.capacity(), 0);
    assert!(writer.write(b"again").is_err());
    assert!(writer.flush().is_err());
    assert!(writer.finalise().is_err());
    assert_eq!(writer.seals, 1);
    assert_eq!(bytes.borrow().len(), size);
    assert!(writer.into_inner().is_err());
    assert!(decrypt(&bytes.borrow(), &key, BACKUP, limits(64)).is_err());
}
#[test]
fn flush_failure_poison_and_drop_never_finalizes() {
    let key = key();
    let bytes = Rc::new(RefCell::new(Vec::new()));
    let failure = Rc::new(Cell::new(false));
    let sink = FaultSink {
        bytes: bytes.clone(),
        fail_after: usize::MAX,
        flush_error: failure.clone(),
    };
    let mut writer = FrameWriter::new(sink, &key, BACKUP, limits(64)).unwrap();
    writer.write_all(b"fixture").unwrap();
    writer.flush().unwrap();
    let before_drop = bytes.borrow().len();
    drop(writer);
    assert_eq!(bytes.borrow().len(), before_drop);
    assert!(decrypt(&bytes.borrow(), &key, BACKUP, limits(64)).is_err());
    let sink = FaultSink {
        bytes: Rc::new(RefCell::new(Vec::new())),
        fail_after: usize::MAX,
        flush_error: failure.clone(),
    };
    let mut writer = FrameWriter::new(sink, &key, BACKUP, limits(64)).unwrap();
    writer.write_all(b"fixture").unwrap();
    failure.set(true);
    assert_eq!(
        writer.finalise().unwrap_err().kind(),
        io::ErrorKind::ConnectionReset
    );
    assert!(writer.poisoned);
    assert!(writer.state.is_none());
    assert_eq!(writer.pending.capacity(), 0);
    assert!(writer.into_inner().is_err());
}

struct FaultSource {
    inner: Cursor<Vec<u8>>,
    at: usize,
}
impl Read for FaultSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let left = self.at.saturating_sub(self.inner.position() as usize);
        if left == 0 {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "injected source error",
            ));
        }
        let n = out.len().min(left);
        self.inner.read(&mut out[..n])
    }
}
#[test]
fn source_error_survives_exact_read_and_reader_is_poisoned() {
    let key = key();
    let bytes = encrypt(b"fixture", &key, BACKUP, limits(64));
    let at = prefix_len(&bytes) + 5;
    let mut reader = FrameReader::new(
        FaultSource {
            inner: Cursor::new(bytes),
            at,
        },
        &key,
        BACKUP,
        limits(64),
    )
    .unwrap();
    assert_eq!(
        reader.read(&mut [0; 4]).unwrap_err().kind(),
        io::ErrorKind::ConnectionReset
    );
    assert!(reader.poisoned);
    assert!(reader.state.is_none());
    assert!(reader.plain.is_none());
    assert!(reader.finish().is_err());
}

#[test]
fn finalization_releases_state_and_refuses_later_advancement() {
    fn assert_send<T: Send>() {}
    assert_send::<FrameWriter<Vec<u8>>>();
    assert_send::<FrameReader<Cursor<Vec<u8>>>>();
    let key = key();
    for operation in 0..3 {
        let mut writer = FrameWriter::new(Vec::new(), &key, BACKUP, limits(64)).unwrap();
        writer.write_all(b"fixture").unwrap();
        writer.finalise().unwrap();
        assert!(writer.state.is_none());
        assert_eq!(writer.pending.capacity(), 0);
        let len = writer.sink.len();
        let seals = writer.seals;
        match operation {
            0 => assert!(writer.write(b"later").is_err()),
            1 => assert!(writer.flush().is_err()),
            _ => assert!(writer.finalise().is_err()),
        }
        assert_eq!(writer.sink.len(), len);
        assert_eq!(writer.seals, seals);
    }
    let bytes = encrypt(b"fixture", &key, BACKUP, limits(64));
    let mut reader = FrameReader::new(Cursor::new(bytes), &key, BACKUP, limits(64)).unwrap();
    let mut first = [0];
    reader.read_exact(&mut first).unwrap();
    assert!(reader.final_seen);
    assert!(reader.state.is_none());
    let position = reader.source.position();
    assert!(reader.frame().is_err());
    assert_eq!(reader.source.position(), position);
    reader.finish().unwrap();
    assert!(reader.plain.is_none());
    assert_eq!(reader.read(&mut first).unwrap(), 0);
    assert_eq!(reader.source.position(), position);
}

#[test]
fn rejected_readers_release_owners_before_caller_drops_them() {
    let key = key();
    let cases = [
        (native_frame(&key, BACKUP, 0x7f, b"fixture"), limits(64)),
        (
            encrypt(b"fixture", &self::key(), BACKUP, limits(64)),
            limits(64),
        ),
        (encrypt(b"fixture", &key, BACKUP, limits(64)), limits(1)),
    ];
    for (bytes, policy) in cases {
        let mut reader = FrameReader::new(Cursor::new(bytes), &key, BACKUP, policy).unwrap();
        assert!(reader.state.is_some());
        assert!(reader.read(&mut [0; 16]).is_err());
        assert!(reader.poisoned);
        assert!(reader.state.is_none());
        assert!(reader.plain.is_none());
        let position = reader.source.position();
        assert!(reader.read(&mut [0; 16]).is_err());
        assert!(reader.finish().is_err());
        assert_eq!(reader.source.position(), position);
    }
}

#[test]
fn extra_plaintext_after_valid_archive_is_not_publishable() {
    let key = key();
    let cap = archive_limits(1_000_000);
    let mut archive = ArchiveWriter::new(Vec::new(), cap).unwrap();
    append_fixture(&mut archive, b"fixture").unwrap();
    let valid = archive.finish().unwrap();
    for extra in [b"unexpected plaintext".as_slice(), valid.as_slice()] {
        let mut plain = valid.clone();
        plain.extend_from_slice(extra);
        let bytes = encrypt(&plain, &key, BACKUP, limits(plain.len() as u64));
        let mut reader =
            FrameReader::new(Cursor::new(bytes), &key, BACKUP, limits(plain.len() as u64)).unwrap();
        assert!(validate_archive(&mut reader, cap).is_err());
    }
}
