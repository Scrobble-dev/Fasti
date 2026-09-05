use alkali::{hash::pbkdf::argon2id, symmetric::cipher_stream::Key};
use argon2::{Algorithm, Argon2, Block, Params, Version};
use std::{
    fs,
    io::{self, Read, Write},
    os::fd::AsRawFd,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

const PASSWORD: [u8; 64] = [0x41; 64];
const SALT: [u8; 16] = [0x42; 16];
const LIMIT: u64 = 100_663_296;
const DEADLINE: Duration = Duration::from_secs(5);
// Two maximum-width unsigned integers, separators, prefix and newline fit in 128 bytes.
const RECORD_LIMIT: usize = 128;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[cfg(test)]
mod tests;

fn kdf() -> Result<Argon2<'static>> {
    Ok(Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(65536, 3, 1, Some(32)).map_err(|e| io::Error::other(e.to_string()))?,
    ))
}

fn derive(kdf: &Argon2<'_>, out: &mut [u8]) -> Result<()> {
    let mut scratch = Zeroizing::new(vec![Block::default(); kdf.params().block_count()]);
    let result = kdf.hash_password_into_with_memory(&PASSWORD, &SALT, out, &mut scratch[..]);
    // Explicit Drop invokes the owner's wipe before release, including on error.
    drop(scratch);
    result.map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

fn rss() -> Result<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: usage supplies aligned, writable storage for one rusage value.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: successful getrusage initialized usage; the failure path returned.
    Ok(unsafe { usage.assume_init() }.ru_maxrss as u64 * 1024)
}

fn child(mode: &str) -> Result<()> {
    diagnostics(mode, "before")?;
    alkali::require_init()?;
    let algorithm = kdf()?;
    let mut output = Key::new_empty()?;
    if mode == "oracle" {
        let mut native = Key::new_empty()?;
        argon2id::derive_key(&PASSWORD, &SALT, 3, 64 * 1024 * 1024, &mut native[..])?;
        derive(&algorithm, &mut output[..])?;
        if !alkali::mem::eq(&native[..], &output[..])? {
            return Err("oracle mismatch".into());
        }
        if rss()? > LIMIT {
            return Err("oracle RSS exceeds limit".into());
        }
        io::stdout().write_all(&output[..])?;
        diagnostics(mode, "after")?;
        return Ok(());
    }
    let mut expected = Key::new_empty()?;
    let mut input = io::stdin().lock();
    input.read_exact(&mut expected[..])?;
    if mode == "warm" {
        println!("READY");
        io::stdout().flush()?;
    }
    let count = if mode == "warm" { 136 } else { 1 };
    for _ in 0..count {
        if mode == "warm" {
            let mut go = [0];
            input.read_exact(&mut go)?;
        }
        let start = Instant::now();
        derive(&algorithm, &mut output[..])?;
        let nanos = start.elapsed().as_nanos();
        if !alkali::mem::eq(&expected[..], &output[..])? {
            return Err("sample mismatch".into());
        }
        let peak = rss()?;
        if peak > LIMIT {
            return Err("sample RSS exceeds limit".into());
        }
        println!("SAMPLE {nanos} {peak}");
        io::stdout().flush()?;
    }
    diagnostics(mode, "after")?;
    Ok(())
}

fn diagnostics(mode: &str, phase: &str) -> Result<()> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: usage supplies aligned, writable storage for one rusage value.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: successful getrusage initialized usage; the failure path returned.
    let usage = unsafe { usage.assume_init() };
    let status = fs::read_to_string("/proc/self/status")?;
    let resident = status
        .lines()
        .find(|line| line.starts_with("VmRSS:"))
        .unwrap_or("VmRSS: unavailable");
    eprintln!("DIAG {mode} {phase} {resident} peak_rss_bytes={} user_us={} system_us={} minor_faults={} major_faults={} fd_count={}", usage.ru_maxrss as u64 * 1024, usage.ru_utime.tv_sec * 1_000_000 + usage.ru_utime.tv_usec, usage.ru_stime.tv_sec * 1_000_000 + usage.ru_stime.tv_usec, usage.ru_minflt, usage.ru_majflt, fs::read_dir("/proc/self/fd")?.count());
    Ok(())
}

