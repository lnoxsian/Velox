#!/usr/bin/env python3
r"""
scripts/measure_cpu.py - Single-Core CPU Usage Profiler & Benchmark for Velox

Runs and profiles Velox binaries pinned strictly to a single CPU core.
Measures user and system CPU time, instantaneous and average CPU utilization,
context switch rates, per-thread CPU distribution, and memory footprint.
Supports comparing multiple builds/profiles (debug, release, optimized-release)
across standardized or custom workloads.

Usage:
    # Profile default release binary interactively pinned to Core 0:
    ./scripts/measure_cpu.py

    # Profile for a fixed duration of 5 seconds on Core 2:
    ./scripts/measure_cpu.py --core 2 --duration 5

    # Benchmark idle CPU efficiency of the release build:
    ./scripts/measure_cpu.py --workload idle --duration 5

    # Benchmark and compare all profiles (debug, release, optimized-release):
    ./scripts/measure_cpu.py --all-profiles --workload stream-ascii

    # Compare arbitrary binaries:
    ./scripts/measure_cpu.py --compare Baseline:target/release/velox Optimized:target/optimized-release/velox

    # Run with a custom shell command inside Velox:
    ./scripts/measure_cpu.py -e "bash -c 'for i in {1..1000}; do echo \$i; done'"

    # Export full metrics and comparison table to JSON and Markdown:
    ./scripts/measure_cpu.py --all-profiles --json cpu_report.json --markdown
"""

import sys
import os
import time
import subprocess
import signal
import argparse
import json
import shutil
import math
from typing import Dict, List, Optional, Any, Tuple

# System clock ticks per second (usually 100 on Linux)
CLK_TCK = os.sysconf(os.sysconf_names.get('SC_CLK_TCK', 100))

# ANSI Color Codes for terminal formatting
COLOR_RESET = "\033[0m"
COLOR_BOLD = "\033[1m"
COLOR_DIM = "\033[2m"
COLOR_GREEN = "\033[32m"
COLOR_YELLOW = "\033[33m"
COLOR_RED = "\033[31m"
COLOR_CYAN = "\033[36m"
COLOR_MAGENTA = "\033[35m"
COLOR_BLUE = "\033[34m"

def is_color_enabled() -> bool:
    return sys.stdout.isatty() and os.environ.get("NO_COLOR") is None

def colorize(text: str, color: str) -> str:
    if is_color_enabled():
        return f"{color}{text}{COLOR_RESET}"
    return text

def get_cpu_info(core_id: int) -> Dict[str, Any]:
    """Retrieve CPU model name, frequency, and core topology from Linux /proc."""
    info = {
        "model_name": "Unknown CPU",
        "mhz": 0.0,
        "total_cores": os.cpu_count() or 1,
        "core_id": core_id,
    }
    try:
        with open("/proc/cpuinfo", "r", encoding="utf-8") as f:
            for line in f:
                if "model name" in line and info["model_name"] == "Unknown CPU":
                    info["model_name"] = line.split(":", 1)[1].strip()
                elif "cpu MHz" in line and info["mhz"] == 0.0:
                    try:
                        info["mhz"] = float(line.split(":", 1)[1].strip())
                    except ValueError:
                        pass
    except Exception:
        pass

    # Try reading current scaling frequency for the specified core
    freq_path = f"/sys/devices/system/cpu/cpu{core_id}/cpufreq/scaling_cur_freq"
    if os.path.exists(freq_path):
        try:
            with open(freq_path, "r", encoding="utf-8") as f:
                khz = int(f.read().strip())
                info["mhz"] = khz / 1000.0
        except Exception:
            pass

    return info

def parse_proc_stat(pid: int) -> Optional[Dict[str, int]]:
    """
    Read user time, system time, and thread count from /proc/<pid>/stat.
    Returns utime and stime in clock ticks.
    """
    stat_path = f"/proc/{pid}/stat"
    try:
        with open(stat_path, "r", encoding="utf-8") as f:
            content = f.read()
        # Find closing paren to safely skip comm which might contain spaces or parens
        rparen = content.rfind(")")
        if rparen == -1:
            return None
        fields = content[rparen + 2:].split()
        # fields[11] is utime (14th field in 1-based index)
        # fields[12] is stime (15th field in 1-based index)
        # fields[17] is num_threads (20th field in 1-based index)
        return {
            "utime": int(fields[11]),
            "stime": int(fields[12]),
            "num_threads": int(fields[17]),
        }
    except (FileNotFoundError, ProcessLookupError, PermissionError, IndexError, ValueError):
        return None

