use std::time::Instant;
use velox::terminal::terminal::Terminal;

fn main() {
    println!("=== Velox Terminal Throughput & Grid Benchmark Suite ===");

    // 1. Pure ASCII Streaming Throughput (e.g. cat large logs / text files)
    {
        let mut term = Terminal::new(120, 40);
        let chunk_size = 64 * 1024; // 64 KB chunk
        let mut ascii_chunk = Vec::with_capacity(chunk_size);
        for i in 0..chunk_size {
            let ch = if i % 80 == 79 {
                b'\n'
            } else {
                32 + ((i % 94) as u8)
            };
            ascii_chunk.push(ch);
        }

        let total_bytes = 100 * 1024 * 1024; // 100 MB
        let iterations = total_bytes / chunk_size;

        let start = Instant::now();
        for _ in 0..iterations {
            term.feed(&ascii_chunk);
        }
        let elapsed = start.elapsed();
        let mb_per_sec = (total_bytes as f64 / 1_048_576.0) / elapsed.as_secs_f64();
        let mchars_per_sec = (total_bytes as f64 / 1_000_000.0) / elapsed.as_secs_f64();
        println!(
            "1. Plain ASCII Stream (100 MB): {:?} total -> {:.2} MB/s ({:.2} million chars/sec)",
            elapsed, mb_per_sec, mchars_per_sec
        );
    }

    // 2. Heavy ANSI SGR Colored Text Streaming (e.g. syntax-highlighted build logs / htop)
    {
        let mut term = Terminal::new(120, 40);
        let mut sgr_chunk = Vec::new();
        for line in 0..500 {
            sgr_chunk.extend_from_slice(format!("\x1b[1;3{}m[Velox Line {}]\x1b[0m \x1b[38;2;100;200;255mSyntax Highlighted Output Token\x1b[0m\r\n", (line % 6) + 1, line).as_bytes());
        }

        let total_iterations = 2000;
        let total_bytes = sgr_chunk.len() * total_iterations;

        let start = Instant::now();
        for _ in 0..total_iterations {
            term.feed(&sgr_chunk);
        }
        let elapsed = start.elapsed();
        let mb_per_sec = (total_bytes as f64 / 1_048_576.0) / elapsed.as_secs_f64();
        let lines_per_sec = (500.0 * total_iterations as f64) / elapsed.as_secs_f64();
        println!(
            "2. ANSI SGR 24-bit Color Stream ({} MB): {:?} total -> {:.2} MB/s ({:.2} thousand lines/sec)",
            total_bytes / 1_048_576,
            elapsed,
            mb_per_sec,
            lines_per_sec / 1000.0
        );
    }

    // 3. CSI Cursor Movements & Screen Editing (e.g. Vim, Neovim, Tmux, Nano)
    {
        let mut term = Terminal::new(120, 40);
        let mut csi_script = Vec::new();
        for row in 1..=40 {
            for col in (1..=120).step_by(10) {
                csi_script.extend_from_slice(format!("\x1b[{};{}H\x1b[KText", row, col).as_bytes());
            }
        }

        let total_iterations = 2000;
        let total_ops = 40 * 12 * total_iterations;

        let start = Instant::now();
        for _ in 0..total_iterations {
            term.feed(&csi_script);
        }
        let elapsed = start.elapsed();
        let ops_per_sec = (total_ops as f64) / elapsed.as_secs_f64();
        println!(
            "3. CSI Cursor Jump & Line Erase ({} ops): {:?} total -> {:.2} million ops/sec",
            total_ops,
            elapsed,
            ops_per_sec / 1_000_000.0
        );
    }

    // 4. Memory Grid Scrolling & Scrollback Push (100,000 scrolled lines)
    {
        let mut term = Terminal::new(120, 40);
        let lines_to_scroll = 100_000;
        let line_data = b"The quick brown fox jumps over the lazy dog 1234567890! Velox Terminal Fast Scrollback Engine\r\n";

        let start = Instant::now();
        for _ in 0..lines_to_scroll {
            term.feed(line_data);
        }
        let elapsed = start.elapsed();
        let lines_per_sec = (lines_to_scroll as f64) / elapsed.as_secs_f64();
        println!(
            "4. Grid Scroll & Scrollback Push ({} lines): {:?} total -> {:.2} thousand lines/sec",
            lines_to_scroll,
            elapsed,
            lines_per_sec / 1000.0
        );
    }

    // 5. UTF-8 Multi-byte Characters (CJK, Symbols, Emojis)
    {
        let mut term = Terminal::new(120, 40);
        let utf8_payload =
            "Velox 终端模拟器 🦀 🚀 高性能 Rust Terminal 🌟 [中文字符测试] \u{f07b} 📁\r\n"
                .as_bytes();
        let iterations = 50_000;

        let start = Instant::now();
        for _ in 0..iterations {
            term.feed(utf8_payload);
        }
        let elapsed = start.elapsed();
        let lines_per_sec = (iterations as f64) / elapsed.as_secs_f64();
        println!(
            "5. UTF-8 CJK & Emoji Stream ({} lines): {:?} total -> {:.2} thousand lines/sec",
            iterations,
            elapsed,
            lines_per_sec / 1000.0
        );
    }

    println!("=== Benchmark Complete ===");
}
