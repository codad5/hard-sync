use super::file_tracker::FileTracker;
use std::{collections::{HashMap, HashSet}, path::Path};
use regex::Regex;
use walkdir::WalkDir;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirTracker {
    // path of the directory
    path: String,

    // meta data
    size: u64,
    last_modified: u64,
    created: u64,

    // dir contents
    ignore: HashSet<String>, // the ignore file is to be placed in the destination directory but refrerencing path relative to the source directory
    files: HashMap<String, FileTracker>, // Key is the file relative path
}

impl DirTracker {
    pub fn new(path: &Path) -> Result<DirTracker, String> {
        if !path.exists() {
            return Err("Path does not exist".to_string());
        }
        let metadata = path.metadata().unwrap();
        if !metadata.is_dir() {
            return Err("Path is not a directory".to_string());
        }
        let path = path.to_str().unwrap().to_string();
        let size = metadata.len();
        let last_modified = metadata.modified().unwrap().elapsed().unwrap().as_secs();
        let created = metadata.created().unwrap_or(std::time::SystemTime::UNIX_EPOCH).elapsed().unwrap().as_secs();

        let mut tracker = DirTracker {
            path,
            size,
            last_modified,
            created,
            ignore: HashSet::from([".hard_sync_cli/*".to_string(), "hard_sync.ignore".to_string()]),
            files: HashMap::new(),
        };
        let _ = tracker.load_ignore();
        Ok(tracker.clone())
    }

    pub fn add_file(&mut self, file: FileTracker) {
        self.files.insert(file.get_relative_path(Path::new(&self.path)).to_string(), file);
    }

    pub fn add_ignore(&mut self, ignore: String) {
        self.ignore.insert(ignore);
    }

}

//  getter methods
impl DirTracker {
    pub fn get_path(&self) -> &str {
        &self.path
    }

    pub fn get_size(&self) -> u64 {
        self.size
    }

    pub fn get_last_modified(&self) -> u64 {
        self.last_modified
    }

    pub fn get_created(&self) -> u64 {
        self.created
    }

    pub fn get_ignore(&self) -> HashSet<String> {
        self.ignore.clone()
    }

    pub fn get_files(&self) -> Vec<FileTracker> {
        self.files.values().cloned().collect()
    }

    pub fn get_file_hashmap(&self) -> &HashMap<String, FileTracker> {
        &self.files
    }

    pub fn get_file(&self, file_path: &str) -> Option<&FileTracker> {
        self.files.get(file_path)
    }

    pub fn has_file(&self, file_path: &str) -> bool {
        self.files.contains_key(file_path)
    }
}

//  implentation to get / load all the files and sub directories
impl DirTracker {
    pub fn import_files_from_directory(&mut self, recursive: bool) {
        let dir = Path::new(&self.path);
        let mut walker = WalkDir::new(dir);
        if !recursive {
            walker = walker.max_depth(1);
        }
        let walker = walker.into_iter();
        let mut files = HashMap::new();
        for entry in walker
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let metadata = entry.metadata().unwrap();
            if metadata.is_file() {
                let file_path = entry.path();
                files.insert(
                    // relative path
                    file_path
                        .strip_prefix(Path::new(&self.path))
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string(),
                    FileTracker::new(file_path.to_str().unwrap()).unwrap(),
                );
            }
        }

        self.files = files;
    }

    pub fn get_dir_diff(&self, other: &DirTracker) -> Vec<FileTracker> {
        let mut diff = Vec::new();
        for (key, file) in &self.files {
            if let Some(other_file) = other.files.get(key) {
                if file.get_last_file_hash() != other_file.get_last_file_hash() && !other.is_ignored(file.get_relative_path(Path::new(&self.get_path()))) {
                    diff.push(file.clone());
                }
            } else {
                if !other.is_ignored(file.get_relative_path(Path::new(&self.get_path()))) {
                    diff.push(file.clone());
                }
            }
        }
        diff
    }
    pub fn dir_initialized(&self) -> Result<(), String> {
        // look for .hard_sync_cli in first level
        let dir = Path::new(&self.path);
        if !dir.exists() {
            return Err("Directory does not exist".to_string());
        }
        // check if .hard_sync_cli exists
        let hard_sync_cli = dir.join(".hard_sync_cli");
        if !hard_sync_cli.exists() {
            return Err("Directory is not initalized".to_string());
        }
        Ok(())
    }

    pub fn setup_dir_config(&mut self) -> Result<(), String> {
        // create .hard_sync_cli
        let dir = Path::new(&self.path);
        // check if .hard_sync_cli exists
        if !dir.exists() {
            return Err("Directory does not exist".to_string());
        }
        if !dir.is_dir() {
            return Err("Path is not a directory".to_string());
        }
        let hard_sync_cli = dir.join(".hard_sync_cli");
        if hard_sync_cli.exists() {
            return Err("Directory is already initalized".to_string());
        }
        self.import_files_from_directory(true);
        std::fs::create_dir(hard_sync_cli.clone()).unwrap();
        // create the tracker.json file
        let tracker = hard_sync_cli.join("tracker.json");
        let tracker = std::fs::File::create(tracker).unwrap();
        let tracker = serde_json::to_writer(tracker, self).unwrap();
        Ok(())
    }

    //  to update the tracker.json file
    pub fn update_tracker(&self) -> Result<(), String> {
        let dir = Path::new(&self.path);
        let hard_sync_cli = dir.join(".hard_sync_cli");
        let tracker = hard_sync_cli.join("tracker.json");
        // check if .hard_sync_cli/tracker.json exists
        if !tracker.exists() {
            return Err("Tracker file does not exist".to_string());
        }
        // dont create a new file, just update the existing one
        let tracker = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(tracker)
            .unwrap();
        let tracker = serde_json::to_writer(tracker, self).unwrap();
        Ok(())
    }

    pub fn load_ignore(&mut self) -> Result<(), String> {
        let dir = Path::new(&self.path);
        let ignore = dir.join("hard_sync.ignore");
        if !ignore.exists() {
            return Err("Ignore file does not exist".to_string());
        }
        let ignore = std::fs::read_to_string(ignore).unwrap();
        for line in ignore.lines() {
            if line.starts_with("#") {
                continue;
            }
            self.add_ignore(line.to_string());
        }
        Ok(())
    }

    pub fn is_ignored(&self, file_path: &str) -> bool {
        for ignore in &self.ignore {
            let reg = Regex::new(ignore).unwrap();
            if reg.is_match(file_path) {
                return true;
            }
        }
        false
    }
}