def parse_proc_status(pid: int) -> Optional[Dict[str, int]]:
    """
    Read context switches and memory RSS from /proc/<pid>/status.
    """
    status_path = f"/proc/{pid}/status"
    try:
        metrics = {}
        with open(status_path, "r", encoding="utf-8") as f:
            for line in f:
                parts = line.split(":", 1)
                if len(parts) == 2:
                    metrics[parts[0].strip()] = parts[1].strip()

        def parse_kb(key: str) -> int:
            val_str = metrics.get(key, "0 kB").split()[0]
            try:
                return int(val_str)
            except ValueError:
                return 0

        def parse_int(key: str) -> int:
            try:
                return int(metrics.get(key, "0"))
            except ValueError:
                return 0

        return {
            "voluntary_ctxt_switches": parse_int("voluntary_ctxt_switches"),
            "nonvoluntary_ctxt_switches": parse_int("nonvoluntary_ctxt_switches"),
            "vm_rss_kb": parse_kb("VmRSS"),
            "vm_hwm_kb": parse_kb("VmHWM"),
        }
    except (FileNotFoundError, ProcessLookupError, PermissionError):
        return None

def parse_thread_metrics(pid: int) -> Dict[int, Dict[str, Any]]:
    """
    Read per-thread CPU utilization from /proc/<pid>/task/<tid>.
    """
    tasks_dir = f"/proc/{pid}/task"
    threads = {}
    if not os.path.exists(tasks_dir):
        return threads

    try:
        for tid_str in os.listdir(tasks_dir):
            if not tid_str.isdigit():
                continue
            tid = int(tid_str)
            comm_path = f"{tasks_dir}/{tid_str}/comm"
            stat_path = f"{tasks_dir}/{tid_str}/stat"

            comm = "unknown"
            if os.path.exists(comm_path):
                try:
                    with open(comm_path, "r", encoding="utf-8") as f:
                        comm = f.read().strip()
                except Exception:
                    pass

            if os.path.exists(stat_path):
                try:
                    with open(stat_path, "r", encoding="utf-8") as f:
                        content = f.read()
                    rparen = content.rfind(")")
                    if rparen != -1:
                        fields = content[rparen + 2:].split()
                        utime = int(fields[11])
                        stime = int(fields[12])
                        threads[tid] = {
                            "comm": comm,
                            "utime": utime,
                            "stime": stime,
                            "total_ticks": utime + stime,
                        }
                except Exception:
                    pass
    except (FileNotFoundError, PermissionError):
        pass

    return threads

def build_profile_if_needed(profile: str) -> str:
    """Build binary for specified cargo profile if missing, and return path."""
    profile_dir = "debug" if profile in ("debug", "dev") else profile
    bin_path = f"target/{profile_dir}/velox"

    if not os.path.exists(bin_path):
        print(colorize(f"[*] Binary '{bin_path}' not found. Compiling profile '{profile}'...", COLOR_YELLOW))
        build_cmd = ["cargo", "build"]
        if profile == "release":
            build_cmd.append("--release")
        elif profile not in ("debug", "dev"):
            build_cmd.extend(["--profile", profile])

        t0 = time.time()
        res = subprocess.run(build_cmd)
        if res.returncode != 0:
            print(colorize(f"[X] Compilation failed for cargo profile '{profile}'.", COLOR_RED))
            sys.exit(1)
        print(colorize(f"[+] Compiled '{bin_path}' in {time.time() - t0:.2f}s.", COLOR_GREEN))

    return bin_path

