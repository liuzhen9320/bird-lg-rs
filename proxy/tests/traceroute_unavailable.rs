use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

struct RunningProxy(Child);

impl Drop for RunningProxy {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn missing_traceroute_returns_a_readable_service_unavailable_response() {
    for configured_missing_binary in [false, true] {
        let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reservation.local_addr().unwrap();
        drop(reservation);
        let mut command = Command::new(env!("CARGO_BIN_EXE_bird-lgproxy-rs"));
        command
            .env_clear()
            .env("PATH", "/nonexistent")
            .env("RUST_LOG", "off")
            .arg(format!("--listen={address}"))
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        if configured_missing_binary {
            command.args([
                "--traceroute-bin=/nonexistent/traceroute",
                "--traceroute-flags=-q1",
            ]);
        }
        let mut proxy = RunningProxy(command.spawn().unwrap());
        let mut connection = None;
        for _ in 0..200 {
            assert!(
                proxy.0.try_wait().unwrap().is_none(),
                "proxy exited during startup"
            );
            if let Ok(stream) = TcpStream::connect(address) {
                connection = Some(stream);
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let mut stream = connection.expect("proxy failed to start");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream.write_all(b"GET /traceroute?q=127.0.0.1 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(
            response.starts_with("HTTP/1.1 503 Service Unavailable\r\n"),
            "{response}"
        );
        assert!(
            response.contains("Traceroute is unavailable on this node"),
            "{response}"
        );
        assert!(
            response.contains("no working traceroute or mtr executable was found"),
            "{response}"
        );
        assert!(response.contains("restart the proxy"), "{response}");
    }
}
