use colored::Colorize;
use hard_sync_core::{SyncOutcome, SyncReport};

// ── Report printing ───────────────────────────────────────────────────────────

const MAX_FILE_LINES: usize = 10;

pub fn print_sync_report(report: &SyncReport, dry_run: bool) {
    if dry_run {
        println!("{}", "[DRY RUN] No files will be modified.".dimmed());
    }

    // Show individual operations (cap at MAX_FILE_LINES, summarise the rest)
    let shown: Vec<_> = report.ops.iter().filter(|op| !matches!(op.outcome, SyncOutcome::Skipped)).collect();
    let display_count = shown.len().min(MAX_FILE_LINES);

    for op in &shown[..display_count] {
        let (symbol, label, color_fn): (&str, &str, fn(&str) -> colored::ColoredString) = match op.outcome {
            SyncOutcome::Copied  => ("+", "copied ", |s: &str| s.green()),
            SyncOutcome::Updated => ("~", "updated", |s: &str| s.yellow()),
            SyncOutcome::Trashed => ("-", "trashed", |s: &str| s.red()),
            SyncOutcome::Deleted => ("-", "deleted", |s: &str| s.red()),
            SyncOutcome::Ignored => ("·", "ignored", |s: &str| s.dimmed()),
            SyncOutcome::Skipped => ("·", "skipped", |s: &str| s.dimmed()),
        };
        println!("  {} {}  {}", color_fn(symbol), label.dimmed(), op.rel_path);
    }

    if shown.len() > MAX_FILE_LINES {
        let rest = shown.len() - MAX_FILE_LINES;
        let skipped_total = report.skipped;
        println!("  {} {} more file{}, {} skipped",
            "·".dimmed(),
            rest,
            if rest == 1 { "" } else { "s" },
            skipped_total,
        );
    } else if report.skipped > 0 && shown.is_empty() {
        println!("  {}", format!("· {} file{} skipped (up to date)",
            report.skipped,
            if report.skipped == 1 { "" } else { "s" }
        ).dimmed());
    }

    for err in &report.errors {
        println!("  {} {}  {} — {}",
            "!".bright_red(),
            "error  ".dimmed(),
            err.path,
            err.message.bright_red(),
        );
    }

    println!();
    println!("Done.  {}  {}  {}  {}  {}",
        format!("{} copied",   report.copied).green(),
        format!("{} updated",  report.updated).yellow(),
        format!("{} trashed",  report.trashed).red(),
        format!("{} skipped",  report.skipped).dimmed(),
        if report.errors.is_empty() {
            "0 errors".dimmed()
        } else {
            format!("{} errors", report.errors.len()).bright_red()
        },
    );
}

// ── Byte size formatting ──────────────────────────────────────────────────────

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// ── Error / success helpers ───────────────────────────────────────────────────

pub fn print_err(msg: &str) {
    eprintln!("{} {}", "Error:".bright_red().bold(), msg);
}

pub fn require_str(data: &fli::command::FliCallbackData, flag: &str) -> Option<String> {
    match data.get_option_value(flag).and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => Some(s.to_string()),
        _ => {
            print_err(&format!("--{} is required", flag));
            None
        }
    }
}

pub fn require_name(data: &fli::command::FliCallbackData) -> Option<String> {
    require_str(data, "name")
}
