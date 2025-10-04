#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ssh_config;

use eframe::{egui, CreationContext};
use ssh_config::{ConfigLine, SshConfig};
use std::path::PathBuf;
use std::sync::Arc;
use egui::{ViewportCommand, WindowLevel};

fn main() -> Result<(), eframe::Error> {
    // Set up panic handler to allocate console on Windows if needed
    #[cfg(all(windows, not(debug_assertions)))]
    {
        std::panic::set_hook(Box::new(|panic_info| {
            unsafe {
                // Allocate console for crash output
                winapi::um::consoleapi::AllocConsole();
            }
            eprintln!("Application panicked: {}", panic_info);
            eprintln!("\nPress Enter to exit...");
            let _ = std::io::stdin().read_line(&mut String::new());
        }));
    }

    let icon = Arc::new(eframe::icon_data::from_png_bytes(include_bytes!("../icon.png")).expect("Failed to load icon"));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_active(true)
            .with_title("SSH Config Editor")
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "SSH Config Editor",
        options,
        Box::new(|cc| Ok(Box::new(SshConfigApp::new(cc)))),
    )
}

struct SshConfigApp {
    config: Option<SshConfig>,
    config_path: Option<PathBuf>,
    selected_host: Option<usize>,
    status_message: String,
    initialized: bool,
    search_query: String,
    search_focused: bool,
    new_option_key: String,
    new_option_value: String,
    show_shortcuts: bool,
    is_dirty: bool,
    show_quit_dialog: bool,
    show_new_host_dialog: bool,
    new_host_pattern: String,
    new_host_target_file: Option<PathBuf>,
    always_on_top: bool,
}

impl SshConfigApp {
    fn new(_cc: &CreationContext) -> Self {
        Self {
            config: None,
            config_path: None,
            selected_host: None,
            status_message: String::new(),
            initialized: false,
            search_query: String::new(),
            search_focused: false,
            new_option_key: String::new(),
            new_option_value: String::new(),
            show_shortcuts: false,
            is_dirty: false,
            show_quit_dialog: false,
            show_new_host_dialog: false,
            new_host_pattern: String::new(),
            new_host_target_file: None,
            always_on_top: false,
        }
    }

    fn save_config(&mut self) {
        if let (Some(config), Some(path)) = (&self.config, &self.config_path) {
            match config.save_all(path) {
                Ok(_) => {
                    let file_count = config.included_files.len() + 1;
                    self.status_message = format!("Saved {} file(s)", file_count);
                    self.is_dirty = false;
                }
                Err(e) => {
                    self.status_message = format!("Error saving: {}", e);
                }
            }
        } else {
            self.status_message = "No file loaded".to_string();
        }
    }

    fn load_default_config(&mut self) {
        if let Some(home) = dirs::home_dir() {
            let default_path = home.join(".ssh").join("config");
            if default_path.exists() {
                match SshConfig::parse_file(&default_path) {
                    Ok(config) => {
                        let included_count = config.included_files.len();
                        self.config = Some(config);
                        self.config_path = Some(default_path.clone());
                        self.status_message = if included_count > 0 {
                            format!(
                                "Loaded: {} ({} included files)",
                                default_path.display(),
                                included_count
                            )
                        } else {
                            format!("Loaded: {}", default_path.display())
                        };
                    }
                    Err(e) => {
                        self.status_message = format!("Error loading default config: {}", e);
                    }
                }
            } else {
                self.status_message = format!("Default config not found: {}", default_path.display());
            }
        }
    }

