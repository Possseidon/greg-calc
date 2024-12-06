use std::{
    collections::BTreeMap,
    fs::{read_to_string, write},
    io,
    path::{Path, PathBuf},
};

use anyhow::Result;
use egui::{Color32, Id, Ui, WidgetText};
use egui_dock::{NodeIndex, SurfaceIndex, TabViewer};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{processing::ProcessingChain, Notification};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
enum UnsavedChanges {
    Dialog,
    Discard,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tabs {
    new_processing_chains: BTreeMap<Uuid, ProcessingChain>,
    processing_chains: BTreeMap<PathBuf, SavedProcessingChain>,
    unsaved_changes: Option<UnsavedChanges>,
    new_tabs: Vec<(SurfaceIndex, NodeIndex)>,
}

impl Tabs {
    fn get_processing_chain_mut(&mut self, tab: &ProcessingChainTab) -> &mut ProcessingChain {
        match tab {
            ProcessingChainTab::New { id } => self.new_processing_chains.entry(*id).or_default(),
            ProcessingChainTab::Path(path) => {
                &mut self
                    .processing_chains
                    .get_mut(path)
                    .expect("processing chain should be loaded")
                    .current
            }
        }
    }

    fn unload_processing_chain(&mut self, tab: &ProcessingChainTab) {
        match tab {
            ProcessingChainTab::New { id } => {
                self.new_processing_chains.remove(id);
            }
            ProcessingChainTab::Path(path) => {
                self.processing_chains
                    .remove(path)
                    .expect("processing chain should exist");
            }
        }
    }

    fn load_processing_chain(&mut self, path: PathBuf) -> Result<()> {
        let content = read_to_string(&path)?;
        let processing_chain = serde_json::from_str(&content)?;
        self.processing_chains.insert(path, processing_chain);

        Ok(())
    }

    fn save_new_processing_chain(&mut self, id: Uuid, path: PathBuf) -> io::Result<()> {
        write(
            &path,
            self.new_processing_chains
                .get(&id)
                .map(|processing_chain| processing_chain.to_json())
                .unwrap_or_else(|| ProcessingChain::default().to_json()),
        )?;

        let processing_chain = self.new_processing_chains.remove(&id).unwrap_or_default();

        self.processing_chains.insert(
            path,
            SavedProcessingChain {
                saved: processing_chain.clone(),
                current: processing_chain,
            },
        );

        Ok(())
    }

    fn save_processing_chain(&mut self, path: &Path) -> io::Result<()> {
        let processing_chain = self
            .processing_chains
            .get_mut(path)
            .expect("processing chain should exist");

        write(path, processing_chain.current.to_json())?;
        processing_chain.saved = processing_chain.current.clone();

        Ok(())
    }

    fn load_processing_chain_tab(&mut self, path: PathBuf) -> Result<Tab, Notification> {
        if let Err(error) = self.load_processing_chain(path.clone()) {
            Err(Notification::Error(error.to_string()))
        } else {
            Ok(Tab::ProcessingChain(ProcessingChainTab::Path(path)))
        }
    }

    fn save_processing_chain_tab(&mut self, tab: &ProcessingChainTab) -> Option<Notification> {
        match tab {
            ProcessingChainTab::New { .. } => self.save_processing_chain_tab_as(tab),
            ProcessingChainTab::Path(path) => self
                .save_processing_chain(path)
                .err()
                .map(|error| Notification::Error(error.to_string())),
        }
    }

    fn save_processing_chain_tab_as(&mut self, tab: &ProcessingChainTab) -> Option<Notification> {
        processing_chain_file_dialog()
            .save_file()
            .and_then(|path| match tab {
                ProcessingChainTab::New { id } => self
                    .save_new_processing_chain(*id, path)
                    .err()
                    .map(|error| Notification::Error(error.to_string())),
                ProcessingChainTab::Path(path) => self
                    .save_processing_chain(path)
                    .err()
                    .map(|error| Notification::Error(error.to_string())),
            })
    }
}

impl TabViewer for Tabs {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            Tab::Config => WidgetText::from("Config").strong(),
            Tab::ProcessingChain(tab) => match tab {
                ProcessingChainTab::New { .. } => WidgetText::from("New").strong(),
                ProcessingChainTab::Path(path) => {
                    if let Some(file_name) = path.file_name() {
                        file_name.to_string_lossy().into()
                    } else {
                        WidgetText::from("Invalid Filename").color(Color32::RED)
                    }
                }
            },
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        ui.label(format!("{tab:#?}"));
    }

    fn id(&mut self, tab: &mut Self::Tab) -> Id {
        Id::new(tab)
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
        let can_close = match tab {
            Tab::Config => true,
            Tab::ProcessingChain(tab) => match tab {
                ProcessingChainTab::New { id } => self
                    .new_processing_chains
                    .get(id)
                    .is_none_or(|processing_chain| processing_chain.is_empty()),
                ProcessingChainTab::Path(path) => !&self.processing_chains[path].changed(),
            },
        };

        if !can_close {
            self.unsaved_changes = Some(UnsavedChanges::Dialog);
        } else if let Tab::ProcessingChain(tab) = tab {
            self.unload_processing_chain(tab);
        }

        can_close
    }

    fn force_close(&mut self, tab: &mut Self::Tab) -> bool {
        let close = matches!(self.unsaved_changes, Some(UnsavedChanges::Discard));

        if close {
            if let Tab::ProcessingChain(tab) = tab {
                self.unload_processing_chain(tab);
            }
        }

        close
    }

    fn on_add(&mut self, surface: SurfaceIndex, node: NodeIndex) {
        self.new_tabs.push((surface, node));
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Tab {
    /// A [`Tab`] containing the current [`Config`].
    ///
    /// The [`Config`] can also be updated indirectly through [`ProcessingChainTab`]s.
    Config,
    ProcessingChain(ProcessingChainTab),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
enum ProcessingChainTab {
    /// A [`ProcessingChain`] that has not yet been saved to disk.
    New { id: Uuid },
    /// A [`ProcessingChain`] that has an associated file path.
    Path(PathBuf),
}

impl ProcessingChainTab {
    fn new() -> Self {
        Self::New { id: Uuid::new_v4() }
    }
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct SavedProcessingChain {
    saved: ProcessingChain,
    current: ProcessingChain,
}

impl SavedProcessingChain {
    fn changed(&self) -> bool {
        self.current != self.saved
    }
}

fn processing_chain_file_dialog() -> FileDialog {
    FileDialog::new().add_filter("Processing Chain", &["json"])
}
