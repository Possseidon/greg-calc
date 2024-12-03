use std::{
    cell::OnceCell,
    iter::{self, once, once_with},
    mem::take,
};

use egui::{Align, Layout, Response, Separator, Ui, Widget};
use egui_extras::{Column, TableBuilder};
use enum_map::{Enum, EnumMap};
use enumset::{enum_set, EnumSet, EnumSetType};
use itertools::Itertools;
use malachite::{
    num::conversion::{string::options::ToSciOptions, traits::ToSci},
    Rational,
};

use super::{ProcessingChain, Setup, WeightedSpeeds};
use crate::{machine::Machines, recipe::ProductCount};

pub struct ProcessingChainViewer<'a> {
    view_mode: &'a mut ViewMode,
    processing_chain: &'a mut ProcessingChainTableRows,
}

impl<'a> ProcessingChainViewer<'a> {
    pub fn new(
        view_mode: &'a mut ViewMode,
        processing_chain: &'a mut ProcessingChainTableRows,
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
        ui.vertical_centered_justified(|ui| {
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
    const fn name(self) -> &'static str {
        match self {
            ViewMode::Recipe => "Recipe",
            ViewMode::Setup => "Setup",
            ViewMode::Speed => "Speed",
        }
    }

    const fn count_header(self) -> &'static str {
        match self {
            ViewMode::Recipe => "📦/🔄",
            ViewMode::Setup => "📦/sec",
            ViewMode::Speed => "📦/sec",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            ViewMode::Recipe => "Shows information about only the recipes.",
            ViewMode::Setup => "Shows information based on a specific machine setup.",
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
                    | TableColumn::Time
                    | TableColumn::Eu
            ],
            Self::Setup => enum_set![
                TableColumn::Machine
                    | TableColumn::Setup
                    | TableColumn::Catalysts
                    | TableColumn::Consumed
                    | TableColumn::ConsumedCount
                    | TableColumn::Produced
                    | TableColumn::ProducedCount
                    | TableColumn::Eu
            ],
            Self::Speed => enum_set![
                TableColumn::Machine
                    | TableColumn::Setup
                    | TableColumn::Catalysts
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
    Time,
    Eu,
    Consumed,
    ConsumedCount,
    Produced,
    ProducedCount,
}

impl TableColumn {
    fn header(self, view_mode: ViewMode) -> &'static str {
        match self {
            Self::Machine => "Machine 🏭",
            Self::Catalysts => "Catalysts 🔥",
            Self::Setup => "Setup 📜",
            Self::Speed => "Speed ⏱",
            Self::Consumed => "Consumed",
            Self::ConsumedCount => view_mode.count_header(),
            Self::Produced => "Produced",
            Self::ProducedCount => view_mode.count_header(),
            Self::Time => "Time 🔄",
            Self::Eu => "Power ⚡",
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
            Self::Time => "Duration of a single processing cycle.",
            Self::Eu => match view_mode {
                ViewMode::Recipe => "EU/t for a single machine at its minimum voltage.",
                ViewMode::Setup => "EU/t of all machines.",
                ViewMode::Speed => "EU/t at the current speed.",
            },
        }
    }

    fn table_builder_column(self) -> Column {
        match self {
            Self::Catalysts | Self::Eu | Self::ConsumedCount | Self::ProducedCount => {
                Column::auto()
            }
            _ => Column::auto().resizable(true),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProcessingChainTableRows {
    processing_chain: ProcessingChain,
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
        speed: &'a Rational,
    ) -> impl Iterator<Item = Self> + 'a {
        let mut machine_col = once_with(|| setup.recipe.machine.name.clone());

        let mut machines_col: Box<dyn Iterator<Item = _>> = match &setup.machines {
            Machines::Count(count) => Box::new(once(format!("🏭 ×{count}"))),
            Machines::Overclocked(clocked_machines) => Box::new(
                clocked_machines
                    .machines
                    .iter()
                    .map(|(clocked_machine, count)| {
                        let tier = clocked_machine.tier();
                        let underclocking = clocked_machine.underclocking();
                        if tier == underclocking {
                            format!("🏭{tier} ×{count}",)
                        } else {
                            format!("🏭{tier}⤵{underclocking} ×{count}",)
                        }
                    }),
            ),
        };

        let mut catalysts_col = setup
            .recipe
            .catalysts
            .iter()
            .map(|product| product.name.clone());

        let mut speed_col = once_with(move || {
            let speed_percent = speed * Rational::from(100);
            let mut options = ToSciOptions::default();
            options.set_scale(2);
            format!("{}%", speed_percent.to_sci_with_options(options))
        });

        let mut consumed_col = product_names(&setup.recipe.consumed);
        let mut produced_col = product_names(&setup.recipe.produced);

        let mut consumed_count_col = product_counts(
            view_mode,
            &setup.recipe.consumed,
            setup.recipe.seconds(),
            setup.machines.speed_factor(),
            speed,
        );
        let mut produced_count_col = product_counts(
            view_mode,
            &setup.recipe.produced,
            setup.recipe.seconds(),
            setup.machines.speed_factor(),
            speed,
        );

        let mut time_col = once_with(|| {
            let mut options = ToSciOptions::default();
            options.set_scale(2);
            format!(
                "{} sec",
                setup.recipe.seconds().to_sci_with_options(options)
            )
        });

        let mut eu_col = once_with(move || match &setup.machines {
            Machines::Count(_) => "".to_string(),
            Machines::Overclocked(overclocked_machines) => match view_mode {
                ViewMode::Recipe => {
                    let eu = &overclocked_machines.base_eu_per_tick;
                    format!("{eu} EU/t")
                }
                ViewMode::Setup => {
                    let eu = overclocked_machines.eu_per_tick();
                    format!("{eu} EU/t")
                }
                ViewMode::Speed => {
                    let eu = Rational::from(overclocked_machines.eu_per_tick()) * speed;
                    let mut options = ToSciOptions::default();
                    options.set_scale(2);
                    format!("{} EU/t", eu.to_sci_with_options(options))
                }
            },
        });

        once(Self::Separator).chain(iter::from_fn(move || {
            let texts = view_mode
                .columns()
                .into_iter()
                .map(|column| {
                    (
                        column,
                        match column {
                            TableColumn::Machine => machine_col.next().unwrap_or_default(),
                            TableColumn::Setup => machines_col.next().take().unwrap_or_default(),
                            TableColumn::Catalysts => catalysts_col.next().unwrap_or_default(),
                            TableColumn::Speed => speed_col.next().unwrap_or_default(),
                            TableColumn::Consumed => consumed_col.next().unwrap_or_default(),
                            TableColumn::ConsumedCount => {
                                consumed_count_col.next().unwrap_or_default()
                            }
                            TableColumn::Produced => produced_col.next().unwrap_or_default(),
                            TableColumn::ProducedCount => {
                                produced_count_col.next().unwrap_or_default()
                            }
                            TableColumn::Time => time_col.next().unwrap_or_default(),
                            TableColumn::Eu => eu_col.next().unwrap_or_default(),
                        },
                    )
                })
                .collect::<EnumMap<_, _>>();

            texts
                .values()
                .any(|text| !text.is_empty())
                .then(|| Self::Columns {
                    texts: Box::new(texts),
                })
        }))
    }

    fn total<'a>(
        view_mode: ViewMode,
        processing_chain: &'a ProcessingChain,
        weighted_speeds: &'a WeightedSpeeds,
    ) -> impl Iterator<Item = Self> + 'a {
        let products = match view_mode {
            ViewMode::Recipe => None,
            ViewMode::Setup => Some(processing_chain.products_with_max_speeds()),
            ViewMode::Speed => Some(processing_chain.products_with_speeds(weighted_speeds)),
        };

        products
            .map(move |products| {
                let mut machine_col = once_with(|| "Total".to_string());

                let mut consumed_col = products
                    .products_per_sec
                    .clone()
                    .into_iter()
                    .filter(|(_, amount)| *amount < 0)
                    .map(|(product, amount)| {
                        let mut options = ToSciOptions::default();
                        options.set_scale(2);
                        (
                            product.name.clone(),
                            (-amount).to_sci_with_options(options).to_string(),
                        )
                    });

                let mut produced_col = products
                    .products_per_sec
                    .into_iter()
                    .filter(|(_, amount)| *amount > 0)
                    .map(|(product, amount)| {
                        let mut options = ToSciOptions::default();
                        options.set_scale(2);
                        (
                            product.name.clone(),
                            amount.to_sci_with_options(options).to_string(),
                        )
                    });

                let mut eu_col = once_with(|| {
                    let eu = products.eu_per_tick;
                    let mut options = ToSciOptions::default();
                    options.set_scale(2);
                    format!("{} EU/t", eu.to_sci_with_options(options))
                });

                once(Self::Separator).chain(iter::from_fn(move || {
                    let (mut consumed, mut consumed_amount) =
                        consumed_col.next().unwrap_or_default();
                    let (mut produced, mut produced_amount) =
                        produced_col.next().unwrap_or_default();

                    let texts = view_mode
                        .columns()
                        .into_iter()
                        .map(|column| {
                            (
                                column,
                                match column {
                                    TableColumn::Machine => machine_col.next().unwrap_or_default(),
                                    TableColumn::Setup => String::new(),
                                    TableColumn::Catalysts => String::new(),
                                    TableColumn::Speed => String::new(),
                                    TableColumn::Consumed => take(&mut consumed),
                                    TableColumn::ConsumedCount => take(&mut consumed_amount),
                                    TableColumn::Produced => take(&mut produced),
                                    TableColumn::ProducedCount => take(&mut produced_amount),
                                    TableColumn::Time => String::new(),
                                    TableColumn::Eu => eu_col.next().unwrap_or_default(),
                                },
                            )
                        })
                        .collect::<EnumMap<_, _>>();

                    texts
                        .values()
                        .any(|text| !text.is_empty())
                        .then(|| Self::Columns {
                            texts: Box::new(texts),
                        })
                }))
            })
            .into_iter()
            .flatten()
    }
}

fn product_counts<'a>(
    view_mode: ViewMode,
    product_counts: &'a [ProductCount],
    seconds: Rational,
    speed_factor: Rational,
    speed: &'a Rational,
) -> impl Iterator<Item = String> + 'a {
    product_counts
        .iter()
        .map(|product_count| product_count.count)
        .map(move |count| match view_mode {
            ViewMode::Recipe => count.to_string(),
            ViewMode::Setup => {
                let amount = Rational::from(count.get()) / &seconds * &speed_factor;
                let mut options = ToSciOptions::default();
                options.set_scale(2);
                amount.to_sci_with_options(options).to_string()
            }
            ViewMode::Speed => {
                let amount = Rational::from(count.get()) / &seconds * &speed_factor * speed;
                let mut options = ToSciOptions::default();
                options.set_scale(2);
                amount.to_sci_with_options(options).to_string()
            }
        })
}

fn product_names(product_counts: &[ProductCount]) -> impl Iterator<Item = String> + '_ {
    product_counts
        .iter()
        .map(|product_count| &product_count.product.name)
        .cloned()
}

impl ProcessingChainTableRows {
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

    fn rows(&self, view_mode: ViewMode) -> &[TableRow] {
        self.rows[view_mode].get_or_init(|| {
            let weighted_speeds = self.processing_chain.weighted_speeds();
            self.processing_chain
                .setups
                .iter()
                .zip_eq(&weighted_speeds.speeds)
                .flat_map(|(setup, speed)| TableRow::from_setup(view_mode, setup, speed))
                .chain(TableRow::total(
                    view_mode,
                    &self.processing_chain,
                    weighted_speeds,
                ))
                .collect::<Vec<_>>()
        })
    }
}