    fn show_shortcuts_popup(&mut self, ctx: &egui::Context) {
        egui::Window::new("⌨ Keyboard Shortcuts")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(400.0);

                ui.heading("File Operations");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl+O").monospace().strong());
                    ui.label("Open SSH config file");
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl+N").monospace().strong());
                    ui.label("New host entry");
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl+S").monospace().strong());
                    ui.label("Save all changes");
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl+Q").monospace().strong());
                    ui.label("Quit (prompts to save if dirty)");
                });

                ui.add_space(10.0);
                ui.heading("Search & Navigation");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl+F").monospace().strong());
                    ui.label("Focus search box");
                });
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Escape").monospace().strong());
                    ui.label("Clear search / unfocus");
                });

                ui.add_space(10.0);
                ui.heading("View");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl+A").monospace().strong());
                    ui.label("Toggle always on top");
                });

                ui.add_space(10.0);
                ui.heading("Quick Actions");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Ctrl+Shift+L").monospace().strong());
                    ui.label("Add legacy SSH options");
                });
                ui.label(
                    egui::RichText::new("  (to selected host)")
                        .color(egui::Color32::GRAY)
                        .italics(),
                );

                ui.add_space(10.0);
                ui.heading("Legacy SSH Options");
                ui.separator();
                ui.label(egui::RichText::new("Adds these options:").color(egui::Color32::GRAY));
                ui.label(egui::RichText::new("  • HostKeyAlgorithms +ssh-rsa,ssh-rsa-cert-v01@openssh.com").monospace().small());
                ui.label(egui::RichText::new("  • PubkeyAcceptedAlgorithms +ssh-rsa,ssh-rsa-cert-v01@openssh.com").monospace().small());
                ui.label(egui::RichText::new("  • Ciphers +aes256-cbc,aes128-cbc").monospace().small());
                ui.label(egui::RichText::new("  • MACs +aes256-cbc,hmac-sha1").monospace().small());
                ui.label(egui::RichText::new("  • KexAlgorithms +diffie-hellman-group1-sha1").monospace().small());
                ui.add_space(15.0);
                ui.separator();
                if ui.button("Close").clicked() {
                    self.show_shortcuts = false;
                }
            });
    }

    fn show_quit_dialog(&mut self, ctx: &egui::Context) {
        egui::Window::new("⚠ Unsaved Changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(300.0);

                ui.label("You have unsaved changes. Do you want to save before quitting?");
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Save and Quit").clicked() {
                        self.save_config();
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                        self.show_quit_dialog = false;
                    }

                    if ui.button("Quit Without Saving").clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                        self.show_quit_dialog = false;
                    }

                    if ui.button("Cancel").clicked() {
                        self.show_quit_dialog = false;
                    }
                });
            });
    }

    fn show_new_host_dialog(&mut self, ctx: &egui::Context) {
        egui::Window::new("➕ New Host Entry")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_width(400.0);

                ui.label("Create a new SSH host entry:");
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    ui.label("Host Pattern:");
                    let pattern_response = ui.text_edit_singleline(&mut self.new_host_pattern);

                    // Enter on host pattern creates the entry (if valid)
                    if pattern_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let can_create = !self.new_host_pattern.is_empty()
                            && self.new_host_target_file.is_some();

                        if can_create {
                            if let (Some(config), Some(target_file)) =
                                (&mut self.config, &self.new_host_target_file)
                            {
                                // Create new host entry
                                let new_entry = ConfigLine::HostEntry {
                                    pattern: self.new_host_pattern.clone(),
                                    options: Vec::new(),
                                    source_file: target_file.clone(),
                                };

                                config.lines.push(new_entry);

                                self.is_dirty = true;
                                self.status_message = format!(
                                    "Created new host '{}' in {}",
                                    self.new_host_pattern,
                                    target_file.display()
                                );

                                self.selected_host = Some(config.lines.len() - 1);

                                self.new_host_pattern.clear();
                                self.new_host_target_file = None;
                                self.show_new_host_dialog = false;
                            }
                        }
                    }
                });

                ui.add_space(5.0);

                // File selection dropdown
                ui.horizontal(|ui| {
                    ui.label("Target File:");

                    if let Some(config) = &self.config {
                        // Build list of all files (main + included)
                        let mut all_files = vec![];
                        if let Some(main_path) = &self.config_path {
                            all_files.push(main_path.clone());
                        }
                        for (include_path, _) in &config.included_files {
                            all_files.push(include_path.clone());
                        }

                        if !all_files.is_empty() {
                            // Set default if not set
                            if self.new_host_target_file.is_none() {
                                self.new_host_target_file = Some(all_files[0].clone());
                            }

                            egui::ComboBox::from_id_salt("target_file_combo")
                                .selected_text(
                                    self.new_host_target_file
                                        .as_ref()
                                        .map(|p| p.display().to_string())
                                        .unwrap_or_else(|| "Select file...".to_string()),
                                )
                                .show_ui(ui, |ui| {
                                    for file in &all_files {
                                        let is_selected = self.new_host_target_file.as_ref() == Some(file);
                                        if ui.selectable_label(is_selected, file.display().to_string()).clicked() {
                                            self.new_host_target_file = Some(file.clone());
                                        }
                                    }
                                });
                        }
                    }
                });

                ui.add_space(15.0);
                ui.separator();

                ui.horizontal(|ui| {
                    let can_create = !self.new_host_pattern.is_empty()
                        && self.new_host_target_file.is_some();

                    if ui.add_enabled(can_create, egui::Button::new("Create")).clicked() {
                        if let (Some(config), Some(target_file)) =
                            (&mut self.config, &self.new_host_target_file)
                        {
                            // Create new host entry
                            let new_entry = ConfigLine::HostEntry {
                                pattern: self.new_host_pattern.clone(),
                                options: Vec::new(),
                                source_file: target_file.clone(),
                            };

                            // Add to the end
                            config.lines.push(new_entry);

                            self.is_dirty = true;
                            self.status_message = format!(
                                "Created new host '{}' in {}",
                                self.new_host_pattern,
                                target_file.display()
                            );

                            // Select the newly created host
                            self.selected_host = Some(config.lines.len() - 1);

                            // Clear and close
                            self.new_host_pattern.clear();
                            self.new_host_target_file = None;
                            self.show_new_host_dialog = false;
                        }
                    }

                    if ui.button("Cancel").clicked() {
                        self.new_host_pattern.clear();
                        self.new_host_target_file = None;
                        self.show_new_host_dialog = false;
                    }
                });
            });
    }
}

