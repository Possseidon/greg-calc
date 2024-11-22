use std::{
    cell::OnceCell,
    iter::{self, once},
    mem::replace,
};

use egui::{Align, Layout, Response, Separator, Ui, Widget};
use egui_extras::{Column, TableBuilder};
use enum_map::{Enum, EnumMap};
use enumset::{enum_set, EnumSet, EnumSetType};

use super::ProcessingChain;
use crate::config::Product;

pub struct ProcessingChainViewer<'a> {
    view_mode: &'a mut ViewMode,
    processing_chain: &'a mut CachedProcessingChain,
}

impl<'a> ProcessingChainViewer<'a> {
    pub fn new(
        view_mode: &'a mut ViewMode,
        processing_chain: &'a mut CachedProcessingChain,
    ) -> Self {
        Self {
            view_mode,
            processing_chain,
        }
    }
}

const HEADER_HEIGHT: f32 = 20.0;
pub const ROW_HEIGHT: f32 = 20.0;

impl Widget for ProcessingChainViewer<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.vertical(|ui| {
            ui.add(&mut *self.view_mode);
            ui.separator();

            let columns = self.view_mode.columns();
            let mut table_builder = TableBuilder::new(ui)
                .id_salt(*self.view_mode)
                .cell_layout(Layout::right_to_left(Align::Center))
                .striped(true);

            for column in columns {
                table_builder = table_builder.column(column.table_builder_column());
            }

            table_builder
                .header(HEADER_HEIGHT, |mut header| {
                    for column in columns {
                        header.col(|ui| {
                            ui.heading(column.header())
                                .on_hover_text(column.header_hover());
                        });
                    }
                })
                .body(|body| {
                    let cache = self.processing_chain.cache();
                    let total_rows = cache.view_mode_row_counts[*self.view_mode];
                    body.rows(ROW_HEIGHT, total_rows, |mut row| {
                        let index = row.index();
                        for column in columns {
                            row.col(|ui| {
                                match &cache.rows[index] {
                                    TableRow::Columns { texts } => ui.strong(&texts[column]),
                                    TableRow::Separator => {
                                        ui.add(Separator::default().horizontal())
                                    }
                                };
                            });
                        }
                    });
                });
        })
        .response
    }
}

/// The mode at which the [`ProcessingChain`] is viewed.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Enum)]
pub enum ViewMode {
    Recipe,
    Configuration,
    Speed,
}

impl ViewMode {
    const fn name(self) -> &'static str {
        match self {
            ViewMode::Recipe => "Recipe",
            ViewMode::Configuration => "Machine Configuration",
            ViewMode::Speed => "Speed",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            ViewMode::Recipe => "Shows information about only the recipes.",
            ViewMode::Configuration => {
                "Shows information based on specific machine configurations."
            }
            ViewMode::Speed => {
                "Shows information based on the speed at which machines are effectively running."
            }
        }
    }

    const fn columns(self) -> EnumSet<TableColumn> {
        match self {
            Self::Recipe => enum_set![
                TableColumn::Machine
                    | TableColumn::Catalysts
                    | TableColumn::Consumed
                    | TableColumn::ConsumedCount
                    | TableColumn::Produced
                    | TableColumn::ProducedCount
                    | TableColumn::TotalEu
                    | TableColumn::ProcessingTime
                    | TableColumn::Eu
            ],
            Self::Configuration => enum_set![
                TableColumn::Machine
                    | TableColumn::Catalysts
                    | TableColumn::Configuration
                    | TableColumn::ConfigurationConsumed
                    | TableColumn::ConfigurationConsumedCount
                    | TableColumn::ConfigurationProduced
                    | TableColumn::ConfigurationProducedCount
                    | TableColumn::ConfigurationEu
            ],
            Self::Speed => enum_set![
                TableColumn::Machine
                    | TableColumn::Catalysts
                    | TableColumn::Configuration
                    | TableColumn::Speed
                    | TableColumn::SpeedConsumed
                    | TableColumn::SpeedConsumedCount
                    | TableColumn::SpeedProduced
                    | TableColumn::SpeedProducedCount
                    | TableColumn::SpeedEu
            ],
        }
    }
}

impl Widget for &mut ViewMode {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.horizontal(|ui| {
            ui.heading("View Mode");
            for view_mode in [ViewMode::Recipe, ViewMode::Configuration, ViewMode::Speed] {
                ui.selectable_value(self, view_mode, view_mode.name())
                    .on_hover_text(view_mode.description());
            }
        })
        .response
    }
}

#[derive(Debug, Hash, PartialOrd, Ord, Enum, EnumSetType)]
enum TableColumn {
    Machine,
    Catalysts,
    Consumed,
    ConsumedCount,
    Produced,
    ProducedCount,
    TotalEu,
    ProcessingTime,
    Eu,

