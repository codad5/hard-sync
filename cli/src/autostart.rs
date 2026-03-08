use auto_launch::AutoLaunchBuilder;
use colored::Colorize;
use hard_sync_core::list_pairs;

use crate::output::{print_err, require_name};

// Each autostart entry is named "hsync-watch-<pairname>" so multiple pairs
// can each have their own startup entry.
fn make_builder(name: &str) -> Result<AutoLaunchBuilder, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Cannot get exe path: {}", e))?;
    let exe_str = exe.to_string_lossy().to_string();
    let app_name = format!("hsync-watch-{}", name);
    let args = ["watch", "--name", name, "--detach"];

    let mut builder = AutoLaunchBuilder::new();
    builder
        .set_app_name(&app_name)
        .set_app_path(&exe_str)
        .set_args(&args);
    Ok(builder)
}

pub fn autostart_enable_callback(data: &fli::command::FliCallbackData) {
    let name = match require_name(data) { Some(n) => n, None => return };

    let al = match make_builder(&name).and_then(|b| {
        b.build().map_err(|e| format!("Failed to configure autostart: {}", e))
    }) {
        Ok(a) => a,
        Err(e) => { print_err(&e); return; }
    };

    match al.enable() {
        Ok(_) => println!(
            "Autostart {} for pair {}. The watcher will start in the background on next login.",
            "enabled".green().bold(),
            format!("\"{}\"", name).cyan().bold()
        ),
        Err(e) => print_err(&format!("Failed to enable autostart: {}", e)),
    }
}

pub fn autostart_disable_callback(data: &fli::command::FliCallbackData) {
    let name = match require_name(data) { Some(n) => n, None => return };

    let al = match make_builder(&name).and_then(|b| {
        b.build().map_err(|e| format!("Failed to configure autostart: {}", e))
    }) {
        Ok(a) => a,
        Err(e) => { print_err(&e); return; }
    };

    match al.disable() {
        Ok(_) => println!(
            "Autostart {} for pair {}.",
            "disabled".yellow().bold(),
            format!("\"{}\"", name).cyan().bold()
        ),
        Err(e) => print_err(&format!("Failed to disable autostart: {}", e)),
    }
}

pub fn autostart_list_callback(_data: &fli::command::FliCallbackData) {
    let pairs = match list_pairs() {
        Ok(p) => p,
        Err(e) => { print_err(&e); return; }
    };

    if pairs.is_empty() {
        println!("{}", "No sync pairs configured. Run `hsync init` to set one up.".dimmed());
        return;
    }

    let mut any = false;
    for pair in &pairs {
        let al = match make_builder(&pair.name).and_then(|b| {
            b.build().map_err(|e| format!("auto-launch error: {}", e))
        }) {
            Ok(a) => a,
            Err(_) => continue,
        };
        let enabled = al.is_enabled().unwrap_or(false);
        let marker = if enabled { "●".green() } else { "○".dimmed() };
        let status = if enabled { "autostart on".green() } else { "autostart off".dimmed() };
        println!("  {} {}  {}", marker, pair.name.cyan().bold(), status);
        any = true;
    }

    if !any {
        println!("{}", "No pairs found.".dimmed());
    }
}
