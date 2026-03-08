use chrono::Utc;
use colored::Colorize;
use hard_sync_core::{
    add_pair, get_drive_id, same_drive, DeleteBehavior, PairConfig, SoundConfig, SourceSide,
};

use crate::output::{print_err, require_str};

pub fn init_callback(data: &fli::command::FliCallbackData) {
    let base       = match require_str(data, "base")   { Some(s) => s, None => return };
    let target     = match require_str(data, "target") { Some(s) => s, None => return };
    let name       = match require_str(data, "name")   { Some(s) => s, None => return };
    let source_str = data.get_option_value("source").and_then(|v| v.as_str()).unwrap_or("base").to_string();

    let base_path = std::path::PathBuf::from(&base);
    let target_path = std::path::PathBuf::from(&target);

    if !base_path.exists() {
        print_err(&format!("Base path does not exist: {}", base));
        return;
    }
    if !target_path.exists() {
        print_err(&format!("Target path does not exist: {}", target));
        return;
    }

    let source = match source_str.as_str() {
        "base"   => SourceSide::Base,
        "target" => SourceSide::Target,
        other    => { print_err(&format!("Invalid --source value: \"{}\". Use \"base\" or \"target\"", other)); return; }
    };

    // Auto-detect drive type
    let drive_id = if same_drive(&base_path, &target_path) {
        None
    } else {
        get_drive_id(&target_path)
    };

    let pair = PairConfig {
        name: name.clone(),
        base: base_path.clone(),
        target: target_path.clone(),
        source,
        drive_id: drive_id.clone(),
        ignore: vec![],
        delete_behavior: DeleteBehavior::Trash,
        sounds: SoundConfig::default(),
        created_at: Utc::now(),
    };

    match add_pair(pair) {
        Ok(_) => {
            println!("Pair {} initialized.", format!("\"{}\"", name).cyan().bold());
            println!("  base:    {}", base_path.display());
            println!("  target:  {}", target_path.display());
            println!("  source:  {}", source_str);
            match &drive_id {
                Some(id) => {
                    let label = id.label.as_deref().unwrap_or("unknown");
                    let uuid  = id.uuid.as_deref().unwrap_or("—");
                    println!("  drive:   {} (uuid: {})", label.yellow(), uuid);
                }
                None => println!("  drive:   {}", "same drive (no detection needed)".dimmed()),
            }
            println!("  delete:  trash");
        }
        Err(e) => print_err(&e),
    }
}