def resolve_target_binary(target: str) -> Tuple[str, str]:
    """
    Resolve target argument (e.g. 'release', 'debug', or path).
    Returns (label, binary_path).
    """
    if ":" in target and not os.path.exists(target):
        label, path = target.split(":", 1)
        return label, path

    if target in ("debug", "release", "optimized-release"):
        bin_path = build_profile_if_needed(target)
        return target, bin_path

    if os.path.exists(target):
        label = os.path.basename(os.path.dirname(os.path.abspath(target)))
        if label in ("target", ".", ""):
            label = os.path.basename(target)
        return label, target

    # Check target/<profile>/target
    for prof in ("optimized-release", "release", "debug"):
        candidate = f"target/{prof}/{target}"
        if os.path.exists(candidate):
            return prof, candidate

    # Fallback to building profile if target is named after profile
    bin_path = build_profile_if_needed(target)
    return target, bin_path

def get_workload_command(workload_type: str, duration: Optional[float] = None) -> List[str]:
    """Generate shell workload command for automated benchmarking."""
    dur = duration if duration is not None else 5.0

    if workload_type == "idle":
        # Pure idle wait
        return ["python3", "-c", f"import time; time.sleep({dur})"]

    elif workload_type == "stream-ascii":
        # High-throughput ASCII streaming
        script = (
            "import sys, time; "
            "line = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()_+-=~[]{}|;:<>?,./\\n' * 5; "
            "chunk = line * 50; "
            f"t_end = time.time() + {dur}; "
            "while time.time() < t_end: "
            "    sys.stdout.write(chunk); sys.stdout.flush()"
        )
        return ["python3", "-c", script]

    elif workload_type == "stream-ansi":
        # High-throughput 24-bit TrueColor ANSI sequence streaming
        script = (
            "import sys, time; "
            "colors = ''.join(f'\\x1b[38;2;{(i*17)%256};{(i*31)%256};{(i*47)%256}m[Velox Benchmark Token {i:04d}]\\x1b[0m ' for i in range(20)) + '\\n'; "
            "chunk = colors * 30; "
            f"t_end = time.time() + {dur}; "
            "while time.time() < t_end: "
            "    sys.stdout.write(chunk); sys.stdout.flush()"
        )
        return ["python3", "-c", script]

    elif workload_type == "scroll":
        # Rapid multiline scrollback generator
        script = (
            "import sys, time; "
            "line = 'Velox Terminal Scrollback Throughput Profiler Line Buffer Fill Test\\n'; "
            "chunk = line * 100; "
            f"t_end = time.time() + {dur}; "
            "while time.time() < t_end: "
            "    sys.stdout.write(chunk); sys.stdout.flush()"
        )
        return ["python3", "-c", script]

    else:
        raise ValueError(f"Unknown workload type: '{workload_type}'")

