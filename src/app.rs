use eframe::{App, CreationContext, Frame, Storage};
use egui::{global_theme_preference_switch, menu, Button, CentralPanel, Context, TopBottomPanel};
use log::info;

use crate::processing::ui::{ProcessingChainTableRows, ProcessingChainViewer, ViewMode};

#[derive(Clone, Debug)]
pub struct GregCalc {
    // config: Config,
    // tabs: Tabs,
    // dock_state: DockState<Tab>,
    processing_chain: ProcessingChainTableRows,
    processing_chain_view_mode: ViewMode,
    notifications: Vec<Notification>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum Notification {
    Error(String),
}

impl GregCalc {
    pub fn new(_creation_context: &CreationContext) -> Self {
        // let mut config = Default::default();
        // let mut tabs = Default::default();
        // let dock_state;
        // if let Some(storage) = creation_context.storage {
        //     config = get_value(storage, CONFIG_KEY).unwrap_or_default();
        //     dock_state = get_value(storage, DOCK_STATE_KEY)
        //         .unwrap_or_else(|| DockState::new(Self::default_tabs()));
        //     tabs = get_value(storage, TABS_KEY).unwrap_or_default();
        // } else {
        //     dock_state = DockState::new(Self::default_tabs());
        // }

        Self {
            // config,
            // tabs,
            // dock_state,
            processing_chain: ProcessingChainTableRows::new(
                serde_json::from_str(include_str!("../recipes.json")).unwrap(),
            ),
            processing_chain_view_mode: ViewMode::Recipe,
            notifications: Default::default(),
        }
    }

    // fn focus_or_push_tab(&mut self, tab: Tab) {
    //     if let Some((surface_index, node_index, _)) = self.dock_state.find_tab(&tab) {
    //         self.dock_state
    //             .set_focused_node_and_surface((surface_index, node_index));
    //     } else {
    //         self.dock_state.push_to_first_leaf(tab);
    //     }
    // }

    fn show_menu_bar(&mut self, ctx: &Context) {
        TopBottomPanel::top("menu").show(ctx, |ui| {
            menu::bar(ui, |ui| {
                global_theme_preference_switch(ui);

                ui.menu_button("File", |ui| {
                    if ui.button("New Processing Chain").clicked() {
                        ui.close_menu();
                        // self.focus_or_push_tab(Tab::ProcessingChain(ProcessingChainTab::new()));
                    }
                    if ui.button("Open Processing Chain...").clicked() {
                        ui.close_menu();
                        // TODO use processing_chain_file_dialog() again
                        // for file in AsyncFileDialog::new()
                        //     .add_filter("Processing Chain", &["json"])
                        //     .pick_files()
                        //     .await
                        //     .into_iter()
                        // {
                        //     info!("{file:?}");
                        //     match self.tabs.load_processing_chain_tab(path) {
                        //         Ok(tab) => self.focus_or_push_tab(tab),
                        //         Err(notification) => self.notifications.push(notification),
                        //     }
                        // }
                    }

                    let tab = None::<()>;
                    // let tab =
                    //     self.dock_state
                    //         .find_active_focused()
                    //         .and_then(|(_, tab)| match &*tab {
                    //             Tab::Config => None,
                    //             Tab::ProcessingChain(processing_chain_tab) => {
                    //                 Some(processing_chain_tab)
                    //             }
                    //         });

                    if ui.add_enabled(tab.is_some(), Button::new("Save")).clicked() {
                        ui.close_menu();
                        // let notification = self
                        //     .tabs
                        //     .save_processing_chain_tab(tab.expect("tab should exist"));
                        // self.notifications.extend(notification);
                    }

                    if ui
                        .add_enabled(tab.is_some(), Button::new("Save As..."))
                        .clicked()
                    {
                        ui.close_menu();
                        // let notification = self
                        //     .tabs
                        //     .save_processing_chain_tab_as(tab.expect("tab should exist"));
                        // self.notifications.extend(notification);
                    }

                    ui.separator();

                    if ui.button("Config").clicked() {
                        ui.close_menu();
                        // self.focus_or_push_tab(Tab::Config);
                    }
                    if ui.button("Import Config...").clicked() {
                        ui.close_menu();
                        self.notifications
                            .push(Notification::Error("not yet implemented".into()));
                    }
                    if ui.button("Export Config...").clicked() {
                        ui.close_menu();
                        self.notifications
                            .push(Notification::Error("not yet implemented".into()));
                    }
                });
            });
        });
    }

    // fn show_dock_area(&mut self, ctx: &Context) {
    //     DockArea::new(&mut self.dock_state)
    //         .show_add_buttons(true)
    //         .show_window_close_buttons(false)
    //         .show_tab_name_on_hover(true)
    //         .show(ctx, &mut self.tabs);
    // }

    // fn default_tabs() -> Vec<Tab> {
    //     vec![Tab::ProcessingChain(ProcessingChainTab::new())]
    // }
}

impl App for GregCalc {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        ctx.input(|input_state| {
            for file in &input_state.raw.dropped_files {
                // TODO: Open file
                info!("Dropped: {file:#?}");
            }
        });

        self.show_menu_bar(ctx);
        // self.show_dock_area(ctx);

        CentralPanel::default().show(ctx, |ui| {
            ui.add(ProcessingChainViewer::new(
                &mut self.processing_chain_view_mode,
                &mut self.processing_chain,
            ));
        });
    }

    fn save(&mut self, _storage: &mut dyn Storage) {
        // set_value(storage, CONFIG_KEY, &self.config);
        // set_value(storage, DOCK_STATE_KEY, &self.dock_state);
        // set_value(storage, TABS_KEY, &self.tabs);
    }
}