impl eframe::App for SshConfigApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Reduce frame rate when idle to save power (2 FPS = 500ms)
        // UI still feels instant but uses much less GPU when idle
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        // Load default config on first frame
        if !self.initialized {
            self.load_default_config();
            self.initialized = true;
        }

        // Handle Ctrl+F for search
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
            self.search_focused = true;
        }

        // Handle Escape to clear search
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.search_query.clear();
            self.search_focused = false;
        }

        // Handle Ctrl+Shift+L to add legacy SSH options
        let add_legacy = ctx.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::L));

        // Handle Ctrl+S to save
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S)) {
            self.save_config();
        }

        // Handle Ctrl+O to open
        let open_file = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::O));

        // Handle Ctrl+Q to quit
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Q)) {
            if self.is_dirty {
                self.show_quit_dialog = true;
            } else {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }

        // Handle Ctrl+N to create new host
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::N)) {
            // Pre-fill target file based on currently selected host
            if let Some(config) = &self.config {
                if let Some(selected_idx) = self.selected_host {
                    if let Some(ConfigLine::HostEntry { source_file, .. }) =
                        config.lines.get(selected_idx)
                    {
                        self.new_host_target_file = Some(source_file.clone());
                    }
                } else if let Some(main_path) = &self.config_path {
                    self.new_host_target_file = Some(main_path.clone());
                }
            }
            self.show_new_host_dialog = true;
        }

        // Handle Ctrl+A to toggle always on top
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::A)) {
            self.always_on_top = !self.always_on_top;
            let level = if self.always_on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            };
            ctx.send_viewport_cmd(ViewportCommand::WindowLevel(level));
            self.status_message = if self.always_on_top {
                "Always on top: enabled".to_string()
            } else {
                "Always on top: disabled".to_string()
            };
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::containers::menu::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open SSH Config  (Ctrl+O)").clicked() || open_file {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("SSH Config", &["config", "*"])
                            .pick_file()
                        {
                            match SshConfig::parse_file(&path) {
                                Ok(config) => {
                                    let included_count = config.included_files.len();
                                    self.config = Some(config);
                                    self.config_path = Some(path.clone());
                                    self.is_dirty = false;
                                    self.status_message = if included_count > 0 {
                                        format!(
                                            "Loaded: {} ({} included files)",
                                            path.display(),
                                            included_count
                                        )
                                    } else {
                                        format!("Loaded: {}", path.display())
                                    };
                                }
                                Err(e) => {
                                    self.status_message = format!("Error loading file: {}", e);
                                }
                            }
                        }
                        ui.close();
                    }

                    if ui.button("Save  (Ctrl+S)").clicked() {
                        self.save_config();
                        ui.close();
                    }

                    if ui.button("Reload").clicked() {
                        if let Some(path) = &self.config_path.clone() {
                            match SshConfig::parse_file(path) {
                                Ok(config) => {
                                    let included_count = config.included_files.len();
                                    self.config = Some(config);
                                    self.is_dirty = false;
                                    self.status_message = if included_count > 0 {
                                        format!(
                                            "Reloaded: {} ({} included files)",
                                            path.display(),
                                            included_count
                                        )
                                    } else {
                                        format!("Reloaded: {}", path.display())
                                    };
                                }
                                Err(e) => {
                                    self.status_message = format!("Error reloading: {}", e);
                                }
                            }
                        }
                        ui.close();
                    }

                    ui.separator();

                    if ui.button("Quit  (Ctrl+Q)").clicked() {
                        if self.is_dirty {
                            self.show_quit_dialog = true;
                        } else {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                        ui.close();
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.button("New Host Entry  (Ctrl+N)").clicked() {
                        // Pre-fill target file based on currently selected host
                        if let Some(config) = &self.config {
                            if let Some(selected_idx) = self.selected_host {
                                if let Some(ConfigLine::HostEntry { source_file, .. }) =
                                    config.lines.get(selected_idx)
                                {
                                    self.new_host_target_file = Some(source_file.clone());
                                }
                            } else if let Some(main_path) = &self.config_path {
                                self.new_host_target_file = Some(main_path.clone());
                            }
                        }
                        self.show_new_host_dialog = true;
                        ui.close();
                    }
                });

                ui.menu_button("View", |ui| {
                    let always_on_top_label = if self.always_on_top {
                        "✓ Always on Top  (Ctrl+A)"
                    } else {
                        "Always on Top  (Ctrl+A)"
                    };

                    if ui.button(always_on_top_label).clicked() {
                        self.always_on_top = !self.always_on_top;
                        let level = if self.always_on_top {
                            WindowLevel::AlwaysOnTop
                        } else {
                            WindowLevel::Normal
                        };
                        ctx.send_viewport_cmd(ViewportCommand::WindowLevel(level));
                        self.status_message = if self.always_on_top {
                            "Always on top: enabled".to_string()
                        } else {
                            "Always on top: disabled".to_string()
                        };
                        ui.close();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("Keyboard Shortcuts").clicked() {
                        self.show_shortcuts = true;
                        ui.close();
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
            });
        });

        if let Some(config) = &mut self.config {
            egui::SidePanel::left("hosts_panel")
                .resizable(true)
                .default_width(250.0)
                .show(ctx, |ui| {
                    ui.heading("SSH Hosts");
                    ui.separator();

                    // Search box
                    ui.horizontal(|ui| {
                        ui.label("🔍");
                        let search_response = ui.text_edit_singleline(&mut self.search_query);

                        if self.search_focused {
                            search_response.request_focus();
                            self.search_focused = false;
                        }

                        if !self.search_query.is_empty() && ui.button("✖").clicked() {
                            self.search_query.clear();
                        }
                    });
                    ui.separator();

                    let search_lower = self.search_query.to_lowercase();
                    let is_searching = !search_lower.is_empty();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (idx, line) in config.lines.iter().enumerate() {
                            match line {
                                ConfigLine::HostEntry {
                                    pattern,
                                    source_file,
                                    ..
                                } => {
                                    // Filter by search query
                                    if is_searching && !pattern.to_lowercase().contains(&search_lower) {
                                        continue;
                                    }

                                    let is_selected = self.selected_host == Some(idx);

                                    // Show indicator if from included file
                                    let display_text = if let Some(main_path) = &self.config_path {
                                        if source_file != main_path {
                                            format!("  {}", pattern)
                                        } else {
                                            pattern.clone()
                                        }
                                    } else {
                                        pattern.clone()
                                    };

                                    if ui.selectable_label(is_selected, &display_text).clicked() {
                                        self.selected_host = Some(idx);
                                    }
                                }
                                ConfigLine::Include { path, .. } => {
                                    if !is_searching {
                                        ui.label(
                                            egui::RichText::new(format!("📁 Include: {}", path))
                                                .color(egui::Color32::DARK_GRAY),
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                    });
                });

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Configuration Details");
                ui.separator();

                if let Some(selected_idx) = self.selected_host {
                    if let Some(ConfigLine::HostEntry {
                        pattern,
                        options,
                        source_file,
                    }) = config.lines.get_mut(selected_idx)
                    {
                        // Add legacy SSH options if Ctrl+Shift+L was pressed
                        if add_legacy {
                            let legacy_options = vec![
                                ("HostKeyAlgorithms", "+ssh-rsa,ssh-rsa-cert-v01@openssh.com,ssh-dss"),
                                ("PubkeyAcceptedAlgorithms", "+ssh-rsa,ssh-rsa-cert-v01@openssh.com"),
                                ("Ciphers", "+aes256-cbc,aes128-cbc,3des-cbc"),
                                ("MACs", "+hmac-sha1,hmac-md5"),
                                ("KexAlgorithms", "+diffie-hellman-group14-sha1,diffie-hellman-group1-sha1"),
                            ];

                            for (key, value) in legacy_options {
                                // Check if this option already exists
                                if !options.iter().any(|(k, _)| k == key) {
                                    options.push((key.to_string(), value.to_string()));
                                }
                            }

                            self.status_message = format!("Added legacy SSH options to {}", pattern);
                            self.is_dirty = true;
                        }

                        // Show source file info
                        ui.horizontal(|ui| {
                            ui.label("Source File:");
                            ui.label(
                                egui::RichText::new(source_file.display().to_string())
                                    .color(egui::Color32::GRAY),
                            );
                        });

                        ui.separator();

                        ui.horizontal(|ui| {
                            ui.label("Host Pattern:");
                            if ui.text_edit_singleline(pattern).changed() {
                                self.is_dirty = true;
                            }
                        });

                        ui.separator();
                        ui.heading("Options");

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let mut to_remove = None;

                            for (idx, (key, value)) in options.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{}:", key));
                                    if ui.text_edit_singleline(value).changed() {
                                        self.is_dirty = true;
                                    }
                                    if ui.button("🗑").clicked() {
                                        to_remove = Some(idx);
                                    }
                                });
                            }

                            if let Some(idx) = to_remove {
                                options.remove(idx);
                                self.is_dirty = true;
                            }

                            ui.separator();
                            ui.label(egui::RichText::new("Add New Option").strong());

                            let mut add_option = false;

                            ui.horizontal(|ui| {
                                ui.label("Key:");
                                let key_response = ui.add(
                                    egui::TextEdit::singleline(&mut self.new_option_key)
                                        .id(egui::Id::new("new_option_key_field"))
                                );

                                // Show error if key contains spaces
                                if self.new_option_key.contains(' ') {
                                    ui.label(
                                        egui::RichText::new("⚠ No spaces allowed")
                                            .color(egui::Color32::RED),
                                    );
                                }

                                // Enter on key field focuses value field
                                if key_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    ui.memory_mut(|m| m.request_focus(egui::Id::new("new_option_value_field")));
                                }
                            });

                            ui.horizontal(|ui| {
                                ui.label("Value:");
                                let value_response = ui.add(
                                    egui::TextEdit::singleline(&mut self.new_option_value)
                                        .id(egui::Id::new("new_option_value_field"))
                                );

                                // Enter on value field adds the option
                                if value_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    let can_add = !self.new_option_key.is_empty()
                                        && !self.new_option_key.contains(' ')
                                        && !self.new_option_value.is_empty();

                                    if can_add {
                                        add_option = true;
                                    }
                                }
                            });

                            if add_option {
                                options.push((
                                    self.new_option_key.clone(),
                                    self.new_option_value.clone(),
                                ));
                                self.new_option_key.clear();
                                self.new_option_value.clear();
                                self.is_dirty = true;
                            }

                            ui.horizontal(|ui| {
                                let can_add = !self.new_option_key.is_empty()
                                    && !self.new_option_key.contains(' ')
                                    && !self.new_option_value.is_empty();

                                if ui
                                    .add_enabled(can_add, egui::Button::new("➕ Add Option"))
                                    .clicked()
                                {
                                    options.push((
                                        self.new_option_key.clone(),
                                        self.new_option_value.clone(),
                                    ));
                                    self.new_option_key.clear();
                                    self.new_option_value.clear();
                                    self.is_dirty = true;
                                }
                            });
                        });
                    }
                } else {
                    ui.label("Select a host from the left panel to edit");

                    ui.separator();
                    ui.heading("All Configuration Lines");

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for line in &config.lines {
                            match line {
                                ConfigLine::Comment { text, .. } => {
                                    ui.label(egui::RichText::new(text).color(egui::Color32::GRAY));
                                }
                                ConfigLine::Empty { .. } => {
                                    ui.label("");
                                }
                                ConfigLine::Include { path, .. } => {
                                    ui.label(
                                        egui::RichText::new(format!("Include {}", path))
                                            .color(egui::Color32::LIGHT_BLUE),
                                    );
                                }
                                ConfigLine::GlobalOption { key, value, .. } => {
                                    ui.label(format!("{} {}", key, value));
                                }
                                ConfigLine::HostEntry {
                                    pattern,
                                    options,
                                    source_file: _,
                                } => {
                                    ui.label(
                                        egui::RichText::new(format!("Host {}", pattern))
                                            .strong(),
                                    );
                                    for (key, value) in options {
                                        ui.label(format!("    {} {}", key, value));
                                    }
                                }
                            }
                        }
                    });
                }
            });
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(200.0);
                    ui.heading("SSH Config Editor");
                    ui.add_space(20.0);
                    ui.label("Click File → Open SSH Config to get started");
                });
            });
        }

        // Show popups
        if self.show_shortcuts {
            self.show_shortcuts_popup(ctx);
        }

        if self.show_quit_dialog {
            self.show_quit_dialog(ctx);
        }

        if self.show_new_host_dialog {
            self.show_new_host_dialog(ctx);
        }
    }
}