def run_single_profile(
    binary_path: str,
    label: str,
    core_id: int,
    workload_cmd: Optional[List[str]] = None,
    duration: Optional[float] = None,
    interval: float = 0.1,
    hold: bool = False,
) -> Dict[str, Any]:
    """
    Launch binary pinned to single CPU core, monitor its CPU consumption,
    and return complete performance statistics.
    """
    total_cores = os.cpu_count() or 1
    if not (0 <= core_id < total_cores):
        print(colorize(f"[X] Invalid core {core_id}. System has {total_cores} cores (0..{total_cores - 1}).", COLOR_RED))
        sys.exit(1)

    # Prepare command line
    cmd = [binary_path]
    if hold:
        cmd.append("--hold")
    if workload_cmd:
        cmd.extend(["-e"] + workload_cmd)

    # Wrap with taskset if available
    taskset_bin = shutil.which("taskset")
    exec_cmd = ["taskset", "-c", str(core_id)] + cmd if taskset_bin else cmd

    def preexec_affinity():
        try:
            os.sched_setaffinity(0, {core_id})
        except Exception:
            pass

    print(colorize(f"\n[{label}] Launching: {' '.join(cmd)}", COLOR_BOLD))
    print(colorize(f"  Pinned to Core : CPU {core_id}", COLOR_CYAN))
    if duration:
        print(colorize(f"  Duration Limit : {duration:.1f}s", COLOR_DIM))
    print(colorize("  Sampling Rate  : Every {:.0f}ms".format(interval * 1000), COLOR_DIM))
    print(colorize("  Press Ctrl+C to conclude measurement early.\n", COLOR_DIM))

    start_wall = time.time()
    try:
        proc = subprocess.Popen(
            exec_cmd,
            preexec_fn=preexec_affinity if not taskset_bin else None,
        )
    except Exception as e:
        print(colorize(f"[X] Failed to execute {binary_path}: {e}", COLOR_RED))
        sys.exit(1)

    pid = proc.pid

    # Ensure affinity is set on the pid
    try:
        os.sched_setaffinity(pid, {core_id})
    except Exception:
        pass

    # Wait for process to initialize
    time.sleep(0.04)

    initial_stat = parse_proc_stat(pid)
    initial_status = parse_proc_status(pid)

    prev_stat = initial_stat
    prev_time = time.time()
    last_valid_stat = initial_stat
    last_valid_status = initial_status

    samples_cpu_pct: List[float] = []
    samples_rss_mb: List[float] = []
    peak_threads = 1
    final_threads_map = {}

    stopped_by_user = False
    try:
        while True:
            # Check if process exited
            if proc.poll() is not None:
                break

            now = time.time()
            elapsed_wall = now - start_wall

            # Auto-exit if duration limit reached
            if duration is not None and elapsed_wall >= duration:
                try:
                    proc.send_signal(signal.SIGINT)
                    proc.wait(timeout=0.6)
                except Exception:
                    try:
                        proc.terminate()
                        proc.wait(timeout=0.4)
                    except Exception:
                        proc.kill()
                break

            cur_stat = parse_proc_stat(pid)
            if cur_stat is None:
                break

            cur_status = parse_proc_status(pid)
            delta_wall = now - prev_time

            if prev_stat and delta_wall > 0:
                delta_ticks = (cur_stat["utime"] - prev_stat["utime"]) + (cur_stat["stime"] - prev_stat["stime"])
                delta_cpu_sec = delta_ticks / float(CLK_TCK)
                # Instantaneous CPU % on a single pinned core (max 100%)
                instant_cpu_pct = (delta_cpu_sec / delta_wall) * 100.0
                samples_cpu_pct.append(instant_cpu_pct)

            peak_threads = max(peak_threads, cur_stat["num_threads"])

            if cur_stat:
                last_valid_stat = cur_stat
            if cur_status:
                last_valid_status = cur_status
                rss_mb = cur_status["vm_rss_kb"] / 1024.0
                samples_rss_mb.append(rss_mb)

            # Periodically poll per-thread stats
            cur_threads = parse_thread_metrics(pid)
            if cur_threads:
                final_threads_map = cur_threads

            prev_stat = cur_stat
            prev_time = now

            # Live single-line status monitor
            if sys.stdout.isatty():
                cur_cpu_str = f"{samples_cpu_pct[-1]:.1f}%" if samples_cpu_pct else "0.0%"
                avg_cpu_so_far = sum(samples_cpu_pct) / len(samples_cpu_pct) if samples_cpu_pct else 0.0
                peak_cpu_so_far = max(samples_cpu_pct) if samples_cpu_pct else 0.0
                rss_str = f"{samples_rss_mb[-1]:.1f} MB" if samples_rss_mb else "-- MB"

                sys.stdout.write(
                    f"\r  {colorize('●', COLOR_GREEN)} [PID {pid}] "
                    f"CPU: {colorize(cur_cpu_str, COLOR_YELLOW)} "
                    f"(Avg: {avg_cpu_so_far:.1f}%, Peak: {peak_cpu_so_far:.1f}%) | "
                    f"Threads: {cur_stat['num_threads']} | "
                    f"RSS: {rss_str} | "
                    f"Time: {elapsed_wall:.1f}s   "
                )
                sys.stdout.flush()

            time.sleep(interval)

    except KeyboardInterrupt:
        stopped_by_user = True
        print(colorize("\n\n[*] Caught interrupt. Gracefully stopping process...", COLOR_YELLOW))
        try:
            proc.send_signal(signal.SIGINT)
            proc.wait(timeout=0.8)
        except Exception:
            try:
                proc.terminate()
                proc.wait(timeout=0.5)
            except Exception:
                proc.kill()

    end_wall = time.time()
    total_wall_duration = max(0.001, end_wall - start_wall)

    # Clear live progress line
    if sys.stdout.isatty():
        sys.stdout.write("\r" + " " * 85 + "\r")
        sys.stdout.flush()

    # Final metrics gathering
    final_stat = parse_proc_stat(pid) or last_valid_stat or prev_stat or initial_stat or {"utime": 0, "stime": 0, "num_threads": 1}
    final_status = parse_proc_status(pid) or last_valid_status or initial_status or {
        "voluntary_ctxt_switches": 0,
        "nonvoluntary_ctxt_switches": 0,
        "vm_rss_kb": 0,
        "vm_hwm_kb": 0,
    }

    init_stat = initial_stat or final_stat
    init_status = initial_status or final_status

    user_ticks = max(0, final_stat["utime"] - init_stat["utime"])
    sys_ticks = max(0, final_stat["stime"] - init_stat["stime"])
    total_ticks = user_ticks + sys_ticks

    user_cpu_sec = user_ticks / float(CLK_TCK)
    sys_cpu_sec = sys_ticks / float(CLK_TCK)
    total_cpu_sec = total_ticks / float(CLK_TCK)

    avg_cpu_pct = (total_cpu_sec / total_wall_duration) * 100.0
    peak_cpu_pct = max(samples_cpu_pct) if samples_cpu_pct else avg_cpu_pct
    min_cpu_pct = min(samples_cpu_pct) if samples_cpu_pct else 0.0

    vol_switches = max(0, final_status["voluntary_ctxt_switches"] - init_status["voluntary_ctxt_switches"])
    nonvol_switches = max(0, final_status["nonvoluntary_ctxt_switches"] - init_status["nonvoluntary_ctxt_switches"])

    peak_rss_mb = final_status["vm_hwm_kb"] / 1024.0
    if peak_rss_mb == 0.0 and samples_rss_mb:
        peak_rss_mb = max(samples_rss_mb)

    # Format per-thread report
    threads_summary = []
    for tid, tinfo in sorted(final_threads_map.items(), key=lambda kv: kv[1]["total_ticks"], reverse=True):
        t_user = tinfo["utime"] / float(CLK_TCK)
        t_sys = tinfo["stime"] / float(CLK_TCK)
        t_total = tinfo["total_ticks"] / float(CLK_TCK)
        share = min(100.0, (t_total / max(0.0001, total_cpu_sec)) * 100.0) if total_cpu_sec > 0 else 0.0
        threads_summary.append({
            "tid": tid,
            "comm": tinfo["comm"],
            "user_sec": t_user,
            "sys_sec": t_sys,
            "total_sec": t_total,
            "share_pct": share,
        })

    return {
        "label": label,
        "binary_path": binary_path,
        "core_id": core_id,
        "wall_duration_sec": total_wall_duration,
        "sample_count": len(samples_cpu_pct),
        "user_cpu_sec": user_cpu_sec,
        "sys_cpu_sec": sys_cpu_sec,
        "total_cpu_sec": total_cpu_sec,
        "avg_cpu_pct": avg_cpu_pct,
        "peak_cpu_pct": peak_cpu_pct,
        "min_cpu_pct": min_cpu_pct,
        "vol_ctxt_switches": vol_switches,
        "nonvol_ctxt_switches": nonvol_switches,
        "vol_switches_per_sec": vol_switches / total_wall_duration,
        "nonvol_switches_per_sec": nonvol_switches / total_wall_duration,
        "peak_rss_mb": peak_rss_mb,
        "peak_threads": peak_threads,
        "threads": threads_summary,
        "samples_cpu_pct": samples_cpu_pct,
    }

