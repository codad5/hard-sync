use colored::Colorize;

pub fn print(message: &str) {
    println!("{}", message);
}

pub fn print_error(message: &str) {
    print( format!("Error: {}", message.red()).as_str());
}

pub fn print_info(message: &str) {
    print(format!("Info: {}", message.blue()).as_str());
}

pub fn print_success(message: &str) {
    print(format!("Success: {}", message.green()).as_str());
}

pub fn print_warning(message: &str) {
    print( format!("Warning: {}", message.yellow()).as_str());
}

pub fn step_message(step: &str, message: &str) {
    println!("{}", format!("Step: {}", message).cyan());
}