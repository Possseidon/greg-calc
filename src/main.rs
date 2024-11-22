#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod processing;

use std::fs::read_to_string;

use anyhow::Result;
use eframe::{App, CreationContext, Frame, Storage};
use egui::{global_theme_preference_switch, menu, Button, CentralPanel, Context, TopBottomPanel};
use processing::ui::{CachedProcessingChain, ProcessingChainViewer, ViewMode};

#[derive(Clone, Debug)]
struct GregCalc {
    // config: Config,
    // tabs: Tabs,
    // dock_state: DockState<Tab>,
    processing_chain: CachedProcessingChain,
    processing_chain_view_mode: ViewMode,
    notifications: Vec<Notification>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum Notification {
    Error(String),
}

// const CONFIG_KEY: &str = "config";
// const DOCK_STATE_KEY: &str = "dock_state";
// const TABS_KEY: &str = "tabs";

impl GregCalc {
    fn new(_creation_context: &CreationContext) -> Self {
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
            processing_chain: CachedProcessingChain::new(
                serde_json::from_str(&read_to_string("recipes.json").unwrap()).unwrap(),
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
                        // for path in processing_chain_file_dialog()
                        //     .pick_files()
                        //     .into_iter()
                        //     .flatten()
                        // {
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

fn main() -> Result<(), eframe::Error> {
    eframe::run_native(
        "GregCalc",
        Default::default(),
        Box::new(|creation_context| Ok(Box::new(GregCalc::new(creation_context)))),
    )
}