def print_single_summary(result: Dict[str, Any], cpu_info: Dict[str, Any]):
    """Print detailed ASCII performance summary for a single profiled binary."""
    lbl = result["label"]
    wall_dur = result["wall_duration_sec"]
    total_cpu = result["total_cpu_sec"]
    user_cpu = result["user_cpu_sec"]
    sys_cpu = result["sys_cpu_sec"]
    avg_cpu = result["avg_cpu_pct"]
    peak_cpu = result["peak_cpu_pct"]

    print("\n" + "=" * 68)
    print(colorize(f"  Single-Core CPU Performance Profile: {lbl}", COLOR_BOLD))
    print(f"  Target Binary       : {result['binary_path']}")
    print(f"  Pinned Core         : CPU {result['core_id']} ({cpu_info['model_name']})")
    print("=" * 68)

    print(f"  Wall Duration       : {wall_dur:.2f} s ({result['sample_count']} samples @ 100ms)")
    print(f"  Total CPU Time      : {colorize(f'{total_cpu:.3f} s', COLOR_GREEN)} (User: {user_cpu:.3f} s | Sys: {sys_cpu:.3f} s)")
    print(f"  Average CPU Load    : {colorize(f'{avg_cpu:.2f}%', COLOR_YELLOW)} of single core")
    print(f"  Peak CPU Load       : {colorize(f'{peak_cpu:.2f}%', COLOR_YELLOW)}")

    user_ratio = (user_cpu / max(0.0001, total_cpu)) * 100.0 if total_cpu > 0 else 0.0
    sys_ratio = (sys_cpu / max(0.0001, total_cpu)) * 100.0 if total_cpu > 0 else 0.0
    print(f"  User / Kernel Ratio : {user_ratio:.1f}% User / {sys_ratio:.1f}% Kernel")

    print("-" * 68)
    print(f"  Voluntary Switches  : {result['vol_ctxt_switches']:,} ({result['vol_switches_per_sec']:.1f} / sec)")
    print(f"  Involuntary Switches: {result['nonvol_ctxt_switches']:,} ({result['nonvol_switches_per_sec']:.1f} / sec)")
    print(f"  Peak Thread Count   : {result['peak_threads']}")
    print(f"  Peak Memory (RSS)   : {result['peak_rss_mb']:.2f} MB")

    # Per-thread distribution
    threads = result.get("threads", [])
    if threads:
        print("-" * 68)
        print(colorize("  Per-Thread CPU Distribution (Top Threads):", COLOR_BOLD))
        print("  {:<7} {:<20} {:<12} {:<12} {:<10}".format("TID", "Thread Name", "User (s)", "Sys (s)", "Share (%)"))
        print("  " + "-" * 62)
        for t in threads[:6]:
            print("  {:<7} {:<20} {:<12.3f} {:<12.3f} {:<10.1f}".format(
                t["tid"], t["comm"], t["user_sec"], t["sys_sec"], t["share_pct"]
            ))

    # Optimization Insights
    print("-" * 68)
    print(colorize("  Optimization Diagnostic Insights:", COLOR_BOLD))
    insights = []
    if avg_cpu < 1.0:
        insights.append(f"  {colorize('✔', COLOR_GREEN)} Process is highly efficient at idle/low-load (< 1.0% single-core CPU).")
    elif avg_cpu > 70.0:
        insights.append(f"  {colorize('⚠', COLOR_YELLOW)} Process is CPU-bound on single core ({avg_cpu:.1f}% load). Check hot loop throughput.")

    if total_cpu > 0 and sys_ratio > 30.0:
        insights.append(f"  {colorize('ℹ', COLOR_CYAN)} High system/kernel time ({sys_ratio:.1f}%). Check syscall frequency (read/write/epoll).")

    if result["nonvol_switches_per_sec"] > 300.0:
        insights.append(f"  {colorize('⚠', COLOR_YELLOW)} High involuntary context switch rate ({result['nonvol_switches_per_sec']:.1f}/s). Single core CPU time-slice saturated.")

    if not insights:
        insights.append(f"  {colorize('✔', COLOR_GREEN)} CPU profile parameters look balanced and healthy.")

    for ins in insights:
        print(ins)

    print("=" * 68 + "\n")

