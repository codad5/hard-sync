mod helpers;
mod controllers;
use std::{fs, path::Path};

use colored::Colorize;
use controllers::dir_tracker::DirTracker;
use fli::{init_fli_from_toml, Fli};
use helpers::logger::{print_error, print_info, print_success};

// hard sync cli a cli tool for syncing 2 directories similar to rsync but with a few more features
fn main() {
    println!("{}", "Hard Sync CLI".cyan());
    let mut app = init_fli_from_toml!();
    let sync = app.command("sync", "Syncs 2 directories");
    sync.option("-s --src, <>", "Source Directory", sync_callback);
    sync.option("-d --dest, <>", "Destination Directory", sync_callback);
    sync.option("-i --init", "Initialize the directory", sync_callback);
    sync.option("-r --reverse", "Initialize the directory", sync_callback);
    sync.option("-dr --dry-run", "Dry run", sync_callback);
    sync.option("-e --exclude, <...>", "Exclude files", sync_callback);
    sync.allow_duplicate_callback(false);

    app.run();
}

fn sync_callback(x: &Fli) {
    // get the src and dest path
    let src = match x.get_values("src".to_string()) {
        Ok(v) => v.first().unwrap().clone(),
        Err(e) => {
            print_error(&e);
            return;
        }
    };

    let dest = match x.get_values("dest".to_string()) {
        Ok(v) => v.first().unwrap().clone(),
        Err(e) => {
            print_error(format!("Destination directory not provided: {}", e).as_str());
            return;
        }
    };

    // get path of src and dest as Path and check if reverse is passed
    let (src, dest) = match !x.is_passed("reverse".to_owned()) {
                true => (Path::new(&src), Path::new(&dest)),
                false => (Path::new(&dest), Path::new(&src))
    };
    
    // check if src and dest exists
    if !src.exists() {
        print_error(format!("Source directory {:?} does not exist", src).as_str());
        return;
    }

    if !dest.exists() {
        print_error("Destination directory does not exist");
        return;
    }
    

    // check if path are same 
    if src == dest {
        print_error("Source and destination directories are same");
        return;
    }
    

    let mut src_dir = DirTracker::new(src).unwrap();
    let mut dest_dir = DirTracker::new(dest).unwrap();

    if dest_dir.dir_initialized().is_err() {
        if !x.is_passed("-i".to_owned()) {
            print_error("Dest directory is not initialized pass -i to initailise it.");
            return;
        }
        match dest_dir.setup_dir_config() {
            Ok(_) => {
                print_success("Source directory initialized");
            }
            Err(e) => {
                print_error(&e);
                return;
            }
        }
    }

    src_dir.import_files_from_directory(true);
    dest_dir.import_files_from_directory(true);

    if x.is_passed("-e".to_owned()) {
        let exclude = x.get_values("exclude".to_owned()).unwrap();
        for file in exclude {
            dest_dir.add_ignore(file);
        }
    }

    // get the diff (files that are in src but not in dest)
    let diff = src_dir.get_dir_diff(&dest_dir);
    if diff.len() == 0 {
        print_success("No diff found");
    }
    
    // warn about dry running turned on
    if x.is_passed("-dr".to_owned()) {
        print_info("Dry run turned on");
    }
    let mut ignore_count = 0;
    for file in &diff {
        let status = match dest_dir.get_file(file.get_relative_path(Path::new(&src_dir.get_path()))) {
            Some(_) => "Modified".yellow(),
            None => "New".green(),
        };
        println!(
            "{} ({})",
            file.get_relative_path(Path::new(&src_dir.get_path())),
            status.underline()
        );
        // copy the file
        let dest_file = dest.join(file.get_relative_path(Path::new(&src_dir.get_path())));
        if dest_dir.load_ignore().is_ok() && dest_dir.is_ignored( &file.get_relative_path(Path::new(&src_dir.get_path()))) {
            ignore_count+=1;
            continue;
        }
        // to implement dry run
        if !x.is_passed("-dr".to_owned()) {
            if !dest_file.parent().unwrap().exists() {
                fs::create_dir_all(dest_file.parent().unwrap()).unwrap();
            }
            fs::copy(file.get_path(), dest_file.clone()).unwrap();
        }
    }
    println!("");
    print_success(format!("{} files copied", format!("{}", diff.len() - ignore_count).blue()).as_str());
    print_success(format!("{} files ignored", format!("{}", ignore_count).red()).as_str());
    print_info(format!("All ignored files patterns: {:?}", dest_dir.get_ignore()).as_str());

}
