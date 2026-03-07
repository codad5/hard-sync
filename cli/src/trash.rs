use colored::Colorize;
use hard_sync_core::clear_trash;
use hard_sync_core::list_trash;

use crate::output::{format_size, print_err, require_name};

pub fn trash_list_callback(data: &fli::command::FliCallbackData) {
    let name = match require_name(data) {
        Some(n) => n,
        None => return,
    };

    match list_trash(&name) {
        Err(e) => print_err(&e),
        Ok(entries) if entries.is_empty() => {
            println!(
                "Trash is empty for pair {}.",
                format!("\"{}\"", name).cyan().bold()
            );
        }
        Ok(entries) => {
            println!(
                "Trash for pair {} ({} file{}):",
                format!("\"{}\"", name).cyan().bold(),
                entries.len(),
                if entries.len() == 1 { "" } else { "s" }
            );
            for entry in &entries {
                println!(
                    "  {}  {}  {}",
                    entry
                        .trashed_at
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                        .dimmed(),
                    format_size(entry.size).yellow(),
                    entry.original_name,
                );
            }
        }
    }
}

pub fn trash_clear_callback(data: &fli::command::FliCallbackData) {
    use fli::option_parser::{Value, ValueTypes};

    let all = matches!(
        data.get_option_value("all"),
        Some(ValueTypes::OptionalSingle(Some(Value::Bool(true))))
    );

    let name: Option<String> = data
        .get_option_value("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    if !all && name.is_none() {
        print_err("Provide --name <pair> or --all to clear trash.");
        return;
    }

    let target = name.as_deref();
    match clear_trash(target) {
        Ok(_) => {
            if all {
                println!("Trash cleared for all pairs.");
            } else {
                println!(
                    "Trash cleared for pair {}.",
                    format!("\"{}\"", name.unwrap()).cyan().bold()
                );
            }
        }
        Err(e) => print_err(&e),
    }
}