def print_comparison_table(results: List[Dict[str, Any]], cpu_info: Dict[str, Any], markdown: bool = False):
    """
    Print side-by-side comparative benchmark table across multiple builds/versions.
    Calculates delta relative to baseline (first target in list).
    """
    if not results:
        return

    baseline = results[0]
    base_cpu = max(0.001, baseline["total_cpu_sec"])
    base_avg_pct = max(0.001, baseline["avg_cpu_pct"])

    if markdown:
        print("\n### Single-Core CPU Benchmark Comparison\n")
        print(f"**Pinned Core**: CPU {baseline['core_id']} ({cpu_info['model_name']})  ")
        print(f"**Clock Ticks**: {CLK_TCK} Hz | **Sample Interval**: 100 ms\n")
        print("| Target / Build | Wall Time | Total CPU (s) | User (s) | Sys (s) | Avg CPU % | Peak CPU % | Ctxt Switches (Vol/Nonvol) | Peak RSS | CPU Diff vs Baseline |")
        print("| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |")
        for idx, r in enumerate(results):
            diff_str = "--"
            if idx > 0:
                diff_pct = ((r["total_cpu_sec"] - base_cpu) / base_cpu) * 100.0
                sign = "+" if diff_pct > 0 else ""
                diff_str = f"{sign}{diff_pct:.1f}%"
            ctxt_str = f"{r['vol_ctxt_switches']:,} / {r['nonvol_ctxt_switches']:,}"
            print(f"| **{r['label']}** | {r['wall_duration_sec']:.2f}s | {r['total_cpu_sec']:.3f}s | {r['user_cpu_sec']:.3f}s | {r['sys_cpu_sec']:.3f}s | {r['avg_cpu_pct']:.2f}% | {r['peak_cpu_pct']:.2f}% | {ctxt_str} | {r['peak_rss_mb']:.1f} MB | {diff_str} |")
        print()
        return

    lbl_w = max(18, max(len(r["label"]) for r in results))
    line_w = lbl_w + 88

    print("\n" + "=" * line_w)
    print(colorize("  Single-Core CPU Performance Comparison Across Builds", COLOR_BOLD))
    print(f"  Pinned Core: CPU {baseline['core_id']} ({cpu_info['model_name']}) | Baseline: {baseline['label']}")
    print("=" * line_w)

    fmt_row = f"{{:<{lbl_w}}} {{:<11}} {{:<13}} {{:<10}} {{:<10}} {{:<11}} {{:<15}} {{:<16}}"
    header = fmt_row.format(
        "Target / Build", "Total CPU", "User / Sys", "Avg CPU %", "Peak CPU %", "Peak RSS", "Ctxt Switches", "Delta vs Base"
    )
    print(colorize(header, COLOR_BOLD))
    print("-" * line_w)

    for idx, r in enumerate(results):
        lbl = r["label"]
        total_str = f"{r['total_cpu_sec']:.3f} s"
        user_sys_str = f"{r['user_cpu_sec']:.2f}/{r['sys_cpu_sec']:.2f}s"
        avg_str = f"{r['avg_cpu_pct']:.1f}%"
        peak_str = f"{r['peak_cpu_pct']:.1f}%"
        rss_str = f"{r['peak_rss_mb']:.1f} MB"
        ctxt_str = f"{r['vol_ctxt_switches']:,} v / {r['nonvol_ctxt_switches']:,} nv"

        diff_str = "Baseline"
        if idx > 0:
            diff_pct = ((r["total_cpu_sec"] - base_cpu) / base_cpu) * 100.0
            if diff_pct < -0.1:
                diff_str = colorize(f"{diff_pct:.1f}% (faster)", COLOR_GREEN)
            elif diff_pct > 0.1:
                diff_str = colorize(f"+{diff_pct:.1f}% (slower)", COLOR_RED)
            else:
                diff_str = "~Parity (0.0%)"

        line = fmt_row.format(
            lbl, total_str, user_sys_str, avg_str, peak_str, rss_str, ctxt_str, diff_str
        )
        print(line)

    print("=" * line_w + "\n")

