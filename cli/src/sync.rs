use colored::Colorize;
use fli::option_parser::{Value, ValueTypes};
use hard_sync_core::{sync_pair, SyncOptions};

use crate::output::{print_err, print_sync_report, require_name};

pub fn sync_callback(data: &fli::command::FliCallbackData) {
    let name = match require_name(data) { Some(n) => n, None => return };

    let dry_run = matches!(data.get_option_value("dry-run"), Some(ValueTypes::OptionalSingle(Some(Value::Bool(true)))));
    let verify  = matches!(data.get_option_value("verify"),  Some(ValueTypes::OptionalSingle(Some(Value::Bool(true)))));

    println!("Syncing {}...", format!("\"{}\"", name).cyan().bold());

    match sync_pair(&name, SyncOptions { dry_run, verify }) {
        Ok(report) => print_sync_report(&report, dry_run),
        Err(e)     => print_err(&e),
    }
}
