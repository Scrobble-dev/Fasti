use super::*;
use std::io::Cursor;

fn fixture(script: &str) -> OwnedChild {
    OwnedChild {
        child: Command::new("/bin/sh")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    }
}

fn assert_reaped(pid: u32) {
    let mut status = 0;
    // SAFETY: waitpid receives a valid output pointer and only our spawned PID.
    assert_eq!(
        unsafe { libc::waitpid(pid as libc::pid_t, &mut status, libc::WNOHANG) },
        -1,
        "owned child was left running or unreaped"
    );
    assert_eq!(
        io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
}

#[test]
fn strict_sample_fields_and_bounds() {
    assert_eq!(sample("SAMPLE 1 100663296\n").unwrap(), (1, LIMIT));
    assert_eq!(
        sample(&format!("SAMPLE {} 0\n", u128::MAX)).unwrap(),
        (u128::MAX, 0)
    );
    for input in [
        "",
        "SAMPLE",
        "SAMPLE 1\n",
        "SAMPLE 1 2",
        "SAMPLE 1 2 extra\n",
        "SAMPLE 1 2\nSAMPLE 3 4\n",
        "SAMPLE nope 2\n",
        "SAMPLE 1 nope\n",
        "SAMPLE -1 2\n",
        "SAMPLE +1 2\n",
        "SAMPLE 1 -2\n",
        "SAMPLE 1 100663297\n",
        "SAMPLE 340282366920938463463374607431768211456 2\n",
        "SAMPLE 1 18446744073709551616\n",
        "SAMPLE  1 2\n",
        "SAMPLE\t1 2\n",
        "SAMPLE 1 2\r\n",
    ] {
        assert!(sample(input).is_err(), "accepted {input:?}");
    }
    assert!(sample(&"1".repeat(RECORD_LIMIT + 1)).is_err());
}

#[test]
fn cold_requires_one_complete_valid_sample_and_eof() {
    let valid = b"SAMPLE 123 456\n";
    assert_eq!(
        cold_sample(&mut Cursor::new(valid), Instant::now()).unwrap(),
        String::from_utf8(valid.to_vec()).unwrap()
    );
    for input in [
        &b""[..],
        &b"not a sample\n"[..],
        &b"SAMPLE 1 2"[..],
        &b"SAMPLE 1 100663297\n"[..],
        &b"SAMPLE 1 2\nSAMPLE 3 4\n"[..],
        &b"SAMPLE 1 2\n\n"[..],
    ] {
        assert!(cold_sample(&mut Cursor::new(input), Instant::now()).is_err());
    }
}

#[test]
fn record_storage_is_bounded_and_utf8_checked() {
    for bytes in [
        vec![b'x'; RECORD_LIMIT + 1],
        [vec![b'x'; RECORD_LIMIT], vec![b'\n']].concat(),
        vec![0xff, b'\n'],
    ] {
        assert!(record(&mut Cursor::new(bytes), Instant::now()).is_err());
    }
    let max_record = [vec![b'x'; RECORD_LIMIT - 1], vec![b'\n']].concat();
    assert_eq!(
        record(&mut Cursor::new(max_record), Instant::now())
            .unwrap()
            .len(),
        RECORD_LIMIT
    );
}

#[test]
fn oracle_requires_exactly_32_bytes_and_eof() {
    for length in [0, 31, 33, 4096] {
        assert!(read_oracle(
            &mut Cursor::new(vec![0x55; length]),
            &mut [0; 32],
            Instant::now()
        )
        .is_err());
    }
    let mut output = [0; 32];
    read_oracle(&mut Cursor::new([0x55; 32]), &mut output, Instant::now()).unwrap();
    assert_eq!(output, [0x55; 32]);
}

#[test]
fn parse_error_kills_and_reaps_the_owned_child() {
    let mut pid = 0;
    let result = (|| -> Result<()> {
        let mut process = fixture("printf 'SAMPLE nope 2\n'; read -r line");
        pid = process.child.id();
        let mut pipe = nonblocking(process.child.stdout.take().unwrap())?;
        sample(&record(&mut pipe, Instant::now())?)?;
        Ok(())
    })();
    assert!(result.is_err());
    assert_reaped(pid);
}

#[test]
fn writes_preserve_bytes_and_reject_zero_progress_or_expired_deadline() {
    let mut output = Vec::new();
    write(&mut output, b"abc", Instant::now()).unwrap();
    assert_eq!(output, b"abc");

    let mut short = [0; 2];
    let error = write(&mut &mut short[..], b"abc", Instant::now()).unwrap_err();
    assert_eq!(
        error.downcast_ref::<io::Error>().unwrap().kind(),
        io::ErrorKind::WriteZero
    );
    assert_eq!(short, *b"ab");

    let error = write(&mut output, b"d", Instant::now() - DEADLINE).unwrap_err();
    assert_eq!(error.to_string(), "five-second watchdog expired");
    assert_eq!(output, b"abc");
}

#[test]
fn pipe_error_kills_and_reaps_the_owned_child() {
    let mut pid = 0;
    let result = (|| -> Result<()> {
        let mut process = fixture("exec 0<&-; printf 'READY\n'; exec /bin/sleep 30");
        pid = process.child.id();
        let mut pipe = nonblocking(process.child.stdout.take().unwrap())?;
        assert_eq!(record(&mut pipe, Instant::now())?, "READY\n");
        let mut input = nonblocking(process.child.stdin.take().unwrap())?;
        write(&mut input, &[1], Instant::now())?;
        Ok(())
    })();
    assert!(result.is_err());
    assert_reaped(pid);
}

#[test]
fn successful_child_exit_remains_successful() {
    let mut process = fixture("printf 'SAMPLE 1 2\n'");
    let pid = process.child.id();
    let mut pipe = nonblocking(process.child.stdout.take().unwrap()).unwrap();
    let start = Instant::now();
    wait(&mut process, start).unwrap();
    cold_sample(&mut pipe, start).unwrap();
    drop(process);
    assert_reaped(pid);
}

#[test]
fn successful_exit_cannot_hide_missing_or_malformed_cold_output() {
    for script in ["exit 0", "printf 'bad\n'", "printf 'SAMPLE 1 2\nextra\n'"] {
        let mut process = fixture(script);
        let pid = process.child.id();
        let mut pipe = nonblocking(process.child.stdout.take().unwrap()).unwrap();
        let start = Instant::now();
        wait(&mut process, start).unwrap();
        assert!(cold_sample(&mut pipe, start).is_err());
        drop(process);
        assert_reaped(pid);
    }
}

#[test]
fn nonzero_child_exit_is_rejected_and_reaped() {
    let mut process = fixture("exit 7");
    let pid = process.child.id();
    assert!(wait(&mut process, Instant::now()).is_err());
    drop(process);
    assert_reaped(pid);
}

#[test]
fn expired_deadline_rejects_ready_output_and_cleans_up() {
    let mut process = fixture("printf 'READY\n'; read -r line");
    let pid = process.child.id();
    let mut pipe = nonblocking(process.child.stdout.take().unwrap()).unwrap();
    let expired = Instant::now() - DEADLINE;
    assert!(record(&mut pipe, expired).is_err());
    assert!(wait(&mut process, expired).is_err());
    drop(process);
    assert_reaped(pid);
}

#[test]
fn trailing_output_and_eof_wait_stay_inside_deadline() {
    let mut process = fixture("printf 'SAMPLE 1 2\n'; read -r line");
    let pid = process.child.id();
    let mut pipe = nonblocking(process.child.stdout.take().unwrap()).unwrap();
    sample(&record(&mut pipe, Instant::now()).unwrap()).unwrap();
    assert!(eof(&mut pipe, Instant::now() - DEADLINE).is_err());
    drop(process);
    assert_reaped(pid);
}

#[test]
fn oracle_eof_timeout_kills_and_reaps_an_owned_writer() {
    let mut pid = 0;
    let result = (|| -> Result<()> {
        let mut process = fixture("printf 'READY\n00000000000000000000000000000000'; read -r line");
        pid = process.child.id();
        let mut pipe = nonblocking(process.child.stdout.take().unwrap())?;
        assert_eq!(record(&mut pipe, Instant::now())?, "READY\n");
        let start = Instant::now() - DEADLINE + Duration::from_millis(150);
        read_oracle(&mut pipe, &mut [0; 32], start)
    })();
    assert_eq!(
        result.unwrap_err().to_string(),
        "five-second watchdog expired"
    );
    assert_reaped(pid);
}

#[test]
fn owned_pipe_nonblocking_preserves_other_flags() {
    let mut process = fixture("read -r line");
    let pipe = process.child.stdout.take().unwrap();
    // SAFETY: this is a live descriptor owned by pipe.
    let before = unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_GETFL) };
    let pipe = nonblocking(pipe).unwrap();
    // SAFETY: pipe still owns the same descriptor.
    let after = unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_GETFL) };
    assert_eq!(after, before | libc::O_NONBLOCK);
}
