use colored::Colorize;
use hard_sync_core::remove_pair;

use crate::output::{print_err, require_name};

pub fn remove_callback(data: &fli::command::FliCallbackData) {
    let name = match require_name(data) { Some(n) => n, None => return };

    match remove_pair(&name) {
        Ok(_)  => println!("Pair {} removed.", format!("\"{}\"", name).cyan().bold()),
        Err(e) => print_err(&e),
    }
}
