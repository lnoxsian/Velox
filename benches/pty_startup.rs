use std::sync::Arc;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};
use velox::app::app::{CustomEvent, EventLoopProxy};
use velox::pty::reactor::PtyReactor;
use velox::pty::{spawn_process, spawn_terminal_pty_with_size};

fn main() {
    println!("=== Velox PTY Initialization & Startup Benchmark Suite ===");

    // 1. PTY Process Spawn & Window Geometry Initialization Latency
    {
        let iterations = 50;
        let start = Instant::now();
        let mut pids = Vec::with_capacity(iterations);

        for _ in 0..iterations {
            let pty = spawn_terminal_pty_with_size("/bin/sh", None, None, Some((120, 40)))
                .expect("Failed to spawn PTY");
            if let Some(pid) = pty.child_pid {
                pids.push(pid);
            }
        }
        let elapsed = start.elapsed();
        let avg_spawn = elapsed / iterations as u32;

        for pid in pids {
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
                let mut status = 0;
                libc::waitpid(pid as i32, &mut status, 0);
            }
        }

        println!(
            "1. PTY Process Spawn Latency ({} iterations): {:?} total -> {:?} per spawn",
            iterations, elapsed, avg_spawn
        );
    }

    // 2. Foreground Process Title & Working Directory Retrieval
    {
        let cmd = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "sleep 3".to_string(),
        ];
        let pty = spawn_process("/bin/sh", Some(&cmd), Some("/tmp"))
            .expect("Failed to spawn process for title benchmark");
        std::thread::sleep(Duration::from_millis(50));

        let iterations = 10_000;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = pty.get_foreground_process_name();
            let _ = pty.get_foreground_process_cwd();
        }
        let elapsed = start.elapsed();
        let per_query = elapsed / (iterations * 2) as u32;
        let queries_per_sec = (iterations as f64 * 2.0) / elapsed.as_secs_f64();

        if let Some(pid) = pty.child_pid {
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
                let mut status = 0;
                libc::waitpid(pid as i32, &mut status, 0);
            }
        }

        println!(
            "2. /proc Foreground Process & CWD Polling ({} queries): {:?} total -> {:?} per query ({:.2} queries/sec)",
            iterations * 2,
            elapsed,
            per_query,
            queries_per_sec
        );
    }

    // 3. PTY Reactor Registration and First Prompt Turnaround
    {
        let (sender, receiver) = channel();
        let proxy = EventLoopProxy::new(sender, -1);
        let reactor = PtyReactor::new(proxy).expect("Failed to create reactor");

        let iterations = 20;
        let mut total_latency = Duration::ZERO;

        for i in 0..iterations {
            let cmd = vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "printf 'READY\\n'".to_string(),
            ];
            let t0 = Instant::now();
            let pty = Arc::new(
                spawn_terminal_pty_with_size("/bin/sh", Some(&cmd), None, Some((80, 24)))
                    .expect("Failed to spawn test shell"),
            );
            let child_pid = pty.child_pid;

            reactor
                .register(1, 0, i as u64, pty)
                .expect("Failed to register PTY");

            let ev = receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("Timed out waiting for initial PTY output");

            total_latency += t0.elapsed();

            if let CustomEvent::PtyData { pane_id, data, .. } = ev {
                assert_eq!(pane_id, i as u64);
                let out = String::from_utf8_lossy(&data);
                assert!(out.contains("READY"));
            }

            reactor.unregister(1, 0, i as u64).unwrap();

            if let Some(pid) = child_pid {
                let mut status = 0;
                unsafe {
                    libc::waitpid(pid as i32, &mut status, 0);
                }
            }
        }

        let avg_turnaround = total_latency / iterations as u32;
        println!(
            "3. Spawn-to-First-Byte Turnaround ({} rounds): {:?} total -> {:?} avg latency",
            iterations, total_latency, avg_turnaround
        );
    }

    println!("=== PTY Benchmark Complete ===");
}
