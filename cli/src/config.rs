use colored::Colorize;
use hard_sync_core::{get_config_path, reset_config};

use crate::output::print_err;

pub fn config_path_callback(_data: &fli::command::FliCallbackData) {
    match get_config_path() {
        Ok(path) => println!("{}", path.display()),
        Err(e)   => print_err(&e),
    }
}

pub fn config_reset_callback(_data: &fli::command::FliCallbackData) {
    match reset_config() {
        Ok(_) => println!("{}", "Config reset. All pairs removed.".yellow()),
        Err(e) => print_err(&e),
    }
}
