use colored::Colorize;
use hard_sync_core::{list_connected_drives, list_pairs, DeleteBehavior, SourceSide};

use crate::output::{format_size, print_err};

pub fn list_callback(_data: &fli::command::FliCallbackData) {
    match list_pairs() {
        Err(e) => { print_err(&e); return; }
        Ok(pairs) if pairs.is_empty() => {
            println!("{}", "No sync pairs configured. Run `hsync init` to add one.".dimmed());
            return;
        }
        Ok(pairs) => {
            println!("Sync pairs ({})\n", pairs.len());
            for pair in &pairs {
                println!("  {}", pair.name.cyan().bold());
                println!("    base:    {}", pair.base.display());
                println!("    target:  {}", pair.target.display());
                println!("    source:  {}", match pair.source {
                    SourceSide::Base   => "base",
                    SourceSide::Target => "target",
                });
                match &pair.drive_id {
                    Some(id) => {
                        let label = id.label.as_deref().unwrap_or("unknown");
                        let uuid  = id.uuid.as_deref().unwrap_or("—");
                        println!("    drive:   {} ({})", label.yellow(), uuid);
                    }
                    None => println!("    drive:   {}", "same drive".dimmed()),
                }
                println!("    delete:  {}", match pair.delete_behavior {
                    DeleteBehavior::Trash  => "trash",
                    DeleteBehavior::Delete => "delete",
                    DeleteBehavior::Ignore => "ignore",
                });
                println!();
            }
        }
    }
}

pub fn drives_callback(_data: &fli::command::FliCallbackData) {
    let drives = list_connected_drives();
    if drives.is_empty() {
        println!("{}", "No drives detected.".dimmed());
        return;
    }

    // Also load configured pairs to show matches
    let pairs = list_pairs().unwrap_or_default();

    println!("Connected drives ({})\n", drives.len());
    for drive in &drives {
        let name = if drive.name.is_empty() { "unnamed".to_string() } else { drive.name.clone() };
        let removable = if drive.is_removable { " [removable]".yellow().to_string() } else { String::new() };
        let used = drive.total_space.saturating_sub(drive.available_space);
        let size_info = format!("{} / {}", format_size(used), format_size(drive.total_space));

        // Check if any configured pair targets this drive
        let pair_match = pairs.iter().find(|p| {
            p.drive_id.as_ref().and_then(|id| id.label.as_deref()) == Some(name.as_str())
        });
        let pair_tag = match pair_match {
            Some(p) => format!(" — matches pair \"{}\"", p.name).green().to_string(),
            None    => String::new(),
        };

        println!("  {}  {}{}{}",
            drive.mount_point.display().to_string().cyan(),
            name.bold(),
            removable,
            pair_tag,
        );
        println!("    {}", size_info.dimmed());
        println!();
    }
}