def main():
    parser = argparse.ArgumentParser(
        description="Single-Core CPU Usage Profiler & Benchmark for Velox",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "targets",
        nargs="*",
        help="Path to Velox binary or cargo profile name (e.g. release, debug, optimized-release)",
    )
    parser.add_argument(
        "-c", "--core",
        type=int,
        default=0,
        help="CPU core ID to pin the process to (default: 0)",
    )
    parser.add_argument(
        "-d", "--duration",
        type=float,
        default=None,
        help="Measurement duration limit in seconds (default: runs until process exits or Ctrl+C)",
    )
    parser.add_argument(
        "-i", "--interval",
        type=float,
        default=0.1,
        help="CPU sampling interval in seconds (default: 0.1s / 100ms)",
    )
    parser.add_argument(
        "-p", "--profile",
        type=str,
        default=None,
        choices=["debug", "dev", "release", "optimized-release"],
        help="Cargo profile to profile (builds automatically if missing)",
    )
    parser.add_argument(
        "--all-profiles",
        action="store_true",
        help="Benchmark and compare all three profiles: debug, release, and optimized-release",
    )
    parser.add_argument(
        "--compare",
        nargs="+",
        metavar="TARGET",
        help="Compare multiple binaries or 'Label:path' targets sequentially on the single core",
    )
    parser.add_argument(
        "-w", "--workload",
        type=str,
        default=None,
        choices=["idle", "stream-ascii", "stream-ansi", "scroll"],
        help="Standardized workload to run inside Velox (idle, stream-ascii, stream-ansi, scroll)",
    )
    parser.add_argument(
        "-e", "--command",
        type=str,
        default=None,
        help="Custom command string to execute inside Velox via -e (e.g. 'htop' or 'python3 script.py')",
    )
    parser.add_argument(
        "--hold",
        action="store_true",
        help="Keep Velox window open after command exits",
    )
    parser.add_argument(
        "--markdown",
        action="store_true",
        help="Output comparison results formatted as a Markdown table",
    )
    parser.add_argument(
        "--json",
        type=str,
        default=None,
        metavar="FILE",
        help="Export raw measurement results and summary to specified JSON file",
    )

    args = parser.parse_args()

    cpu_info = get_cpu_info(args.core)

    print("=" * 68)
    print(colorize("  Velox Single-Core CPU Profiler & Benchmark Engine", COLOR_BOLD))
    print(f"  CPU Hardware    : {cpu_info['model_name']}")
    print(f"  Selected Core   : CPU {args.core} (out of {cpu_info['total_cores']} cores)")
    if cpu_info["mhz"] > 0:
        print(f"  Clock Frequency : {cpu_info['mhz']:.1f} MHz")
    print(f"  Clock Ticks     : {CLK_TCK} Hz")
    print("=" * 68)

    # Determine list of targets to benchmark
    target_list: List[Tuple[str, str]] = []

    if args.all_profiles:
        for prof in ["debug", "release", "optimized-release"]:
            label, path = resolve_target_binary(prof)
            target_list.append((label, path))
    elif args.compare:
        for t in args.compare:
            label, path = resolve_target_binary(t)
            target_list.append((label, path))
    elif args.profile:
        label, path = resolve_target_binary(args.profile)
        target_list.append((label, path))
    elif args.targets:
        for t in args.targets:
            label, path = resolve_target_binary(t)
            target_list.append((label, path))
    else:
        # Default binary fallback: optimized-release > release > debug
        default_profile = "release"
        if os.path.exists("target/optimized-release/velox"):
            default_profile = "optimized-release"
        label, path = resolve_target_binary(default_profile)
        target_list.append((label, path))

    # Resolve workload command
    workload_cmd: Optional[List[str]] = None
    if args.workload:
        workload_cmd = get_workload_command(args.workload, args.duration)
        if args.duration is None and args.workload == "idle":
            args.duration = 5.0
    elif args.command:
        workload_cmd = ["bash", "-c", args.command]

    # Run profiling sessions
    results = []
    for label, bin_path in target_list:
        res = run_single_profile(
            binary_path=bin_path,
            label=label,
            core_id=args.core,
            workload_cmd=workload_cmd,
            duration=args.duration,
            interval=args.interval,
            hold=args.hold,
        )
        results.append(res)
        print_single_summary(res, cpu_info)

    # If multiple targets, print comparative summary
    if len(results) > 1:
        print_comparison_table(results, cpu_info, markdown=args.markdown)

    # JSON Export if requested
    if args.json:
        try:
            export_data = {
                "system_info": cpu_info,
                "benchmarked_core": args.core,
                "timestamp": time.time(),
                "results": results,
            }
            with open(args.json, "w", encoding="utf-8") as f:
                json.dump(export_data, f, indent=2)
            print(colorize(f"[+] Exported benchmark metrics to '{args.json}'.", COLOR_GREEN))
        except Exception as e:
            print(colorize(f"[X] Failed to export JSON: {e}", COLOR_RED))

if __name__ == "__main__":
    main()
