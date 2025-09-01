use dotx_core::*;
use eframe::egui;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("DotX - Extreme-Scale Dot Plot"),
        ..Default::default()
    };

    eframe::run_native(
        "DotX",
        options,
        Box::new(|cc| Ok(Box::new(DotXApp::new(cc)))),
    )
}

struct DotXApp {
    project: Option<Arc<DotXProject>>,
    ui_state: UiState,
}

#[derive(Default)]
struct UiState {
    show_new_project_dialog: bool,
    show_performance_panel: bool,
    ref_file_path: String,
    qry_file_path: String,
    project_name: String,
    selected_preset: AlignmentPreset,
}

#[derive(Default, Clone, Copy, PartialEq)]
enum AlignmentPreset {
    #[default]
    Bacterial,
    PlantTE,
    Mammal,
    Viral,
    Custom,
}

impl AlignmentPreset {
    fn name(&self) -> &'static str {
        match self {
            AlignmentPreset::Bacterial => "Bacterial",
            AlignmentPreset::PlantTE => "Plant TE",
            AlignmentPreset::Mammal => "Mammal",
            AlignmentPreset::Viral => "Viral",
            AlignmentPreset::Custom => "Custom",
        }
    }
}

impl DotXApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            project: None,
            ui_state: UiState::default(),
        }
    }

    fn open_project_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("DotX Project", &["dotx"])
            .pick_file()
        {
            match DotXProject::load(&path) {
                Ok(project) => {
                    self.project = Some(Arc::new(project));
                    log::info!("Loaded project: {}", path.display());
                }
                Err(e) => {
                    log::error!("Failed to load project {}: {}", path.display(), e);
                }
            }
        }
    }

    fn browse_fasta_file(&mut self, is_reference: bool) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("FASTA files", &["fa", "fasta", "fas", "fna"])
            .add_filter("All files", &["*"])
            .pick_file()
        {
            let path_str = path.to_string_lossy().to_string();

            // Validate FASTA file
            match FastaValidator::validate_file(&path) {
                Ok(validation) => {
                    if !validation.is_valid {
                        log::warn!(
                            "FASTA validation failed for {}: {:?}",
                            path.display(),
                            validation.errors
                        );
                        // Still allow the file to be selected but warn user
                    } else {
                        log::info!(
                            "FASTA file validated: {} sequences, {} total bases",
                            validation.sequence_count,
                            validation.total_length
                        );
                    }
                }
                Err(e) => {
                    log::error!("Failed to validate FASTA file {}: {}", path.display(), e);
                }
            }

            if is_reference {
                self.ui_state.ref_file_path = path_str;
            } else {
                self.ui_state.qry_file_path = path_str;
            }
        }
    }
}

impl eframe::App for DotXApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw_menu_bar(ctx);

        if self.project.is_none() {
            self.draw_home_screen(ctx);
        } else {
            self.draw_main_interface(ctx);
        }

        self.draw_dialogs(ctx);
    }
}

