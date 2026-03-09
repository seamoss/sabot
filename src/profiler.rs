// Sabot execution profiler
// Tracks per-word call counts, cumulative time, and opcode execution stats.

use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct Profiler {
    /// Per-word stats
    word_stats: HashMap<String, WordStats>,
    /// Stack of (word_name, start_time, self_time_so_far) for nested timing
    call_stack: Vec<CallEntry>,
    /// Total opcodes executed
    pub total_ops: u64,
    /// Total wall time
    pub start_time: Option<Instant>,
}

struct CallEntry {
    name: String,
    start: Instant,
    child_time: Duration, // time spent in callees
}

#[derive(Default, Clone)]
struct WordStats {
    calls: u64,
    total_time: Duration,
    self_time: Duration,
}

impl Profiler {
    pub fn new() -> Self {
        Profiler {
            word_stats: HashMap::new(),
            call_stack: Vec::new(),
            total_ops: 0,
            start_time: Some(Instant::now()),
        }
    }

    /// Called when entering a word (frame push or builtin call)
    pub fn enter_word(&mut self, name: &str) {
        self.call_stack.push(CallEntry {
            name: name.to_string(),
            start: Instant::now(),
            child_time: Duration::ZERO,
        });
    }

    /// Called when leaving a word (frame pop or builtin return)
    pub fn exit_word(&mut self) {
        if let Some(entry) = self.call_stack.pop() {
            let elapsed = entry.start.elapsed();
            let self_time = elapsed.saturating_sub(entry.child_time);

            let stats = self.word_stats.entry(entry.name).or_default();
            stats.calls += 1;
            stats.total_time += elapsed;
            stats.self_time += self_time;

            // Attribute our time to parent's child_time
            if let Some(parent) = self.call_stack.last_mut() {
                parent.child_time += elapsed;
            }
        }
    }

    /// Increment opcode counter
    pub fn tick(&mut self) {
        self.total_ops += 1;
    }

    /// Print the profiling report
    pub fn report(&self) {
        let wall_time = self.start_time.map(|s| s.elapsed()).unwrap_or_default();

        println!();
        println!("=== Profile Report ===");
        println!();
        println!("Wall time:   {:.1}ms", wall_time.as_secs_f64() * 1000.0);
        println!("Total ops:   {}", format_num(self.total_ops));
        if wall_time.as_nanos() > 0 {
            let ops_per_sec = self.total_ops as f64 / wall_time.as_secs_f64();
            println!("Throughput:  {} ops/sec", format_num(ops_per_sec as u64));
        }
        println!();

        if self.word_stats.is_empty() {
            println!("(no word calls recorded)");
            return;
        }

        // Sort by total time descending
        let mut entries: Vec<(&String, &WordStats)> = self.word_stats.iter().collect();
        entries.sort_by(|a, b| b.1.total_time.cmp(&a.1.total_time));

        // Table header
        println!(
            "{:<25} {:>10} {:>10} {:>10} {:>10}",
            "Word", "Calls", "Total(ms)", "Self(ms)", "Avg(μs)"
        );
        println!("{}", "-".repeat(70));

        let top_n = 30;
        for (name, stats) in entries.iter().take(top_n) {
            let total_ms = stats.total_time.as_secs_f64() * 1000.0;
            let self_ms = stats.self_time.as_secs_f64() * 1000.0;
            let avg_us = if stats.calls > 0 {
                stats.total_time.as_secs_f64() * 1_000_000.0 / stats.calls as f64
            } else {
                0.0
            };

            println!(
                "{:<25} {:>10} {:>10.2} {:>10.2} {:>10.1}",
                truncate(name, 25),
                format_num(stats.calls),
                total_ms,
                self_ms,
                avg_us,
            );
        }

        if entries.len() > top_n {
            println!("  ... and {} more words", entries.len() - top_n);
        }

        // Summary: hottest by self time
        let mut by_self: Vec<(&String, &WordStats)> = self.word_stats.iter().collect();
        by_self.sort_by(|a, b| b.1.self_time.cmp(&a.1.self_time));

        println!();
        println!("Hottest by self time:");
        for (name, stats) in by_self.iter().take(5) {
            let pct = if wall_time.as_nanos() > 0 {
                stats.self_time.as_secs_f64() / wall_time.as_secs_f64() * 100.0
            } else {
                0.0
            };
            println!(
                "  {:<25} {:>8.2}ms  ({:.1}%)",
                name,
                stats.self_time.as_secs_f64() * 1000.0,
                pct,
            );
        }
        println!();
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

fn format_num(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}
