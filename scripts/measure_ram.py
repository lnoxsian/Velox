#!/usr/bin/env python3
"""
scripts/measure_ram.py - Monitors and reports the real-time and peak RAM (RSS, PSS, VmSize)
usage of the Velox binary (or any specified command) on Linux using /proc metrics.
"""

import sys
import os
import time
import subprocess
import signal

def parse_proc_status(pid):
    """Read memory metrics in KiB from /proc/<pid>/status."""
    status_path = f"/proc/{pid}/status"
    if not os.path.exists(status_path):
        return None

    metrics = {}
    try:
        with open(status_path, "r", encoding="utf-8") as f:
            for line in f:
                parts = line.split(":")
                if len(parts) == 2:
                    key = parts[0].strip()
                    val = parts[1].strip()
                    metrics[key] = val
    except (FileNotFoundError, ProcessLookupError, PermissionError):
        return None

    def get_kb(key):
        val_str = metrics.get(key, "0 kB").split()[0]
        try:
            return int(val_str)
        except ValueError:
            return 0

    return {
        "vm_rss_kb": get_kb("VmRSS"),       # Current Resident Set Size
        "vm_hwm_kb": get_kb("VmHWM"),       # Peak Resident Set Size (High Water Mark)
        "vm_size_kb": get_kb("VmSize"),     # Virtual Memory Size
        "rss_anon_kb": get_kb("RssAnon"),   # Anonymous / Heap RSS
        "rss_file_kb": get_kb("RssFile"),   # File-backed / Code RSS
        "rss_shmem_kb": get_kb("RssShmem"), # Shared Memory RSS
    }

def parse_proc_smaps_rollup(pid):
    """Read PSS and Private memory metrics from /proc/<pid>/smaps_rollup if available."""
    smaps_path = f"/proc/{pid}/smaps_rollup"
    if not os.path.exists(smaps_path):
        return None

    metrics = {}
    try:
        with open(smaps_path, "r", encoding="utf-8") as f:
            for line in f:
                parts = line.split(":")
                if len(parts) == 2:
                    key = parts[0].strip()
                    val = parts[1].strip()
                    metrics[key] = val
    except (FileNotFoundError, ProcessLookupError, PermissionError):
        return None

    def get_kb(key):
        val_str = metrics.get(key, "0 kB").split()[0]
        try:
            return int(val_str)
        except ValueError:
            return 0

    return {
        "pss_kb": get_kb("Pss"),
        "private_clean_kb": get_kb("Private_Clean"),
        "private_dirty_kb": get_kb("Private_Dirty"),
        "shared_clean_kb": get_kb("Shared_Clean"),
        "shared_dirty_kb": get_kb("Shared_Dirty"),
    }

def format_mb(kb):
    return f"{kb / 1024.0:.2f} MB ({kb:,} KB)"

def resolve_binary(args):
    """Resolve binary path and remaining arguments from CLI input."""
    profile = None
    cmd = []
    
    if args and (args[0] in ("--profile", "-p")):
        if len(args) > 1:
            profile = args[1]
            remaining = args[2:]
        else:
            remaining = []
    elif args and args[0] in ("optimized-release", "release", "debug", "dev"):
        profile = args[0]
        remaining = args[1:]
    else:
        remaining = args

    if profile:
        profile_dir = "debug" if profile in ("debug", "dev") else profile
        bin_path = f"target/{profile_dir}/velox"
        if not os.path.exists(bin_path):
            print(f"[!] Binary '{bin_path}' not found. Building with cargo build --profile {profile}...")
            build_cmd = ["cargo", "build"]
            if profile == "release":
                build_cmd.append("--release")
            elif profile != "debug" and profile != "dev":
                build_cmd.extend(["--profile", profile])
            res = subprocess.run(build_cmd)
            if res.returncode != 0:
                print(f"[X] Cargo build failed for profile '{profile}'.")
                sys.exit(1)
        cmd = [bin_path] + remaining
    elif remaining:
        bin_path = remaining[0]
        if not os.path.exists(bin_path):
            if os.path.exists(f"target/optimized-release/{bin_path}"):
                bin_path = f"target/optimized-release/{bin_path}"
            elif os.path.exists(f"target/release/{bin_path}"):
                bin_path = f"target/release/{bin_path}"
            elif os.path.exists(f"target/debug/{bin_path}"):
                bin_path = f"target/debug/{bin_path}"
        cmd = [bin_path] + remaining[1:]
    else:
        # Default binary fallback: optimized-release > release > debug
        default_bin = "target/release/velox"
        if os.path.exists("target/optimized-release/velox"):
            default_bin = "target/optimized-release/velox"
        elif not os.path.exists(default_bin) and os.path.exists("target/debug/velox"):
            default_bin = "target/debug/velox"
        elif not os.path.exists(default_bin):
            print("[!] Binary not found. Building release binary with cargo build --release...")
            res = subprocess.run(["cargo", "build", "--release"])
            if res.returncode != 0:
                print("[X] Cargo build failed.")
                sys.exit(1)
            default_bin = "target/release/velox"
        cmd = [default_bin]

    return cmd

