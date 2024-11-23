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

use super::{ProcessingChain, Setup, Speeds};
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
                                        TableRow::Columns { texts } => ui.label(&texts[column]),
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
    Setup,
    Speed,
}

impl ViewMode {
    const fn count_header(self) -> &'static str {
        match self {
            ViewMode::Recipe => "#",
            ViewMode::Setup => "/sec",
            ViewMode::Speed => "/sec",
        }
    }

    const fn name(self) -> &'static str {
        match self {
            ViewMode::Recipe => "Recipe",
            ViewMode::Setup => "Setup",
            ViewMode::Speed => "Speed",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            ViewMode::Recipe => "Shows information about only the recipes.",
            ViewMode::Setup => "Shows information based on specific machine setup.",
            ViewMode::Speed => "Shows information based on the effective speed of machines.",
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
                    | TableColumn::ProcessingTime
                    | TableColumn::Eu
                    | TableColumn::TotalEu
            ],
            Self::Setup => enum_set![
                TableColumn::Machine
                    | TableColumn::Catalysts
                    | TableColumn::Setup
                    | TableColumn::Consumed
                    | TableColumn::ConsumedCount
                    | TableColumn::Produced
                    | TableColumn::ProducedCount
                    | TableColumn::Eu
            ],
            Self::Speed => enum_set![
                TableColumn::Machine
                    | TableColumn::Catalysts
                    | TableColumn::Setup
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
            for view_mode in [ViewMode::Recipe, ViewMode::Setup, ViewMode::Speed] {
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
    Setup,
    Speed,
    Consumed,
    ConsumedCount,
    Produced,
    ProducedCount,
    ProcessingTime,
    Eu,
    TotalEu,
}

impl TableColumn {
    fn header(self, view_mode: ViewMode) -> &'static str {
        match self {
            Self::Machine => "Machine",
            Self::Catalysts => "Catalysts",
            Self::Setup => "Setup",
            Self::Speed => "Speed",
            Self::Consumed => "Consumed",
            Self::ConsumedCount => view_mode.count_header(),
            Self::Produced => "Produced",
            Self::ProducedCount => view_mode.count_header(),
            Self::ProcessingTime => "Processing Time",
            Self::Eu => "EU/tick",
            Self::TotalEu => "Total EU",
        }
    }

    fn header_hover(self, view_mode: ViewMode) -> &'static str {
        match self {
            Self::Machine => "The kind of machine processing this recipe.",
            Self::Catalysts => "Products that are required but not consumed.",
            Self::Setup => "The machines processing this recipe.",
            Self::Speed => "How fast this machine can run.",
            Self::Consumed | Self::ConsumedCount => match view_mode {
                ViewMode::Recipe => "Consumed products per processing cycle.",
                ViewMode::Setup => "Consumed products by all machines.",
                ViewMode::Speed => "Consumed products at the current speed.",
            },
            Self::Produced | Self::ProducedCount => match view_mode {
                ViewMode::Recipe => "Produced products per processing cycle.",
                ViewMode::Setup => "Produced procuts by all machines.",
                ViewMode::Speed => "Produced products at the current speed.",
            },
            Self::ProcessingTime => "Duration of a single processing cycle.",
            Self::Eu => match view_mode {
                ViewMode::Recipe => "EU/t for a single machine without overclocking.",
                ViewMode::Setup => "EU/t of all machines.",
                ViewMode::Speed => "EU/t at the current speed.",
            },
            Self::TotalEu => "Total EU per processing cycle.",
        }
    }

    fn table_builder_column(self) -> Column {
        match self {
            Self::ConsumedCount | Self::ProducedCount => Column::auto(),
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
    fn from_setup<'a>(
        view_mode: ViewMode,
        setup: &'a Setup,
        speeds: &'a Speeds,
    ) -> impl Iterator<Item = Self> + 'a {
        let recipe = &setup.recipe;

        let mut first = true;
        let mut machines = setup.machines.iter();
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
                    TableColumn::Setup => machine
                        .map(|(overclocking, count)| {
                            if let Some(voltage) = recipe.voltage() {
                                let voltage = voltage.with_overclocking(*overclocking);
                                format!("{} x{count}", voltage)
                            } else {
                                format!("x{count}")
                            }
                        })
                        .unwrap_or_default(),
                    TableColumn::Speed => first
                        .then(|| {
                            let speed = (speeds.machines[&setup.recipe] * 100).to_f64().unwrap();
                            format!("{speed:.1}%")
                        })
                        .unwrap_or_default(),
                    TableColumn::Consumed => consumed
                        .map(|(product, _)| product.name.clone())
                        .unwrap_or_default(),
                    TableColumn::ConsumedCount => consumed
                        .map(|(_, count)| format_count(view_mode, *count, setup, speeds))
                        .unwrap_or_default(),
                    TableColumn::Produced => produced
                        .map(|(product, _)| product.name.clone())
                        .unwrap_or_default(),
                    TableColumn::ProducedCount => produced
                        .map(|(_, count)| format_count(view_mode, *count, setup, speeds))
                        .unwrap_or_default(),
                    TableColumn::ProcessingTime => first
                        .then(|| format!("{:.2} sec", recipe.seconds().to_f64().unwrap()))
                        .unwrap_or_default(),
                    TableColumn::Eu => first
                        .then(|| {
                            format_eu(
                                view_mode,
                                recipe.eu_per_tick,
                                setup.eu_factor(),
                                speeds.machines[recipe],
                            )
                        })
                        .unwrap_or_default(),
                    TableColumn::TotalEu => first
                        .then(|| recipe.total_eu().to_string())
                        .unwrap_or_default(),
                })),
            })
        }))
    }

    fn total<'a>(
        processing_chain: &'a ProcessingChain,
        speeds: &'a Speeds,
    ) -> impl Iterator<Item = Self> + 'a {
        let products = processing_chain.products(speeds);

        let mut first = true;
        let mut consumed = products
            .products
            .clone()
            .into_iter()
            .map(|(product, product_per_tick)| (product, product_per_tick.total()))
            .filter(|(_, total)| *total < Rational64::ZERO)
            .map(|(product, total)| (product, -total));
        let mut produced = products
            .products
            .into_iter()
            .map(|(product, product_per_tick)| (product, product_per_tick.total()))
            .filter(|(_, total)| *total > Rational64::ZERO);

        once(Self::Separator).chain(iter::from_fn(move || {
            let first = replace(&mut first, false);
            let consumed = consumed.next();
            let produced = produced.next();

            (first || consumed.is_some() || produced.is_some()).then(|| Self::Columns {
                texts: Box::new(EnumMap::from_fn(|column| match column {
                    TableColumn::Machine => first.then(|| "Total".to_string()).unwrap_or_default(),
                    TableColumn::Catalysts => String::new(),
                    TableColumn::Setup => String::new(),
                    TableColumn::Speed => String::new(),
                    TableColumn::Consumed => consumed
                        .as_ref()
                        .map(|(product, _)| product.name.clone())
                        .unwrap_or_default(),
                    TableColumn::ConsumedCount => consumed
                        .as_ref()
                        .map(|(_, count)| {
                            let count = count.to_f64().unwrap();
                            format!("{:.1}", count)
                        })
                        .unwrap_or_default(),
                    TableColumn::Produced => produced
                        .as_ref()
                        .map(|(product, _)| product.name.clone())
                        .unwrap_or_default(),
                    TableColumn::ProducedCount => produced
                        .as_ref()
                        .map(|(_, count)| {
                            let count = count.to_f64().unwrap();
                            format!("{:.1}", count)
                        })
                        .unwrap_or_default(),
                    TableColumn::ProcessingTime => String::new(),
                    TableColumn::Eu => first
                        .then(|| {
                            let eu = products.eu_per_tick.to_f64().unwrap();
                            format!("{eu:.1}")
                        })
                        .unwrap_or_default(),
                    TableColumn::TotalEu => String::new(),
                })),
            })
        }))
    }
}

