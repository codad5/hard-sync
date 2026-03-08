use fli::{init_fli_from_toml, option_parser::{Value, ValueTypes}};

mod output;
mod init;
mod sync;
mod watch;
mod list;
mod remove;
mod trash;
mod config;
mod daemon;
mod autostart;

use init::init_callback;
use sync::sync_callback;
use watch::watch_callback;
use list::{list_callback, drives_callback};
use remove::remove_callback;
use trash::{trash_list_callback, trash_clear_callback};
use config::{config_path_callback, config_reset_callback};
use daemon::{watch_list_callback, watch_attach_callback, watch_stop_callback};
use autostart::{autostart_enable_callback, autostart_disable_callback, autostart_list_callback};

fn set_source_callback(data: &fli::command::FliCallbackData) {
    use colored::Colorize;
    use output::{print_err, require_str};

    let name       = match require_str(data, "name")   { Some(s) => s, None => return };
    let source_str = match require_str(data, "source") { Some(s) => s, None => return };
    let source = match source_str.as_str() {
        "base"   => hard_sync_core::SourceSide::Base,
        "target" => hard_sync_core::SourceSide::Target,
        other    => { print_err(&format!("Invalid --source: \"{}\". Use \"base\" or \"target\"", other)); return; }
    };

    match hard_sync_core::set_source(&name, source) {
        Ok(_) => println!(
            "Source for pair {} set to {}.",
            format!("\"{}\"", name).cyan().bold(),
            source_str.yellow()
        ),
        Err(e) => print_err(&e),
    }
}

fn main() {
    let mut app = init_fli_from_toml!();

    // hsync init
    let init = app.command("init", "Set up a new sync pair").unwrap();
    init.add_option("base", "Local/primary path", "-b", "--base", ValueTypes::RequiredSingle(Value::Str(String::new())));
    init.add_option("target", "Other path (often a removable drive)", "-t", "--target", ValueTypes::RequiredSingle(Value::Str(String::new())));
    init.add_option("name", "Name for this pair", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())));
    init.add_option("source", "Which side is truth: base or target (default: base)", "-s", "--source", ValueTypes::OptionalSingle(Some(Value::Str("base".to_string()))));
    init.set_callback(init_callback);

    // hsync sync
    let sync = app.command("sync", "One-shot sync for a named pair").unwrap();
    sync.add_option("name", "Name of the pair to sync", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())));
    sync.add_option("dry-run", "Show what would happen without touching files", "-d", "--dry-run", ValueTypes::OptionalSingle(Some(Value::Bool(false))));
    sync.add_option("verify", "Use SHA256 checksum comparison instead of mtime+size", "-v", "--verify", ValueTypes::OptionalSingle(Some(Value::Bool(false))));
    sync.set_callback(sync_callback);

    // hsync watch  (foreground or --detach background)
    let watch = app.command("watch", "Auto-sync when drive is detected and files change").unwrap();
    watch.add_option("name", "Name of the pair to watch", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())));
    watch.add_option("detach", "Run in the background (write PID + log file)", "-d", "--detach", ValueTypes::OptionalSingle(Some(Value::Bool(false))));
    watch.set_callback(watch_callback);

    // hsync watch list
    watch.subcommand("list", "Show all running background watchers")
        .set_callback(watch_list_callback);

    // hsync watch attach
    watch.subcommand("attach", "Tail the log of a running background watcher")
        .add_option("name", "Pair name", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())))
        .set_callback(watch_attach_callback);

    // hsync watch stop
    watch.subcommand("stop", "Stop a background watcher")
        .add_option("name", "Pair name", "-n", "--name", ValueTypes::OptionalSingle(None))
        .add_option("all", "Stop all running background watchers", "-a", "--all", ValueTypes::OptionalSingle(Some(Value::Bool(false))))
        .set_callback(watch_stop_callback);

    // hsync set-source
    let set_source = app.command("set-source", "Flip which side is the source of truth").unwrap();
    set_source.add_option("name", "Pair name", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())));
    set_source.add_option("source", "New source side: base or target", "-s", "--source", ValueTypes::RequiredSingle(Value::Str(String::new())));
    set_source.set_callback(set_source_callback);

    // hsync list
    let list = app.command("list", "List all configured sync pairs").unwrap();
    list.set_callback(list_callback);

    // hsync drives
    let drives = app.command("drives", "List connected drives, annotated with any matching pairs").unwrap();
    drives.set_callback(drives_callback);

    // hsync remove
    let remove = app.command("remove", "Remove a named sync pair from config").unwrap();
    remove.add_option("name", "Name of the pair to remove", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())));
    remove.set_callback(remove_callback);

    // hsync config
    let cfg = app.command("config", "Manage hard-sync configuration").unwrap();

    // hsync config path
    cfg.subcommand("path", "Print the path to the config file")
        .set_callback(config_path_callback);

    // hsync config reset
    cfg.subcommand("reset", "Delete the config file and remove all pairs")
        .set_callback(config_reset_callback);

    // hsync trash
    let trash = app.command("trash", "Manage the trash folder for a sync pair").unwrap();

    // hsync trash list
    trash.subcommand("list", "Show what's in the trash for a pair")
        .add_option("name", "Pair name", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())))
        .set_callback(trash_list_callback);

    // hsync trash clear
    trash.subcommand("clear", "Delete everything in the trash for a pair or all pairs")
        .add_option("name", "Pair name", "-n", "--name", ValueTypes::OptionalSingle(None))
        .add_option("all", "Clear trash for all pairs", "-a", "--all", ValueTypes::OptionalSingle(Some(Value::Bool(false))))
        .set_callback(trash_clear_callback);

    // hsync autostart
    let autostart = app.command("autostart", "Manage auto-launch of watchers on login").unwrap();

    // hsync autostart enable
    autostart.subcommand("enable", "Register a watcher to start automatically on login")
        .add_option("name", "Pair name", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())))
        .set_callback(autostart_enable_callback);

    // hsync autostart disable
    autostart.subcommand("disable", "Remove a watcher from login autostart")
        .add_option("name", "Pair name", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())))
        .set_callback(autostart_disable_callback);

    // hsync autostart list
    autostart.subcommand("list", "Show autostart status for all pairs")
        .set_callback(autostart_list_callback);

    app.run();
}