impl DotXApp {
    fn draw_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Project...").clicked() {
                        self.ui_state.show_new_project_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button("Open Project...").clicked() {
                        self.open_project_dialog();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("View", |ui| {
                    ui.checkbox(
                        &mut self.ui_state.show_performance_panel,
                        "Performance Panel",
                    );
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About DotX").clicked() {
                        // TODO: Show about dialog
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn draw_home_screen(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);

                ui.heading("Welcome to DotX");
                ui.add_space(20.0);

                ui.label("Extreme-scale dot plot visualization for bioinformatics");
                ui.add_space(40.0);

                ui.horizontal(|ui| {
                    if ui.button("New Project").clicked() {
                        self.ui_state.show_new_project_dialog = true;
                    }

                    if ui.button("Open Project").clicked() {
                        self.open_project_dialog();
                    }

                    if ui.button("Quick Compare").clicked() {
                        // TODO: Quick compare dialog
                    }
                });

                ui.add_space(60.0);

                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.heading("Recent Projects");
                        ui.add_space(10.0);
                        ui.label("No recent projects");
                    });
                });
            });
        });
    }

    fn draw_main_interface(&mut self, ctx: &egui::Context) {
        // Left panel - Data sources
        egui::SidePanel::left("data_panel")
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.heading("Data");
                ui.separator();

                ui.collapsing("Inputs", |ui| {
                    ui.label("Reference: ref.fa");
                    ui.label("Query: query.fa");
                });

                ui.collapsing("Alignments", |ui| {
                    ui.label("run1.paf (32.5M records)");
                });

                ui.collapsing("Annotations", |ui| {
                    ui.label("No annotations loaded");
                });
            });

        // Right panel - Properties
        egui::SidePanel::right("properties_panel")
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.heading("Properties");
                ui.separator();

                ui.collapsing("Display", |ui| {
                    ui.label("Color by:");
                    egui::ComboBox::from_id_salt("color_by")
                        .selected_text("Density")
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut "density", "density", "Density");
                            ui.selectable_value(&mut "identity", "identity", "Identity");
                            ui.selectable_value(&mut "strand", "strand", "Strand");
                        });
                });

                ui.collapsing("Filters", |ui| {
                    ui.label("Min identity: 0.0");
                    ui.label("Min length: 0");
                });

                ui.collapsing("Selection", |ui| {
                    ui.label("No selection");
                });
            });

        // Bottom panel - Jobs and logs
        if self.ui_state.show_performance_panel {
            egui::TopBottomPanel::bottom("jobs_panel")
                .default_height(150.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Jobs & Performance");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label("GPU: ✓");
                            ui.label("RAM: 2.1/8.0 GB");
                            ui.label("VRAM: 0.5/4.0 GB");
                        });
                    });
                    ui.separator();

                    ui.label("Ready");
                });
        }

        // Central panel - Dot plot canvas
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Dot Plot Canvas");
            ui.separator();

            let available_rect = ui.available_rect_before_wrap();
            ui.allocate_painter(available_rect.size(), egui::Sense::drag())
                .1
                .rect_filled(available_rect, 0.0, egui::Color32::from_gray(240));

            ui.centered_and_justified(|ui| {
                ui.label("Dot plot visualization will appear here");
            });
        });
    }

    fn draw_dialogs(&mut self, ctx: &egui::Context) {
        if self.ui_state.show_new_project_dialog {
            egui::Window::new("New Project")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    egui::Grid::new("new_project_grid")
                        .num_columns(2)
                        .spacing([10.0, 10.0])
                        .show(ui, |ui| {
                            ui.label("Project name:");
                            ui.text_edit_singleline(&mut self.ui_state.project_name);
                            ui.end_row();

                            ui.label("Reference FASTA:");
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.ui_state.ref_file_path);
                                if ui.button("Browse...").clicked() {
                                    self.browse_fasta_file(true);
                                }
                            });
                            ui.end_row();

                            ui.label("Query FASTA:");
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.ui_state.qry_file_path);
                                if ui.button("Browse...").clicked() {
                                    self.browse_fasta_file(false);
                                }
                            });
                            ui.end_row();

                            ui.label("Alignment preset:");
                            egui::ComboBox::from_id_salt("preset_combo")
                                .selected_text(self.ui_state.selected_preset.name())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.ui_state.selected_preset,
                                        AlignmentPreset::Bacterial,
                                        "Bacterial",
                                    );
                                    ui.selectable_value(
                                        &mut self.ui_state.selected_preset,
                                        AlignmentPreset::PlantTE,
                                        "Plant TE",
                                    );
                                    ui.selectable_value(
                                        &mut self.ui_state.selected_preset,
                                        AlignmentPreset::Mammal,
                                        "Mammal",
                                    );
                                    ui.selectable_value(
                                        &mut self.ui_state.selected_preset,
                                        AlignmentPreset::Viral,
                                        "Viral",
                                    );
                                    ui.selectable_value(
                                        &mut self.ui_state.selected_preset,
                                        AlignmentPreset::Custom,
                                        "Custom",
                                    );
                                });
                            ui.end_row();
                        });

                    ui.add_space(20.0);

                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            // TODO: Create project
                            self.ui_state.show_new_project_dialog = false;
                        }

                        if ui.button("Cancel").clicked() {
                            self.ui_state.show_new_project_dialog = false;
                        }
                    });
                });
        }
    }
}