    Configuration,
    ConfigurationConsumed,
    ConfigurationConsumedCount,
    ConfigurationProduced,
    ConfigurationProducedCount,
    ConfigurationEu,

    Speed,
    SpeedConsumed,
    SpeedConsumedCount,
    SpeedProduced,
    SpeedProducedCount,
    SpeedEu,
}

impl TableColumn {
    fn header(self) -> &'static str {
        match self {
            TableColumn::Machine => "Machine",
            TableColumn::Catalysts => "Catalysts",
            TableColumn::Consumed => "Consumed",
            TableColumn::ConsumedCount => "#",
            TableColumn::Produced => "Produced",
            TableColumn::ProducedCount => "#",
            TableColumn::TotalEu => "Total EU",
            TableColumn::ProcessingTime => "Processing Time",
            TableColumn::Eu => "EU/tick",
            TableColumn::Configuration => "Configuration",
            TableColumn::ConfigurationConsumed => "Consumed",
            TableColumn::ConfigurationConsumedCount => "/sec",
            TableColumn::ConfigurationProduced => "Produced",
            TableColumn::ConfigurationProducedCount => "/sec",
            TableColumn::ConfigurationEu => "EU/tick",
            TableColumn::Speed => "Speed",
            TableColumn::SpeedConsumed => "Consumed",
            TableColumn::SpeedConsumedCount => "/sec",
            TableColumn::SpeedProduced => "Produced",
            TableColumn::SpeedProducedCount => "/sec",
            TableColumn::SpeedEu => "EU/tick",
        }
    }

    fn header_hover(self) -> &'static str {
        match self {
            TableColumn::Machine => "The kind of machine processing this recipe.",
            TableColumn::Catalysts => "Products that are required but not consumed.",
            TableColumn::Consumed => "Consumed products per processing cycle.",
            TableColumn::Produced => "Produced products per processing cycle.",
            TableColumn::TotalEu => "Total EU per processing cycle.",
            TableColumn::ProcessingTime => "Duration of a single processing cycle.",
            TableColumn::Eu => "EU/t for a single machine without overclocking.",
            TableColumn::Configuration => "The machines processing this recipe.",
            TableColumn::ConfigurationConsumed => "Consumed products by all machines.",
            TableColumn::ConfigurationProduced => "Produced procuts by all machines.",
            TableColumn::ConfigurationEu => "EU/t of all machines.",
            TableColumn::Speed => "How fast this machine can run.",
            TableColumn::SpeedConsumed => "Consumed products at the current speed.",
            TableColumn::SpeedProduced => "Produced products at the current speed.",
            TableColumn::SpeedEu => "EU/t at the current speed.",
            _ => "TODO",
        }
    }

    fn table_builder_column(self) -> Column {
        match self {
            TableColumn::ConsumedCount
            | TableColumn::ProducedCount
            | TableColumn::ConfigurationConsumedCount
            | TableColumn::ConfigurationProducedCount
            | TableColumn::SpeedConsumedCount
            | TableColumn::SpeedProducedCount => Column::auto(),
            _ => Column::initial(200.0).resizable(true),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CachedProcessingChain {
    processing_chain: ProcessingChain,
    allow_overproduction: Vec<Product>,
    cache: OnceCell<Cache>,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct Cache {
    rows: Vec<TableRow>,
    view_mode_row_counts: EnumMap<ViewMode, usize>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum TableRow {
    Columns {
        texts: Box<EnumMap<TableColumn, String>>,
    },
    Separator,
}

impl TableRow {
    fn any_visible(&self, columns: EnumSet<TableColumn>) -> bool {
        match self {
            Self::Columns { texts } => columns.iter().any(|column| !texts[column].is_empty()),
            Self::Separator => true, // separators are always visible
        }
    }
}

impl CachedProcessingChain {
    pub fn new(processing_chain: ProcessingChain) -> Self {
        Self {
            processing_chain,
            ..Default::default()
        }
    }

    fn processing_chain(&self) -> &ProcessingChain {
        &self.processing_chain
    }

    fn processing_chain_mut(&mut self) -> &mut ProcessingChain {
        self.cache.take();
        &mut self.processing_chain
    }

    fn allow_overproduction(&self) -> &[Product] {
        &self.allow_overproduction
    }

    fn allow_overproduction_mut(&mut self) -> &mut Vec<Product> {
        self.cache.take();
        &mut self.allow_overproduction
    }

    fn cache(&self) -> &Cache {
        self.cache.get_or_init(|| {
            let products = self.processing_chain.products();
            let products = self.processing_chain.products_with_configuration();
            let speeds = self
                .processing_chain
                .speeds(|product| self.allow_overproduction.contains(product));
            let products = self.processing_chain.products_with_speeds(&speeds);

            let rows = self
                .processing_chain
                .machines
                .iter()
                .flat_map(|machine_configuration| {
                    let recipe = &machine_configuration.recipe;

                    let mut first = true;
                    let mut machines = machine_configuration.machines.iter();
                    let mut catalysts = recipe.catalysts.iter();
                    let mut consumed = recipe.consumed.iter();
                    let mut produced = recipe.produced.iter();

                    once(TableRow::Separator).chain(iter::from_fn(move || {
                        let first = replace(&mut first, false);
                        let machine = machines.next();
                        let catalyst = catalysts.next();
                        let consumed = consumed.next();
                        let produced = produced.next();

                        (first
                            || machine.is_some()
                            || catalyst.is_some()
                            || consumed.is_some()
                            || produced.is_some())
                        .then(|| TableRow::Columns {
                            texts: Box::new(EnumMap::from_fn(|column| match column {
                                TableColumn::Machine => first
                                    .then(|| recipe.machine.name.clone())
                                    .unwrap_or_default(),
                                TableColumn::Catalysts => catalyst
                                    .map(|product| product.name.clone())
                                    .unwrap_or_default(),
                                TableColumn::Consumed => consumed
                                    .map(|(product, _)| product.name.clone())
                                    .unwrap_or_default(),
                                TableColumn::ConsumedCount => consumed
                                    .map(|(_, count)| count.to_string())
                                    .unwrap_or_default(),
                                TableColumn::Produced => produced
                                    .map(|(product, _)| product.name.clone())
                                    .unwrap_or_default(),
                                TableColumn::ProducedCount => produced
                                    .map(|(_, count)| count.to_string())
                                    .unwrap_or_default(),
                                TableColumn::TotalEu => first
                                    .then(|| recipe.total_eu().to_string())
                                    .unwrap_or_default(),
                                TableColumn::ProcessingTime => first
                                    .then(|| format!("{:.2} sec", (recipe.ticks as f64) / 20.0))
                                    .unwrap_or_default(),
                                TableColumn::Eu => first
                                    .then(|| format!("{} EU/tick", recipe.eu_per_tick))
                                    .unwrap_or_default(),
                                TableColumn::Configuration => "TODO".to_string(),
                                TableColumn::ConfigurationConsumed => "TODO".to_string(),
                                TableColumn::ConfigurationConsumedCount => "TODO".to_string(),
                                TableColumn::ConfigurationProduced => "TODO".to_string(),
                                TableColumn::ConfigurationProducedCount => "TODO".to_string(),
                                TableColumn::ConfigurationEu => "TODO".to_string(),
                                TableColumn::Speed => "TODO".to_string(),
                                TableColumn::SpeedConsumed => "TODO".to_string(),
                                TableColumn::SpeedConsumedCount => "TODO".to_string(),
                                TableColumn::SpeedProduced => "TODO".to_string(),
                                TableColumn::SpeedProducedCount => "TODO".to_string(),
                                TableColumn::SpeedEu => "TODO".to_string(),
                            })),
                        })
                    }))
                })
                .chain([
                    TableRow::Separator,
                    TableRow::Columns {
                        texts: Box::new(EnumMap::from_fn(|column| match column {
                            TableColumn::Machine => "Total".to_string(),
                            TableColumn::Catalysts => String::new(),
                            TableColumn::Consumed => "TODO".to_string(),
                            TableColumn::ConsumedCount => "TODO".to_string(),
                            TableColumn::Produced => "TODO".to_string(),
                            TableColumn::ProducedCount => "TODO".to_string(),
                            TableColumn::TotalEu => "TODO".to_string(),
                            TableColumn::ProcessingTime => "TODO".to_string(),
                            TableColumn::Eu => "TODO".to_string(),
                            TableColumn::Configuration => "TODO".to_string(),
                            TableColumn::ConfigurationConsumed => "TODO".to_string(),
                            TableColumn::ConfigurationConsumedCount => "TODO".to_string(),
                            TableColumn::ConfigurationProduced => "TODO".to_string(),
                            TableColumn::ConfigurationProducedCount => "TODO".to_string(),
                            TableColumn::ConfigurationEu => "TODO".to_string(),
                            TableColumn::Speed => "TODO".to_string(),
                            TableColumn::SpeedConsumed => "TODO".to_string(),
                            TableColumn::SpeedConsumedCount => "TODO".to_string(),
                            TableColumn::SpeedProduced => "TODO".to_string(),
                            TableColumn::SpeedProducedCount => "TODO".to_string(),
                            TableColumn::SpeedEu => "TODO".to_string(),
                        })),
                    },
                ])
                .collect::<Vec<_>>();

            let view_mode_row_counts = EnumMap::from_fn(|view_mode: ViewMode| {
                rows.iter()
                    .filter(|row| row.any_visible(view_mode.columns()))
                    .count()
            });

            Cache {
                rows,
                view_mode_row_counts,
            }
        })
    }
}
