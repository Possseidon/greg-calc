use std::{
    cell::OnceCell,
    iter::{self, once},
    mem::replace,
    num::NonZeroU64,
};

use egui::{Align, Layout, Response, Separator, Ui, Widget};
use egui_extras::{Column, TableBuilder};
use enum_map::{Enum, EnumMap};
use enumset::{enum_set, EnumSet, EnumSetType};
use num_rational::Rational64;
use num_traits::ToPrimitive;

use super::{MachineConfiguration, ProcessingChain, Speeds};
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

const HEADER_HEIGHT: f32 = 30.0;
const ROW_HEIGHT: f32 = 20.0;
const ROW_SEPARATOR_HEIGHT: f32 = 7.0;

impl Widget for ProcessingChainViewer<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.vertical(|ui| {
            ui.add(&mut *self.view_mode);
            ui.separator();

            let view_mode = *self.view_mode;

            let columns = view_mode.columns();
            let mut table_builder = TableBuilder::new(ui)
                .id_salt(view_mode)
                .cell_layout(Layout::right_to_left(Align::Center))
                .striped(true);

            for column in columns {
                table_builder = table_builder.column(column.table_builder_column());
            }

            table_builder
                .header(HEADER_HEIGHT, |mut header| {
                    for column in columns {
                        header.col(|ui| {
                            ui.heading(column.header(view_mode))
                                .on_hover_text(column.header_hover(view_mode));
                        });
                    }
                })
                .body(|body| {
                    let rows = self.processing_chain.rows(view_mode);
                    body.heterogeneous_rows(
                        rows.iter().map(|row| match row {
                            TableRow::Columns { .. } => ROW_HEIGHT,
                            TableRow::Separator => ROW_SEPARATOR_HEIGHT,
                        }),
                        |mut row| {
                            let index = row.index();
                            for column in columns {
                                row.col(|ui| {
                                    match &rows[index] {
                                        TableRow::Columns { texts } => ui.strong(&texts[column]),
                                        TableRow::Separator => {
                                            ui.add(Separator::default().horizontal())
                                        }
                                    };
                                });
                            }
                        },
                    );
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
    const fn count_header(self) -> &'static str {
        match self {
            ViewMode::Recipe => "#",
            ViewMode::Configuration => "/sec",
            ViewMode::Speed => "/sec",
        }
    }

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
                    | TableColumn::Eu
                    | TableColumn::ProcessingTime
                    | TableColumn::TotalEu
            ],
            Self::Configuration => enum_set![
                TableColumn::Machine
                    | TableColumn::Catalysts
                    | TableColumn::Configuration
                    | TableColumn::Consumed
                    | TableColumn::ConsumedCount
                    | TableColumn::Produced
                    | TableColumn::ProducedCount
                    | TableColumn::Eu
            ],
            Self::Speed => enum_set![
                TableColumn::Machine
                    | TableColumn::Catalysts
                    | TableColumn::Configuration
                    | TableColumn::Speed
                    | TableColumn::Consumed
                    | TableColumn::ConsumedCount
                    | TableColumn::Produced
                    | TableColumn::ProducedCount
                    | TableColumn::Eu
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
    Configuration,
    Speed,
    Consumed,
    ConsumedCount,
    Produced,
    ProducedCount,
    Eu,
    ProcessingTime,
    TotalEu,
}

impl TableColumn {
    fn header(self, view_mode: ViewMode) -> &'static str {
        match self {
            TableColumn::Machine => "Machine",
            TableColumn::Catalysts => "Catalysts",
            TableColumn::Configuration => "Configuration",
            TableColumn::Speed => "Speed",
            TableColumn::Consumed => "Consumed",
            TableColumn::ConsumedCount => view_mode.count_header(),
            TableColumn::Produced => "Produced",
            TableColumn::ProducedCount => view_mode.count_header(),
            TableColumn::TotalEu => "Total EU",
            TableColumn::ProcessingTime => "Processing Time",
            TableColumn::Eu => "EU/tick",
        }
    }

    fn header_hover(self, view_mode: ViewMode) -> &'static str {
        match self {
            TableColumn::Machine => "The kind of machine processing this recipe.",
            TableColumn::Catalysts => "Products that are required but not consumed.",
            TableColumn::Consumed | TableColumn::ConsumedCount => match view_mode {
                ViewMode::Recipe => "Consumed products per processing cycle.",
                ViewMode::Configuration => "Consumed products by all machines.",
                ViewMode::Speed => "Consumed products at the current speed.",
            },
            TableColumn::Produced | TableColumn::ProducedCount => match view_mode {
                ViewMode::Recipe => "Produced products per processing cycle.",
                ViewMode::Configuration => "Produced procuts by all machines.",
                ViewMode::Speed => "Produced products at the current speed.",
            },
            TableColumn::TotalEu => "Total EU per processing cycle.",
            TableColumn::ProcessingTime => "Duration of a single processing cycle.",
            TableColumn::Eu => match view_mode {
                ViewMode::Recipe => "EU/t for a single machine without overclocking.",
                ViewMode::Configuration => "EU/t of all machines.",
                ViewMode::Speed => "EU/t at the current speed.",
            },
            TableColumn::Configuration => "The machines processing this recipe.",
            TableColumn::Speed => "How fast this machine can run.",
        }
    }

    fn table_builder_column(self) -> Column {
        match self {
            TableColumn::ConsumedCount | TableColumn::ProducedCount => Column::auto(),
            _ => Column::auto().resizable(true),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CachedProcessingChain {
    processing_chain: ProcessingChain,
    allow_overproduction: Vec<Product>,
    rows: EnumMap<ViewMode, OnceCell<Vec<TableRow>>>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum TableRow {
    Columns {
        texts: Box<EnumMap<TableColumn, String>>, // TODO: second string for hover_text
    },
    Separator,
}

impl TableRow {
    fn from_machine_configuration<'a>(
        view_mode: ViewMode,
        machine_configuration: &'a MachineConfiguration,
        speeds: &'a Speeds,
    ) -> impl Iterator<Item = Self> + 'a {
        let recipe = &machine_configuration.recipe;

        let mut first = true;
        let mut machines = machine_configuration.machines.iter();
        let mut catalysts = recipe.catalysts.iter();
        let mut consumed = recipe.consumed.iter();
        let mut produced = recipe.produced.iter();

        once(Self::Separator).chain(iter::from_fn(move || {
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
            .then(|| Self::Columns {
                texts: Box::new(EnumMap::from_fn(|column| match column {
                    TableColumn::Machine => first
                        .then(|| recipe.machine.name.clone())
                        .unwrap_or_default(),
                    TableColumn::Catalysts => catalyst
                        .map(|product| product.name.clone())
                        .unwrap_or_default(),
                    TableColumn::Configuration => machine
                        .map(|(overclocking, count)| format!("{} x{count}", overclocking.0))
                        .unwrap_or_default(),
                    TableColumn::Speed => first
                        .then(|| {
                            let speed = (speeds.machines[&machine_configuration.recipe] * 100)
                                .to_f64()
                                .unwrap();
                            format!("{speed:.1}%")
                        })
                        .unwrap_or_default(),
                    TableColumn::Consumed => consumed
                        .map(|(product, _)| product.name.clone())
                        .unwrap_or_default(),
                    TableColumn::ConsumedCount => consumed
                        .map(|(_, count)| {
                            format_count(
                                view_mode,
                                *count,
                                machine_configuration.speed_factor(),
                                speeds.machines[recipe],
                            )
                        })
                        .unwrap_or_default(),
                    TableColumn::Produced => produced
                        .map(|(product, _)| product.name.clone())
                        .unwrap_or_default(),
                    TableColumn::ProducedCount => produced
                        .map(|(_, count)| {
                            format_count(
                                view_mode,
                                *count,
                                machine_configuration.speed_factor(),
                                speeds.machines[recipe],
                            )
                        })
                        .unwrap_or_default(),
                    TableColumn::Eu => first
                        .then(|| {
                            format_eu(
                                view_mode,
                                recipe.eu_per_tick,
                                machine_configuration.eu_factor(),
                                speeds.machines[recipe],
                            )
                        })
                        .unwrap_or_default(),
                    TableColumn::ProcessingTime => first
                        .then(|| format!("{:.2} sec", (recipe.ticks as f64) / 20.0))
                        .unwrap_or_default(),
                    TableColumn::TotalEu => first
                        .then(|| recipe.total_eu().to_string())
                        .unwrap_or_default(),
                })),
            })
        }))
    }

    fn total(
        view_mode: ViewMode,
        speeds: &Speeds,
        processing_chain: &ProcessingChain,
    ) -> impl Iterator<Item = Self> {
        let products = match view_mode {
            ViewMode::Recipe => processing_chain.products(),
            ViewMode::Configuration => processing_chain.products_with_configuration(),
            ViewMode::Speed => processing_chain.products_with_speeds(speeds),
        };

        let mut first = true;

        once(Self::Separator).chain(iter::from_fn(move || {
            let first = replace(&mut first, false);

            first.then(|| Self::Columns {
                texts: Box::new(EnumMap::from_fn(|column| match column {
                    TableColumn::Machine => "Total".to_string(),
                    TableColumn::Catalysts => String::new(),
                    TableColumn::Configuration => String::new(),
                    TableColumn::Speed => String::new(),
                    TableColumn::Consumed => "TODO".to_string(),
                    TableColumn::ConsumedCount => "TODO".to_string(),
                    TableColumn::Produced => "TODO".to_string(),
                    TableColumn::ProducedCount => "TODO".to_string(),
                    TableColumn::TotalEu => "TODO".to_string(),
                    TableColumn::ProcessingTime => "TODO".to_string(),
                    TableColumn::Eu => "TODO".to_string(),
                })),
            })
        }))
    }
}

fn format_count(
    view_mode: ViewMode,
    count: NonZeroU64,
    configuration_speed_factor: Rational64,
    speed: Rational64,
) -> String {
    match view_mode {
        ViewMode::Recipe => count.to_string(),
        ViewMode::Configuration => {
            let count = (configuration_speed_factor * i64::try_from(count.get()).unwrap())
                .to_f64()
                .unwrap();
            format!("{count:.1}")
        }
        ViewMode::Speed => {
            let count = (configuration_speed_factor * i64::try_from(count.get()).unwrap() * speed)
                .to_f64()
                .unwrap();
            format!("{count:.1}")
        }
    }
}

fn format_eu(view_mode: ViewMode, eu: i64, eu_factor: Rational64, speed: Rational64) -> String {
    match view_mode {
        ViewMode::Recipe => format!("{eu}"),
        ViewMode::Configuration => {
            let eu = (eu_factor * eu).to_f64().unwrap();
            format!("{eu:.1}")
        }
        ViewMode::Speed => {
            let eu = (eu_factor * eu * speed).to_f64().unwrap();
            format!("{eu:.1}")
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
        self.rows = Default::default();
        &mut self.processing_chain
    }

    fn allow_overproduction(&self) -> &[Product] {
        &self.allow_overproduction
    }

    fn allow_overproduction_mut(&mut self) -> &mut Vec<Product> {
        self.rows[ViewMode::Speed].take();
        &mut self.allow_overproduction
    }

    fn rows(&self, view_mode: ViewMode) -> &[TableRow] {
        self.rows[view_mode].get_or_init(|| {
            let speeds = self
                .processing_chain
                .speeds(|product| self.allow_overproduction.contains(product));
            self.processing_chain
                .machines
                .iter()
                .flat_map(|machine_configuration| {
                    TableRow::from_machine_configuration(view_mode, machine_configuration, &speeds)
                })
                .chain(TableRow::total(view_mode, &speeds, &self.processing_chain))
                .collect::<Vec<_>>()
        })
    }
}
