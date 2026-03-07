use fli::{init_fli_from_toml, option_parser::{Value, ValueTypes}};

fn main() {
    let mut app = init_fli_from_toml!();

    // hsync init
    let init = app.command("init", "Set up a new sync pair").unwrap();
    init.add_option("base", "Local/primary path", "-b", "--base", ValueTypes::RequiredSingle(Value::Str(String::new())));
    init.add_option("target", "Other path (often a removable drive)", "-t", "--target", ValueTypes::RequiredSingle(Value::Str(String::new())));
    init.add_option("name", "Name for this pair", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())));
    init.add_option("source", "Which side is truth: base or target (default: base)", "-s", "--source", ValueTypes::OptionalSingle(Some(Value::Str("base".to_string()))));
    init.set_callback(|_data| {
        // TODO: wire to core::init_pair
        eprintln!("init: not yet implemented");
    });

    // hsync sync
    let sync = app.command("sync", "One-shot sync for a named pair").unwrap();
    sync.add_option("name", "Name of the pair to sync", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())));
    sync.add_option("dry-run", "Show what would happen without touching files", "-d", "--dry-run", ValueTypes::OptionalSingle(Some(Value::Bool(false))));
    sync.add_option("verify", "Use SHA256 checksum comparison instead of mtime+size", "-v", "--verify", ValueTypes::OptionalSingle(Some(Value::Bool(false))));
    sync.set_callback(|_data| {
        // TODO: wire to core::sync_pair
        eprintln!("sync: not yet implemented");
    });

    // hsync watch
    let watch = app.command("watch", "Auto-sync when drive is detected and files change").unwrap();
    watch.add_option("name", "Name of the pair to watch", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())));
    watch.set_callback(|_data| {
        // TODO: wire to core::watch_pair
        eprintln!("watch: not yet implemented");
    });

    // hsync set-source
    let set_source = app.command("set-source", "Flip which side is the source of truth").unwrap();
    set_source.add_option("name", "Pair name", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())));
    set_source.add_option("source", "New source side: base or target", "-s", "--source", ValueTypes::RequiredSingle(Value::Str(String::new())));
    set_source.set_callback(|_data| {
        // TODO: wire to core::set_source
        eprintln!("set-source: not yet implemented");
    });

    // hsync list
    let list = app.command("list", "List all configured sync pairs").unwrap();
    list.set_callback(|_data| {
        // TODO: wire to core::list_pairs
        eprintln!("list: not yet implemented");
    });

    // hsync remove
    let remove = app.command("remove", "Remove a named sync pair from config").unwrap();
    remove.add_option("name", "Name of the pair to remove", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())));
    remove.set_callback(|_data| {
        // TODO: wire to core::remove_pair
        eprintln!("remove: not yet implemented");
    });

    // hsync trash
    let trash = app.command("trash", "Manage the trash folder for a sync pair").unwrap();

    // hsync trash list
    trash.subcommand("list", "Show what's in the trash for a pair")
        .add_option("name", "Pair name", "-n", "--name", ValueTypes::RequiredSingle(Value::Str(String::new())))
        .set_callback(|_data| {
            // TODO: wire to core::list_trash
            eprintln!("trash list: not yet implemented");
        });

    // hsync trash clear
    trash.subcommand("clear", "Delete everything in the trash for a pair or all pairs")
        .add_option("name", "Pair name", "-n", "--name", ValueTypes::OptionalSingle(None))
        .add_option("all", "Clear trash for all pairs", "-a", "--all", ValueTypes::OptionalSingle(Some(Value::Bool(false))))
        .set_callback(|_data| {
            // TODO: wire to core::clear_trash
            eprintln!("trash clear: not yet implemented");
        });

    app.run();
}
