use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
fn newline_request_finishes_even_if_stdin_writer_stays_open() {
    let root = std::env::temp_dir().join(format!("vela-protocol-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    let mut child = Command::new(env!("CARGO_BIN_EXE_vela-core"))
        .env("VELA_HOME", &root)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().unwrap();
    let mut writer = child.stdin.take().unwrap();
    writer.write_all(b"{\"op\":\"list\"}\n").unwrap();
    writer.flush().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = child.try_wait().unwrap() { assert!(status.success()); break; }
        if Instant::now() >= deadline { child.kill().unwrap(); child.wait().unwrap(); panic!("Core waited for EOF instead of handling a complete line"); }
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut output = String::new();
    child.stdout.take().unwrap().read_to_string(&mut output).unwrap();
    let reply: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(reply["ok"], true);
    drop(writer);
    std::fs::remove_dir_all(root).unwrap();
}
