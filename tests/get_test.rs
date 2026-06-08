use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

fn read_port() -> u16 {
    let cfg = std::fs::read_to_string(
        format!("{}/Config.toml", env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap_or_default();

    let mut in_server = false;
    for raw in cfg.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_server = line == "[server]";
            continue;
        }
        if in_server {
            if let Some(rest) = line.strip_prefix("port") {
                if let Some(eq) = rest.find('=') {
                    if let Ok(p) = rest[eq + 1..].trim().parse::<u16>() {
                        return p;
                    }
                }
            }
        }
    }
    1140
}

struct ChildGuard(std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn get_test_returns_envelope() {
    // Build the binary explicitly so the test exercises the build step.
    let build = Command::new(env!("CARGO"))
        .args(["build", "--bin", "rustopus"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("failed to invoke cargo build");
    assert!(build.success(), "cargo build failed");

    let port = read_port();
    let binary = env!("CARGO_BIN_EXE_rustopus");

    let child = Command::new(binary)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn rustopus binary");
    let _guard = ChildGuard(child);

    let url = format!("http://127.0.0.1:{}/get-test", port);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("failed to build reqwest client");

    // Poll until the server is ready (up to ~10s).
    let mut body = String::new();
    let mut last_err: Option<String> = None;
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(200));
        match client.get(&url).send() {
            Ok(resp) => {
                let status = resp.status();
                match resp.text() {
                    Ok(text) if status.is_success() && !text.is_empty() => {
                        body = text;
                        break;
                    }
                    Ok(text) => last_err = Some(format!("status {}: {}", status, text)),
                    Err(e) => last_err = Some(format!("body read: {}", e)),
                }
            }
            Err(e) => last_err = Some(e.to_string()),
        }
    }

    assert!(
        !body.is_empty(),
        "server did not respond at {} (last error: {:?})",
        url,
        last_err
    );

    // Validate the response shape matches the expected envelope.
    for tag in [
        "<Envelope>",
        "<body>",
        "<response>",
        "<result>",
        "<answer>",
        "<version>",
        "<data>",
        "<ip>",
        "<uuid>",
        "<time>",
        "</Envelope>",
    ] {
        assert!(
            body.contains(tag),
            "missing `{}` in response:\n{}",
            tag,
            body
        );
    }
}
