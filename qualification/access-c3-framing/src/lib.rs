//! Isolated alternative binding qualification; not a production format or adapter.

use libsodium_rs::crypto_secretstream::xchacha20poly1305 as stream;
use zeroize::{Zeroize, Zeroizing};

/// Keeps the provider's unredacted Debug/Clone key private to this probe boundary.
/// ```compile_fail
/// let key = fasti_c3_frame_probe2::SecretKey::generate().unwrap();
/// let _ = format!("{key:?}");
/// ```
/// ```compile_fail
/// let key = fasti_c3_frame_probe2::SecretKey::generate().unwrap();
/// let _ = key.clone();
/// ```
pub struct SecretKey(stream::Key);
impl SecretKey {
    pub fn generate() -> io::Result<Self> {
        libsodium_rs::ensure_init().map_err(crypt)?;
        Ok(Self(stream::Key::generate()))
    }
}
use std::io::{self, Read, Write};

const MAGIC: &[u8; 8] = b"FASTIC3\0";
const CHUNK: usize = 65536;
const OVERHEAD: usize = 17;
const MAX_FRAME: usize = CHUNK + OVERHEAD;
const MAX_ENVELOPE: usize = 16384;

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn crypt(error: libsodium_rs::SodiumError) -> io::Error {
    io::Error::other(error)
}
fn flush_retry<W: Write>(sink: &mut W) -> io::Result<()> {
    loop {
        match sink.flush() {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
}
fn physical_eof<R: Read>(source: &mut R) -> io::Result<()> {
    let mut byte = [0];
    loop {
        match source.read(&mut byte) {
            Ok(0) => return Ok(()),
            Ok(_) => return Err(invalid("trailing ciphertext")),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Purpose {
    Backup,
    Record,
}
#[derive(Clone, Copy)]
pub struct Limits {
    purpose: Purpose,
    pmax: u64,
    fmax: u64,
    cmax: u64,
}
impl Limits {
    pub fn new(purpose: Purpose, pmax: u64) -> io::Result<Self> {
        if purpose == Purpose::Record && !(1..=CHUNK as u64).contains(&pmax) {
            return Err(invalid("record owner limit"));
        }
        let fmax = match purpose {
            Purpose::Record => 1,
            Purpose::Backup => (pmax / CHUNK as u64)
                .checked_add(u64::from(!pmax.is_multiple_of(CHUNK as u64)))
                .and_then(|v| v.checked_add(2))
                .ok_or(invalid("frame admission overflow"))?,
        };
        let cmax = 36_u64
            .checked_add(MAX_ENVELOPE as u64)
            .and_then(|v| v.checked_add(pmax))
            .and_then(|v| fmax.checked_mul(21).and_then(|over| v.checked_add(over)))
            .ok_or(invalid("ciphertext admission overflow"))?;
        Ok(Self {
            purpose,
            pmax,
            fmax,
            cmax,
        })
    }
}
#[derive(Clone, Copy, Default)]
struct Budget {
    plain: u64,
    frames: u64,
    cipher: u64,
}
impl Budget {
    fn next(self, limits: Limits, plain: usize, final_frame: bool) -> io::Result<Self> {
        let next = Self {
            plain: self
                .plain
                .checked_add(plain as u64)
                .ok_or(invalid("plaintext counter overflow"))?,
            frames: self
                .frames
                .checked_add(1)
                .ok_or(invalid("frame counter overflow"))?,
            cipher: self
                .cipher
                .checked_add(plain as u64)
                .and_then(|v| v.checked_add(21))
                .ok_or(invalid("ciphertext counter overflow"))?,
        };
        if next.plain > limits.pmax
            || next.cipher > limits.cmax
            || next.frames > limits.fmax
            || (!final_frame && next.frames >= limits.fmax)
        {
            return Err(invalid("admission exhausted"));
        }
        Ok(next)
    }
}

pub struct FrameWriter<W: Write> {
    sink: W,
    state: Option<stream::PushState>,
    prefix: Vec<u8>,
    pending: Zeroizing<Vec<u8>>,
    limits: Limits,
    budget: Budget,
    accepted: u64,
    poisoned: bool,
    complete: bool,
    seals: u64,
}
impl<W: Write> FrameWriter<W> {
    pub fn new(mut sink: W, key: &SecretKey, envelope: &[u8], limits: Limits) -> io::Result<Self> {
        if !(1..=MAX_ENVELOPE).contains(&envelope.len()) {
            return Err(invalid("envelope length"));
        }
        let (state, header) = stream::PushState::init_push(&key.0).map_err(crypt)?;
        let mut prefix = Vec::with_capacity(36 + envelope.len());
        prefix.extend_from_slice(MAGIC);
        prefix.extend_from_slice(&(envelope.len() as u32).to_be_bytes());
        prefix.extend_from_slice(envelope);
        prefix.extend_from_slice(&header);
        sink.write_all(&prefix)?;
        let budget = Budget {
            cipher: prefix.len() as u64,
            ..Budget::default()
        };
        Ok(Self {
            sink,
            state: Some(state),
            prefix,
            pending: Zeroizing::new(Vec::with_capacity(CHUNK)),
            limits,
            budget,
            accepted: 0,
            poisoned: false,
            complete: false,
            seals: 0,
        })
    }
    fn poison(&mut self) {
        self.poisoned = true;
        drop(self.state.take());
        self.pending = Zeroizing::new(Vec::new());
    }
    fn usable(&self) -> io::Result<()> {
        if self.poisoned || self.complete {
            Err(invalid("writer is closed or poisoned"))
        } else {
            Ok(())
        }
    }
    fn emit(&mut self, final_frame: bool) -> io::Result<()> {
        self.usable()?;
        if !final_frame && (self.pending.is_empty() || self.limits.purpose == Purpose::Record) {
            return Err(invalid("ordinary frame violates purpose"));
        }
        if self.limits.purpose == Purpose::Record && self.pending.is_empty() {
            return Err(invalid("empty record"));
        }
        let next = self
            .budget
            .next(self.limits, self.pending.len(), final_frame)?;
        let len = (self.pending.len() + OVERHEAD) as u32;
        let mut aad = self.prefix.clone();
        aad.extend_from_slice(&len.to_be_bytes());
        let cipher = if final_frame {
            self.state
                .take()
                .ok_or(invalid("missing encryption state"))?
                .push(&self.pending, Some(&aad), stream::TAG_FINAL)
                .map_err(crypt)?
        } else {
            self.state
                .as_mut()
                .ok_or(invalid("missing encryption state"))?
                .push(&self.pending, Some(&aad), stream::TAG_MESSAGE)
                .map_err(crypt)?
        };
        self.pending.zeroize();
        self.seals += 1;
        if cipher.len() != len as usize {
            return Err(invalid("provider ciphertext length"));
        }
        self.sink.write_all(&len.to_be_bytes())?;
        self.sink.write_all(&cipher)?;
        self.budget = next;
        self.pending.clear();
        Ok(())
    }
    pub fn finalise(&mut self) -> io::Result<()> {
        let result = (|| {
            self.usable()?;
            self.emit(true)?;
            flush_retry(&mut self.sink)?;
            self.pending = Zeroizing::new(Vec::new());
            self.complete = true;
            Ok(())
        })();
        if result.is_err() {
            self.poison();
        }
        result
    }
    pub fn into_inner(self) -> io::Result<W> {
        if self.complete && !self.poisoned {
            Ok(self.sink)
        } else {
            Err(invalid("not publishable"))
        }
    }
}
impl<W: Write> Write for FrameWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = (|| {
            self.usable()?;
            let next = self
                .accepted
                .checked_add(bytes.len() as u64)
                .filter(|n| *n <= self.limits.pmax)
                .ok_or(invalid("plaintext limit"))?;
            let mut remaining = bytes;
            while !remaining.is_empty() {
                let n = remaining.len().min(CHUNK - self.pending.len());
                self.pending.extend_from_slice(&remaining[..n]);
                remaining = &remaining[n..];
                if self.pending.len() == CHUNK && self.limits.purpose == Purpose::Backup {
                    self.emit(false)?;
                }
            }
            self.accepted = next;
            Ok(bytes.len())
        })();
        if result.is_err() {
            self.poison();
        }
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        let result = (|| {
            self.usable()?;
            if !self.pending.is_empty() {
                self.emit(false)?;
            }
            flush_retry(&mut self.sink)
        })();
        if result.is_err() {
            self.poison();
        }
        result
    }
}

pub struct FrameReader<R: Read> {
    source: R,
    state: Option<stream::PullState>,
    prefix: Vec<u8>,
    plain: Option<Zeroizing<Vec<u8>>>,
    cipher: Vec<u8>,
    position: usize,
    available: usize,
    limits: Limits,
    budget: Budget,
    final_seen: bool,
    poisoned: bool,
}
impl<R: Read> FrameReader<R> {
    pub fn new(
        mut source: R,
        key: &SecretKey,
        expected_envelope: &[u8],
        limits: Limits,
    ) -> io::Result<Self> {
        let mut fixed = [0; 12];
        source.read_exact(&mut fixed)?;
        if &fixed[..8] != MAGIC {
            return Err(invalid("magic"));
        }
        let e = u32::from_be_bytes(fixed[8..].try_into().unwrap()) as usize;
        if !(1..=MAX_ENVELOPE).contains(&e) {
            return Err(invalid("envelope length"));
        }
        let mut prefix = fixed.to_vec();
        prefix.resize(12 + e + 24, 0);
        source.read_exact(&mut prefix[12..])?;
        if &prefix[12..12 + e] != expected_envelope {
            return Err(invalid("opaque envelope/purpose mismatch"));
        }
        let header = prefix[12 + e..].try_into().unwrap();
        let state = stream::PullState::init_pull(&header, &key.0).map_err(crypt)?;
        let budget = Budget {
            cipher: prefix.len() as u64,
            ..Budget::default()
        };
        Ok(Self {
            source,
            state: Some(state),
            prefix,
            plain: None,
            cipher: vec![0; MAX_FRAME],
            position: 0,
            available: 0,
            limits,
            budget,
            final_seen: false,
            poisoned: false,
        })
    }
    fn poison(&mut self) {
        self.poisoned = true;
        drop(self.state.take());
        drop(self.plain.take());
        self.position = 0;
        self.available = 0;
    }
    fn frame(&mut self) -> io::Result<()> {
        if self.poisoned || self.final_seen {
            return Err(invalid("reader is closed or poisoned"));
        }
        // Release the previous allocation before the provider allocates output.
        drop(self.plain.take());
        let mut len_bytes = [0; 4];
        self.source.read_exact(&mut len_bytes)?;
        let len = u32::from_be_bytes(len_bytes) as usize;
        if !(OVERHEAD..=MAX_FRAME).contains(&len) {
            return Err(invalid("frame length"));
        }
        // Check bounded counters before consuming the frame body. Reserve-Final is checked after authenticating the tag.
        let next = self.budget.next(self.limits, len - OVERHEAD, true)?;
        self.source.read_exact(&mut self.cipher[..len])?;
        let mut aad = self.prefix.clone();
        aad.extend_from_slice(&len_bytes);
        let (plain, tag) = self
            .state
            .as_mut()
            .ok_or(invalid("missing decryption state"))?
            .pull(&self.cipher[..len], Some(&aad))
            .map_err(crypt)?;
        // Own plaintext before any policy/error path can return.
        let plain = Zeroizing::new(plain);
        let size = plain.len();
        if size != len - OVERHEAD {
            return Err(invalid("provider plaintext length"));
        }
        match (self.limits.purpose, tag) {
            (Purpose::Backup, stream::TAG_MESSAGE)
                if size > 0 && next.frames < self.limits.fmax => {}
            (Purpose::Backup, stream::TAG_FINAL) => self.final_seen = true,
            (Purpose::Record, stream::TAG_FINAL) if size > 0 && next.frames == 1 => {
                self.final_seen = true
            }
            _ => return Err(invalid("tag/length/purpose")),
        }
        if self.final_seen {
            drop(self.state.take());
        }
        self.plain = Some(plain);
        self.budget = next;
        self.position = 0;
        self.available = size;
        Ok(())
    }
    pub fn finish(&mut self) -> io::Result<()> {
        let result = (|| {
            if self.poisoned {
                return Err(invalid("reader poisoned"));
            }
            io::copy(self, &mut io::sink())?;
            if !self.final_seen || self.state.is_some() {
                return Err(invalid("missing Final"));
            }
            drop(self.plain.take());
            physical_eof(&mut self.source)
        })();
        if result.is_err() {
            self.poison();
        }
        result
    }
}
impl<R: Read> Read for FrameReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let result = (|| {
            if self.poisoned {
                return Err(invalid("reader poisoned"));
            }
            if out.is_empty() {
                return Ok(0);
            }
            while self.position == self.available {
                if self.final_seen {
                    drop(self.plain.take());
                    return Ok(0);
                }
                self.frame()?;
            }
            let n = out.len().min(self.available - self.position);
            out[..n].copy_from_slice(
                &self.plain.as_ref().ok_or(invalid("missing plaintext"))?
                    [self.position..self.position + n],
            );
            self.position += n;
            Ok(n)
        })();
        if result.is_err() {
            self.poison();
        }
        result
    }
}

#[cfg(test)]
mod tests;