fn format_count(view_mode: ViewMode, count: NonZeroU64, setup: &Setup, speeds: &Speeds) -> String {
    match view_mode {
        ViewMode::Recipe => count.to_string(),
        ViewMode::Setup => {
            let count = (setup.speed_factor() * i64::try_from(count.get()).unwrap()
                / setup.recipe.seconds())
            .to_f64()
            .unwrap();
            format!("{count:.1}")
        }
        ViewMode::Speed => {
            let count = (setup.speed_factor()
                * i64::try_from(count.get()).unwrap()
                * speeds.machines[&setup.recipe]
                / setup.recipe.seconds())
            .to_f64()
            .unwrap();
            format!("{count:.1}")
        }
    }
}

fn format_eu(view_mode: ViewMode, eu: i64, eu_factor: Rational64, speed: Rational64) -> String {
    match view_mode {
        ViewMode::Recipe => format!("{eu}"),
        ViewMode::Setup => {
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
                .flat_map(|setup| TableRow::from_setup(view_mode, setup, &speeds))
                .chain(
                    matches!(view_mode, ViewMode::Speed)
                        .then(|| TableRow::total(&self.processing_chain, &speeds))
                        .into_iter()
                        .flatten(),
                )
                .collect::<Vec<_>>()
        })
    }
}
