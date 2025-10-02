use std::fs;
use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub enum ConfigLine {
    Comment {
        text: String,
        source_file: PathBuf,
    },
    Empty {
        source_file: PathBuf,
    },
    Include {
        path: String,
        source_file: PathBuf,
    },
    HostEntry {
        pattern: String,
        options: Vec<(String, String)>,
        source_file: PathBuf,
    },
    GlobalOption {
        key: String,
        value: String,
        source_file: PathBuf,
    },
}

#[derive(Debug, Clone)]
pub struct SshConfig {
    pub lines: Vec<ConfigLine>,
    pub included_files: HashMap<PathBuf, IncludedFileData>,
    visited_files: HashSet<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct IncludedFileData {
    #[allow(dead_code)]
    pub content: String,
    #[allow(dead_code)]
    pub lines: Vec<ConfigLine>,
}

impl SshConfig {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            included_files: HashMap::new(),
            visited_files: HashSet::new(),
        }
    }

    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let mut config = Self::new();
        let canonical_path = path.as_ref().canonicalize()
            .unwrap_or_else(|_| path.as_ref().to_path_buf());
        config.visited_files.insert(canonical_path.clone());
        config.parse_content(&content, path.as_ref())?;
        Ok(config)
    }

    fn parse_content(&mut self, content: &str, base_path: &Path) -> Result<(), String> {
        let mut current_host: Option<(String, Vec<(String, String)>)> = None;

        for line in content.lines() {
            let trimmed = line.trim();

            // Handle comments
            if trimmed.starts_with('#') {
                if let Some((pattern, options)) = current_host.take() {
                    self.lines.push(ConfigLine::HostEntry {
                        pattern,
                        options,
                        source_file: base_path.to_path_buf(),
                    });
                }
                self.lines.push(ConfigLine::Comment {
                    text: line.to_string(),
                    source_file: base_path.to_path_buf(),
                });
                continue;
            }

            // Handle empty lines
            if trimmed.is_empty() {
                if let Some((pattern, options)) = current_host.take() {
                    self.lines.push(ConfigLine::HostEntry {
                        pattern,
                        options,
                        source_file: base_path.to_path_buf(),
                    });
                }
                self.lines.push(ConfigLine::Empty {
                    source_file: base_path.to_path_buf(),
                });
                continue;
            }

            // Parse key-value pairs
            let parts: Vec<&str> = trimmed.splitn(2, char::is_whitespace).collect();
            if parts.len() < 2 {
                continue;
            }

            let key = parts[0].trim();
            let value = parts[1].trim();

            match key.to_lowercase().as_str() {
                "host" => {
                    // Save previous host entry if exists
                    if let Some((pattern, options)) = current_host.take() {
                        self.lines.push(ConfigLine::HostEntry {
                            pattern,
                            options,
                            source_file: base_path.to_path_buf(),
                        });
                    }
                    // Start new host entry
                    current_host = Some((value.to_string(), Vec::new()));
                }
                "include" => {
                    // Save previous host entry if exists
                    if let Some((pattern, options)) = current_host.take() {
                        self.lines.push(ConfigLine::HostEntry {
                            pattern,
                            options,
                            source_file: base_path.to_path_buf(),
                        });
                    }
                    self.lines.push(ConfigLine::Include {
                        path: value.to_string(),
                        source_file: base_path.to_path_buf(),
                    });

                    // Parse included files
                    self.parse_include(value, base_path)?;
                }
                _ => {
                    if let Some((_, ref mut options)) = current_host {
                        // Add option to current host
                        options.push((key.to_string(), value.to_string()));
                    } else {
                        // Global option
                        self.lines.push(ConfigLine::GlobalOption {
                            key: key.to_string(),
                            value: value.to_string(),
                            source_file: base_path.to_path_buf(),
                        });
                    }
                }
            }
        }

        // Don't forget the last host entry
        if let Some((pattern, options)) = current_host {
            self.lines.push(ConfigLine::HostEntry {
                pattern,
                options,
                source_file: base_path.to_path_buf(),
            });
        }

        Ok(())
    }

    fn parse_include(&mut self, pattern: &str, base_path: &Path) -> Result<(), String> {
        // Expand ~ to home directory
        let expanded = if pattern.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(&pattern[2..])
            } else {
                PathBuf::from(pattern)
            }
        } else {
            PathBuf::from(pattern)
        };

        // Make relative paths relative to the config file's directory
        let include_path = if expanded.is_relative() {
            if let Some(parent) = base_path.parent() {
                parent.join(expanded)
            } else {
                expanded
            }
        } else {
            expanded
        };

        // Handle glob patterns
        let pattern_str = include_path.to_string_lossy().to_string();
        match glob::glob(&pattern_str) {
            Ok(paths) => {
                for entry in paths {
                    if let Ok(path) = entry {
                        if path.is_file() {
                            // Check for circular includes
                            let canonical_path = path.canonicalize()
                                .unwrap_or_else(|_| path.clone());

                            if self.visited_files.contains(&canonical_path) {
                                // Skip already visited files to prevent infinite recursion
                                continue;
                            }

                            self.visited_files.insert(canonical_path.clone());

                            if let Ok(content) = fs::read_to_string(&path) {
                                // Parse the included file - reuse visited_files to track across includes
                                self.parse_content(&content, &path)?;

                                // Store for reference
                                self.included_files.insert(
                                    path.clone(),
                                    IncludedFileData {
                                        content: content.clone(),
                                        lines: Vec::new(),
                                    },
                                );
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // If glob fails, try as a single file
                if include_path.is_file() {
                    // Check for circular includes
                    let canonical_path = include_path.canonicalize()
                        .unwrap_or_else(|_| include_path.clone());

                    if self.visited_files.contains(&canonical_path) {
                        // Skip already visited files
                        return Ok(());
                    }

                    self.visited_files.insert(canonical_path.clone());

                    if let Ok(content) = fs::read_to_string(&include_path) {
                        self.parse_content(&content, &include_path)?;

                        self.included_files.insert(
                            include_path.clone(),
                            IncludedFileData {
                                content,
                                lines: Vec::new(),
                            },
                        );
                    }
                }
            }
        }

        Ok(())
    }

    pub fn to_string(&self, file_path: &Path) -> String {
        let mut result = String::new();

        for line in &self.lines {
            // Get source_file from each line type and skip if not from this file
            let line_source = match line {
                ConfigLine::Comment { source_file, .. } => source_file,
                ConfigLine::Empty { source_file } => source_file,
                ConfigLine::Include { source_file, .. } => source_file,
                ConfigLine::HostEntry { source_file, .. } => source_file,
                ConfigLine::GlobalOption { source_file, .. } => source_file,
            };

            if line_source != file_path {
                continue;
            }

            // Now write the line
            match line {
                ConfigLine::Comment { text, .. } => {
                    result.push_str(text);
                    result.push('\n');
                }
                ConfigLine::Empty { .. } => {
                    result.push('\n');
                }
                ConfigLine::Include { path, .. } => {
                    result.push_str("Include ");
                    result.push_str(path);
                    result.push('\n');
                }
                ConfigLine::HostEntry {
                    pattern, options, ..
                } => {
                    result.push_str("Host ");
                    result.push_str(pattern);
                    result.push('\n');
                    for (key, value) in options {
                        result.push_str("    ");
                        result.push_str(key);
                        result.push(' ');
                        result.push_str(value);
                        result.push('\n');
                    }
                }
                ConfigLine::GlobalOption { key, value, .. } => {
                    result.push_str(key);
                    result.push(' ');
                    result.push_str(value);
                    result.push('\n');
                }
            }
        }

        result
    }

    pub fn save_all(&self, main_path: &Path) -> Result<(), String> {
        // Save main config file
        let main_content = self.to_string(main_path);
        fs::write(main_path, main_content).map_err(|e| e.to_string())?;

        // Save all included files
        for (include_path, _) in &self.included_files {
            let include_content = self.to_string(include_path);
            fs::write(include_path, include_content).map_err(|e| e.to_string())?;
        }

        Ok(())
    }
}