def main():
    cmd = resolve_binary(sys.argv[1:])

    print("=" * 65)
    print(f"  Velox RAM Usage Profiler")
    print(f"  Target Command : {' '.join(cmd)}")
    print("=" * 65)
    print("Launching process and monitoring memory usage (Ctrl+C or close window to stop)...\n")

    start_time = time.time()
    try:
        proc = subprocess.Popen(cmd)
    except Exception as e:
        print(f"[X] Failed to launch {' '.join(cmd)}: {e}")
        sys.exit(1)

    pid = proc.pid
    samples = []
    initial_rss = None
    peak_rss = 0
    peak_pss = 0
    peak_vmsize = 0

    # Wait briefly for process startup to stabilize
    time.sleep(0.05)

    try:
        while True:
            if proc.poll() is not None:
                break

            status = parse_proc_status(pid)
            if status is None:
                break

            smaps = parse_proc_smaps_rollup(pid)

            rss = status["vm_rss_kb"]
            hwm = status["vm_hwm_kb"]
            vmsize = status["vm_size_kb"]
            pss = smaps["pss_kb"] if smaps else 0

            if initial_rss is None and rss > 0:
                initial_rss = rss

            peak_rss = max(peak_rss, rss, hwm)
            peak_pss = max(peak_pss, pss)
            peak_vmsize = max(peak_vmsize, vmsize)

            samples.append({
                "time": time.time() - start_time,
                "rss_kb": rss,
                "pss_kb": pss,
                "vmsize_kb": vmsize,
                "anon_kb": status["rss_anon_kb"],
            })

            # Live single-line status
            pss_str = f" | PSS: {pss/1024.0:.1f} MB" if smaps else ""
            print(f"\r  [PID {pid}] RSS: {rss/1024.0:.2f} MB (Peak: {peak_rss/1024.0:.2f} MB){pss_str} | VmSize: {vmsize/1024.0:.1f} MB  ", end="", flush=True)

            time.sleep(0.1)

    except KeyboardInterrupt:
        print("\n\nStopping process...")
        try:
            proc.send_signal(signal.SIGINT)
            proc.wait(timeout=1.0)
        except Exception:
            proc.kill()

    duration = time.time() - start_time
    print("\n")
    print("=" * 65)
    print("  RAM Usage Summary Report")
    print("=" * 65)

    if samples:
        avg_rss = sum(s["rss_kb"] for s in samples) / len(samples)
        last_rss = samples[-1]["rss_kb"]
        init_rss = initial_rss or samples[0]["rss_kb"]
        last_anon = samples[-1]["anon_kb"]

        print(f"  Duration Monitored  : {duration:.2f} seconds ({len(samples)} samples)")
        print(f"  Initial Startup RSS : {format_mb(init_rss)}")
        print(f"  Average Active RSS  : {format_mb(int(avg_rss))}")
        print(f"  Peak Physical (RSS) : {format_mb(peak_rss)}")
        if peak_pss > 0:
            print(f"  Peak Proportional   : {format_mb(peak_pss)} (PSS)")
        print(f"  Anonymous / Heap    : {format_mb(last_anon)}")
        print(f"  Peak Virtual Memory : {format_mb(peak_vmsize)} (VmSize)")
        print(f"  Final Exit RSS      : {format_mb(last_rss)}")
    else:
        print("  Process exited immediately. No samples recorded.")

    print("=" * 65)

if __name__ == "__main__":
    main()
