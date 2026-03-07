use std::{fmt, fs};
use std::path::Path;
use std::time::UNIX_EPOCH;
use serde::de;
use serde::{de::Visitor, ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Sha256, Digest};  // You may need to add `sha2` crate for hashing
#[derive(Debug, Clone)]
pub struct FileTracker {
    path: String,
    size: u64,
    content: Vec<u8>,
    last_modified: u64,
    created: u64,
    last_accessed: u64,
    extension: String,
    last_file_hash: String,
    current_file_hash: String,
}

impl FileTracker {
    pub fn new(file_path: &str) -> std::io::Result<Self> {
        let path = Path::new(file_path);
        
        // Get file metadata
        let metadata = fs::metadata(path)?;

        // Read file content
        let content = fs::read(path)?;

        let last_modified = metadata.modified()?.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let created = metadata.created().unwrap_or(UNIX_EPOCH).duration_since(UNIX_EPOCH).unwrap().as_secs();
        let last_accessed = metadata.accessed()?.duration_since(UNIX_EPOCH).unwrap().as_secs();


        // File extension
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();

        // File hashes
        let current_file_hash = Self::calculate_hash(&content);
        let last_file_hash = current_file_hash.clone();  // Assume it's the same initially

        Ok(FileTracker {
            path: file_path.to_string(),
            size: metadata.len(),
            content,
            last_modified,
            created,
            last_accessed,
            extension,
            last_file_hash,
            current_file_hash,
        })
    }

    pub fn calculate_hash(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        format!("{:x}", hasher.finalize())  // Return as a hex string
    }

    pub fn update_hash(&mut self) {
        self.last_file_hash = self.current_file_hash.clone();
        self.current_file_hash = Self::calculate_hash(&self.content);
    }
}



// impl to get all the file info
impl FileTracker {
    pub fn get_path(&self) -> &str {
        &self.path
    }

    pub fn get_relative_path(&self, base_path: &Path) -> &str {
        let path = Path::new(&self.path);
        path.strip_prefix(base_path).unwrap().to_str().unwrap()
    }

    pub fn get_size(&self) -> u64 {
        self.size
    }

    pub fn get_content(&self) -> &[u8] {
        &self.content
    }

    pub fn get_last_modified(&self) -> u64 {
        self.last_modified
    }

    pub fn get_created(&self) -> u64 {
        self.created
    }

    pub fn get_last_accessed(&self) -> u64 {
        self.last_accessed
    }

    pub fn get_extension(&self) -> &str {
        &self.extension
    }

    pub fn get_last_file_hash(&self) -> &str {
        &self.last_file_hash
    }

    pub fn get_current_file_hash(&self) -> &str {
        &self.current_file_hash
    }
}


// implemtation for seralise

impl Serialize for FileTracker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Start a struct serialization with 3 fields
        let mut state = serializer.serialize_struct("FileTracker", 8)?;

        // Customize the serialized name and value for each field
        // Customize each field
        state.serialize_field("file_path", &self.path)?;
        state.serialize_field("file_size_bytes", &self.size)?;
        // You might want to convert timestamps to human-readable format, but here we use them as-is
        state.serialize_field("last_modified_timestamp", &self.last_modified)?;
        state.serialize_field("created_timestamp", &self.created)?;
        state.serialize_field("last_accessed_timestamp", &self.last_accessed)?;
        state.serialize_field("file_extension", &self.extension)?;
        state.serialize_field("previous_file_hash", &self.last_file_hash)?;
        state.serialize_field("current_file_hash", &self.current_file_hash)?;

        // End the serialization
        state.end()
    }
}


impl<'de> Deserialize<'de> for FileTracker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Define a custom visitor to handle the deserialization logic
        struct FileTrackerVisitor;

        impl<'de> Visitor<'de> for FileTrackerVisitor {
            type Value = FileTracker;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a FileTracker struct")
            }

            fn visit_map<V>(self, mut map: V) -> Result<FileTracker, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mut path = None;
                let mut size = None;
                let mut last_modified = None;
                let mut created = None;
                let mut last_accessed = None;
                let mut extension = None;
                let mut last_file_hash = None;
                let mut current_file_hash = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        "file_path" => {
                            if path.is_some() {
                                return Err(de::Error::duplicate_field("file_path"));
                            }
                            path = Some(map.next_value()?);
                        }
                        "file_size_bytes" => {
                            if size.is_some() {
                                return Err(de::Error::duplicate_field("file_size_bytes"));
                            }
                            size = Some(map.next_value()?);
                        }
                        "last_modified_timestamp" => {
                            if last_modified.is_some() {
                                return Err(de::Error::duplicate_field("last_modified_timestamp"));
                            }
                            last_modified = Some(map.next_value()?);
                        }
                        "created_timestamp" => {
                            if created.is_some() {
                                return Err(de::Error::duplicate_field("created_timestamp"));
                            }
                            created = Some(map.next_value()?);
                        }
                        "last_accessed_timestamp" => {
                            if last_accessed.is_some() {
                                return Err(de::Error::duplicate_field("last_accessed_timestamp"));
                            }
                            last_accessed = Some(map.next_value()?);
                        }
                        "file_extension" => {
                            if extension.is_some() {
                                return Err(de::Error::duplicate_field("file_extension"));
                            }
                            extension = Some(map.next_value()?);
                        }
                        "previous_file_hash" => {
                            if last_file_hash.is_some() {
                                return Err(de::Error::duplicate_field("previous_file_hash"));
                            }
                            last_file_hash = Some(map.next_value()?);
                        }
                        "current_file_hash" => {
                            if current_file_hash.is_some() {
                                return Err(de::Error::duplicate_field("current_file_hash"));
                            }
                            current_file_hash = Some(map.next_value()?);
                        }
                        _ => {
                            // Skip unknown fields
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                // Construct the FileTracker instance
                let path = path.ok_or_else(|| de::Error::missing_field("file_path"))?;
                let size = size.ok_or_else(|| de::Error::missing_field("file_size_bytes"))?;
                let last_modified = last_modified.ok_or_else(|| de::Error::missing_field("last_modified_timestamp"))?;
                let created = created.ok_or_else(|| de::Error::missing_field("created_timestamp"))?;
                let last_accessed = last_accessed.ok_or_else(|| de::Error::missing_field("last_accessed_timestamp"))?;
                let extension = extension.ok_or_else(|| de::Error::missing_field("file_extension"))?;
                let last_file_hash = last_file_hash.ok_or_else(|| de::Error::missing_field("previous_file_hash"))?;
                let current_file_hash = current_file_hash.ok_or_else(|| de::Error::missing_field("current_file_hash"))?;

                Ok(FileTracker {
                    path,
                    size,
                    content: vec![], // We can default it to an empty vector if content is not part of the deserialization
                    last_modified,
                    created,
                    last_accessed,
                    extension,
                    last_file_hash,
                    current_file_hash,
                })
            }
        }

        // Using the visitor to process the deserialization
        deserializer.deserialize_map(FileTrackerVisitor)
    }
}