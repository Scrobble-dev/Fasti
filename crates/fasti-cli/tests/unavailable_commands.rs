use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fasti"))
        .args(args)
        .output()
        .expect("fasti CLI should start")
}

fn assert_unavailable(output: Output, command: &str) {
    assert!(
        !output.status.success(),
        "{command} must not report success"
    );
    assert!(
        output.stdout.is_empty(),
        "{command} must not emit a success receipt"
    );

    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("not available in B0"));
    assert!(stderr.contains("No data was changed"));
    assert!(stderr.contains("no success receipt was emitted"));
}

#[test]
fn export_is_an_explicit_nonzero_stub() {
    assert_unavailable(run(&["export", "--output", "unused.fasti"]), "export");
}

#[test]
fn restore_is_an_explicit_nonzero_stub() {
    assert_unavailable(run(&["restore", "--input", "missing.fasti"]), "restore");
}

#[test]
fn verify_is_an_explicit_nonzero_stub() {
    assert_unavailable(run(&["verify"]), "verify");
}