// Own only the process spawned here. Every error path kills and reaps it.
struct OwnedChild {
    child: Child,
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn spawn(mode: &str) -> Result<OwnedChild> {
    Ok(OwnedChild {
        child: Command::new(std::env::current_exe()?)
            .arg(mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?,
    })
}

// This qualification is Linux-only. Nonblocking owned pipes avoid reader
// threads, queued records and cleanup that could wait on an inherited writer.
fn nonblocking<T: AsRawFd>(pipe: T) -> Result<T> {
    let fd = pipe.as_raw_fd();
    // SAFETY: pipe owns this live descriptor; fcntl does not retain its address.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: the same live owned descriptor remains open; retain every other flag.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error().into());
    }
    Ok(pipe)
}

fn check_deadline(start: Instant) -> Result<()> {
    if start.elapsed() >= DEADLINE {
        return Err("five-second watchdog expired".into());
    }
    Ok(())
}

fn read(pipe: &mut impl Read, bytes: &mut [u8], start: Instant) -> Result<usize> {
    loop {
        check_deadline(start)?;
        match pipe.read(bytes) {
            Ok(count) => return Ok(count),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn write(pipe: &mut impl Write, mut bytes: &[u8], start: Instant) -> Result<()> {
    while !bytes.is_empty() {
        check_deadline(start)?;
        match pipe.write(bytes) {
            Ok(0) => return Err(io::Error::from(io::ErrorKind::WriteZero).into()),
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn eof(pipe: &mut impl Read, start: Instant) -> Result<()> {
    if read(pipe, &mut [0], start)? != 0 {
        return Err("unexpected trailing child output".into());
    }
    Ok(())
}

fn record(pipe: &mut impl Read, start: Instant) -> Result<String> {
    let mut bytes = Vec::with_capacity(RECORD_LIMIT);
    loop {
        let mut byte = [0];
        if read(pipe, &mut byte, start)? == 0 {
            return Err("missing or incomplete child record".into());
        }
        if bytes.len() == RECORD_LIMIT {
            return Err("child record exceeds limit".into());
        }
        bytes.push(byte[0]);
        if byte[0] == b'\n' {
            return Ok(String::from_utf8(bytes)?);
        }
    }
}

fn sample(line: &str) -> Result<(u128, u64)> {
    if line.len() > RECORD_LIMIT {
        return Err("sample exceeds record limit".into());
    }
    let line = line.strip_suffix('\n').ok_or("incomplete sample")?;
    let mut fields = line.split(' ');
    if fields.next() != Some("SAMPLE") {
        return Err("malformed sample".into());
    }
    let nanos = fields.next().ok_or("missing sample time")?;
    let peak = fields.next().ok_or("missing sample RSS")?;
    if fields.next().is_some()
        || nanos.is_empty()
        || peak.is_empty()
        || !nanos.bytes().all(|byte| byte.is_ascii_digit())
        || !peak.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("malformed sample fields".into());
    }
    let nanos = nanos.parse::<u128>()?;
    let peak = peak.parse::<u64>()?;
    if peak > LIMIT {
        return Err("sample RSS exceeded".into());
    }
    Ok((nanos, peak))
}

fn cold_sample(pipe: &mut impl Read, start: Instant) -> Result<String> {
    let line = record(pipe, start)?;
    sample(&line)?;
    eof(pipe, start)?;
    Ok(line)
}

fn wait(process: &mut OwnedChild, start: Instant) -> Result<()> {
    loop {
        check_deadline(start)?;
        if let Some(status) = process.child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!("child failed {status}").into())
            };
        }
        thread::sleep(Duration::from_millis(1));
    }
}

fn read_oracle(pipe: &mut impl Read, bytes: &mut [u8; 32], start: Instant) -> Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        let count = read(pipe, &mut bytes[offset..], start)?;
        if count == 0 {
            return Err("incomplete oracle output".into());
        }
        offset += count;
    }
    eof(pipe, start)
}

fn oracle() -> Result<Key<alkali::mem::FullAccess>> {
    let start = Instant::now();
    let mut process = spawn("oracle")?;
    let mut stdout = nonblocking(process.child.stdout.take().ok_or("missing oracle pipe")?)?;
    let mut key = Key::new_empty()?;
    read_oracle(&mut stdout, (&mut key[..]).try_into()?, start)?;
    wait(&mut process, start)?;
    println!("ORACLE native/RustCrypto match; untimed");
    Ok(key)
}

fn read_cgroup() -> Result<std::path::PathBuf> {
    let location = fs::read_to_string("/proc/self/cgroup")?;
    let relative = location
        .lines()
        .find_map(|l| l.strip_prefix("0::"))
        .ok_or("missing unified cgroup")?;
    if !relative.contains("fasti-c3-kdf-qualify") {
        return Err("not in dedicated qualification cgroup".into());
    }
    Ok(std::path::Path::new("/sys/fs/cgroup").join(relative.trim_start_matches('/')))
}
fn enforce() -> Result<std::path::PathBuf> {
    let path = read_cgroup()?;
    for (name, expected) in [
        ("cpu.max", "100000 100000"),
        ("memory.max", "100663296"),
        ("memory.swap.max", "0"),
    ] {
        let actual = fs::read_to_string(path.join(name))?;
        println!("ENFORCEMENT {name}={}", actual.trim());
        if actual.trim() != expected {
            return Err(format!("unexpected {name}").into());
        }
    }
    let status = fs::read_to_string("/proc/self/status")?;
    let affinity = status
        .lines()
        .find(|l| l.starts_with("Cpus_allowed_list:"))
        .ok_or("no affinity")?;
    println!("ENFORCEMENT {affinity}");
    if affinity.split_whitespace().nth(1) != Some("0") {
        return Err("not pinned to CPU 0".into());
    }
    let mut limit = std::mem::MaybeUninit::<libc::rlimit>::uninit();
    // SAFETY: limit supplies aligned, writable storage for one rlimit value.
    // Short-circuiting evaluates assume_init only after getrlimit succeeds.
    if unsafe { libc::getrlimit(libc::RLIMIT_CORE, limit.as_mut_ptr()) } != 0
        || unsafe { limit.assume_init() }.rlim_cur != 0
    {
        return Err("core dumps not disabled".into());
    }
    println!("ENFORCEMENT core=0");
    Ok(path)
}

fn supervise() -> Result<()> {
    let cgroup = enforce()?;
    println!(
        "HOST load_before={}",
        fs::read_to_string("/proc/loadavg")?.trim()
    );
    let expected = oracle()?;
    let mut process = spawn("warm")?;
    let mut stdin = nonblocking(process.child.stdin.take().ok_or("missing warm input")?)?;
    let mut stdout = nonblocking(process.child.stdout.take().ok_or("missing warm output")?)?;
    write(&mut stdin, &expected[..], Instant::now())?;
    if record(&mut stdout, Instant::now())? != "READY\n" {
        return Err("warm startup failed".into());
    }
    let mut samples = Vec::with_capacity(128);
    for index in 0..136 {
        let deadline_start = Instant::now();
        write(&mut stdin, &[1], deadline_start)?;
        let (nanos, peak) = sample(&record(&mut stdout, deadline_start)?)?;
        println!(
            "{} index={} ns={nanos} peak_rss_bytes={peak}",
            if index < 8 { "WARMUP" } else { "WARM" },
            if index < 8 { index + 1 } else { index - 7 }
        );
        if index >= 8 {
            samples.push(nanos);
        }
    }
    drop(stdin);
    let finish = Instant::now();
    eof(&mut stdout, finish)?;
    wait(&mut process, finish)?;
    let mut cold = Vec::with_capacity(8);
    for index in 0..8 {
        let start = Instant::now();
        let mut process = spawn("cold")?;
        let mut stdin = nonblocking(process.child.stdin.take().ok_or("missing cold input")?)?;
        write(&mut stdin, &expected[..], start)?;
        drop(stdin);
        let mut stdout = nonblocking(process.child.stdout.take().ok_or("missing cold output")?)?;
        wait(&mut process, start)?;
        // Keep the original cold timing boundary: spawn through successful exit.
        let elapsed = start.elapsed().as_nanos();
        let line = cold_sample(&mut stdout, start)?;
        println!(
            "COLD index={} ns={elapsed} child={}",
            index + 1,
            line.trim()
        );
        cold.push(elapsed);
    }
    samples.sort_unstable();
    let p50 = samples[63];
    let p95 = samples[121];
    let p99 = samples[126];
    let cold_max = *cold.iter().max().unwrap();
    println!(
        "SUMMARY warm_n={} p50_ns={p50} p95_ns={p95} p99_ns={p99} cold_n={} cold_max_ns={cold_max}",
        samples.len(),
        cold.len()
    );
    for file in [
        "memory.peak",
        "memory.current",
        "memory.swap.current",
        "memory.events",
        "cpu.stat",
    ] {
        let content = fs::read_to_string(cgroup.join(file))?;
        println!("CGROUP {file} {}", content.trim().replace('\n', ";"));
        if file == "memory.peak" && content.trim().parse::<u64>()? > LIMIT {
            return Err("cgroup peak exceeded".into());
        }
        if file == "memory.events"
            && content
                .lines()
                .any(|l| l.starts_with("oom") && l.split_whitespace().nth(1) != Some("0"))
        {
            return Err("cgroup OOM event".into());
        }
        if file == "memory.swap.current" && content.trim() != "0" {
            return Err("swap observed".into());
        }
    }
    println!(
        "HOST load_after={}",
        fs::read_to_string("/proc/loadavg")?.trim()
    );
    if p50 > 500_000_000 || p95 > 1_000_000_000 || p99 > 2_000_000_000 || cold_max > 2_500_000_000 {
        return Err("frozen latency criterion failed".into());
    }
    println!("QUALIFIED isolated contract c3-kdf-probe-1 only");
    Ok(())
}
fn main() {
    let result = if let Some(mode) = std::env::args().nth(1) {
        child(&mode)
    } else {
        supervise()
    };
    if let Err(error) = result {
        eprintln!("NOT_QUALIFIED {error}");
        std::process::exit(1);
    }
}
