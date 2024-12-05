use std::{
    cell::OnceCell,
    iter::{self, once, once_with},
    mem::take,
    num::NonZeroU64,
};

use egui::{Align, Layout, Response, Separator, Ui, Widget};
use egui_extras::{Column, TableBuilder};
use enum_map::{Enum, EnumMap};
use enumset::{enum_set, EnumSet, EnumSetType};
use itertools::Itertools;
use malachite::{
    num::{
        basic::traits::One,
        conversion::{string::options::ToSciOptions, traits::ToSci},
    },
    Rational,
};

use super::{ProcessingChain, Setup};
use crate::{
    machine::{ClockedMachine, MachinePowerError, Machines},
    recipe::ProductCount,
};

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
                    body.heterogeneous_rows(rows.iter().map(TableRow::height), |mut row| {
                        let index = row.index();
                        for column in columns {
                            row.col(|ui| {
                                match &rows[index] {
                                    TableRow::Cells(cells) => {
                                        cells[column]
                                        // ui.label(format!("{:?}", cells[column]))
                                    }
                                    TableRow::Separator => {
                                        ui.add(Separator::default().horizontal());
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
    Cells(Box<EnumMap<TableColumn, Option<TableCell>>>),
    Separator,
}

impl TableRow {
    fn height(&self) -> f32 {
        match self {
            TableRow::Cells(_) => ROW_HEIGHT,
            TableRow::Separator => ROW_SEPARATOR_HEIGHT,
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum TableCell {
    Setup {
        index: usize,
        content: SetupTableCellContent,
    },
    Total {
        content: TotalTableCellContent,
    },
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum SetupTableCellContent {
    Machine,
    Catalyst { index: usize },
    SetupEco,
    SetupPower { clocked_machine: ClockedMachine },
    Time,
    Speed,
    EuPerTick,
    Produced { index: usize },
    Consumed { index: usize },
    ProducedCount { index: usize },
    ConsumedCount { index: usize },
    ProductAmount(Box<Rational>),
    Error(MachinePowerError),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum TotalTableCellContent {
    Header,
    /// Can be modified, which updates the name in _all_ [`Setup`]s.
    Product(String),
    ProductAmount(Rational),
    EuPerTick(Rational),
}

impl TableRow {
    fn from_setup<'a>(
        view_mode: ViewMode,
        index: usize,
        setup: &'a Setup,
        speed: &'a Rational,
    ) -> impl Iterator<Item = Self> + 'a {
        let mut machine_col = once(SetupTableCellContent::Machine);

        let mut machines_col: Box<dyn Iterator<Item = _>> = match &setup.machines {
            Machines::Eco(_) => Box::new(once(SetupTableCellContent::SetupEco)),
            Machines::Power(clocked_machines) => Box::new(
                clocked_machines
                    .machines
                    .keys()
                    .map(|&clocked_machine| SetupTableCellContent::SetupPower { clocked_machine }),
            ),
        };

        // match &setup.machines {
        //     Machines::Eco(count) => Box::new(once(format!("🏭 ×{count}"))),
        //     Machines::Power(clocked_machines) => Box::new(clocked_machines.machines.iter().map(
        //         |(clocked_machine, count)| {
        //             let tier = clocked_machine.tier();
        //             let underclocking = clocked_machine.underclocking();
        //             if tier == underclocking {
        //                 format!("🏭{tier} ×{count}",)
        //             } else {
        //                 format!("🏭{tier}⤵{underclocking} ×{count}",)
        //             }
        //         },
        //     )),
        // };

        let mut catalysts_col = (0..setup.recipe.catalysts.len())
            .map(|index| SetupTableCellContent::Catalyst { index });

        let mut speed_col = once(SetupTableCellContent::Speed);
        // once_with(move || {
        //     let speed_percent = speed * Rational::from(100);
        //     let mut options = ToSciOptions::default();
        //     options.set_scale(2);
        //     format!("{}%", speed_percent.to_sci_with_options(options))
        // });

        const FULL_SPEED: &Rational = &Rational::ONE;

        let mut consumed_col =
            (0..setup.recipe.consumed.len()).map(|index| SetupTableCellContent::Consumed { index });
        let mut consumed_count_col: Box<dyn Iterator<Item = _>> = match view_mode {
            ViewMode::Recipe => Box::new(
                (0..setup.recipe.consumed.len())
                    .map(|index| SetupTableCellContent::ConsumedCount { index }),
            ),
            ViewMode::Setup => Box::new(product_amounts(&setup.recipe.consumed, setup, FULL_SPEED)),
            ViewMode::Speed => Box::new(product_amounts(&setup.recipe.consumed, setup, speed)),
        };

        let mut produced_col =
            (0..setup.recipe.produced.len()).map(|index| SetupTableCellContent::Produced { index });
        let mut produced_count_col: Box<dyn Iterator<Item = _>> = match view_mode {
            ViewMode::Recipe => Box::new(
                (0..setup.recipe.consumed.len())
                    .map(|index| SetupTableCellContent::ProducedCount { index }),
            ),
            ViewMode::Setup => Box::new(product_amounts(&setup.recipe.produced, setup, FULL_SPEED)),
            ViewMode::Speed => Box::new(product_amounts(&setup.recipe.produced, setup, speed)),
        };

        let mut time_col = once(SetupTableCellContent::Time);
        // once_with(|| {
        //     let mut options = ToSciOptions::default();
        //     options.set_scale(2);
        //     format!(
        //         "{} sec",
        //         setup.recipe.seconds().to_sci_with_options(options)
        //     )
        // });

        let mut eu_col = once(SetupTableCellContent::EuPerTick);
        // once_with(move || match view_mode {
        //     ViewMode::Recipe => {
        //         let eu = &setup.recipe.eu_per_tick;
        //         format!("{eu} EU/t")
        //     }
        //     ViewMode::Setup => match setup.machines.eu_per_tick(setup.recipe.eu_per_tick) {
        //         Ok(eu) => format!("{eu} EU/t"),
        //         Err(_) => "⚠".into(),
        //     },
        //     ViewMode::Speed => match setup.machines.eu_per_tick(setup.recipe.eu_per_tick) {
        //         Ok(eu) => {
        //             let eu = Rational::from(eu) * speed;
        //             let mut options = ToSciOptions::default();
        //             options.set_scale(2);
        //             format!("{} EU/t", eu.to_sci_with_options(options))
        //         }
        //         Err(_) => "⚠".into(),
        //     },
        // });

        once(Self::Separator).chain(iter::from_fn(move || {
            let cells = view_mode
                .columns()
                .into_iter()
                .map(|column| {
                    (
                        column,
                        match column {
                            TableColumn::Machine => machine_col.next(),
                            TableColumn::Setup => machines_col.next(),
                            TableColumn::Catalysts => catalysts_col.next(),
                            TableColumn::Speed => speed_col.next(),
                            TableColumn::Consumed => consumed_col.next(),
                            TableColumn::ConsumedCount => consumed_count_col.next(),
                            TableColumn::Produced => produced_col.next(),
                            TableColumn::ProducedCount => produced_count_col.next(),
                            TableColumn::Time => time_col.next(),
                            TableColumn::Eu => eu_col.next(),
                        },
                    )
                })
                .collect::<EnumMap<_, _>>();

            cells.values().any(|content| content.is_some()).then(|| {
                Self::Cells(Box::new(cells.map(|_, content| {
                    content.map(|content| TableCell::Setup { index, content })
                })))
            })
        }))
    }

    fn total(
        view_mode: ViewMode,
        processing_chain: &ProcessingChain,
    ) -> impl Iterator<Item = Self> {
        let products = match view_mode {
            ViewMode::Recipe => None,
            ViewMode::Setup => Some(processing_chain.products_with_max_speeds()),
            ViewMode::Speed => {
                Some(processing_chain.products_with_speeds(processing_chain.weighted_speeds()))
            }
        };

        products.into_iter().flat_map(move |products| {
            let mut machine_col = once(TotalTableCellContent::Header);

            let mut consumed_col = products
                .products_per_sec
                .clone()
                .into_iter()
                .filter(|(_, amount)| *amount < 0)
                .map(|(product, amount)| {
                    (
                        TotalTableCellContent::Product(product.name.clone()),
                        TotalTableCellContent::ProductAmount(-amount),
                    )
                });

            let mut produced_col = products
                .products_per_sec
                .into_iter()
                .filter(|(_, amount)| *amount > 0)
                .map(|(product, amount)| {
                    (
                        TotalTableCellContent::Product(product.name.clone()),
                        TotalTableCellContent::ProductAmount(amount),
                    )
                });

            let mut eu_col = once(TotalTableCellContent::EuPerTick(products.eu_per_tick));

            once(Self::Separator).chain(iter::from_fn(move || {
                let (mut consumed, mut consumed_amount) = consumed_col.next().unzip();
                let (mut produced, mut produced_amount) = produced_col.next().unzip();

                let cells = view_mode
                    .columns()
                    .into_iter()
                    .map(|column| {
                        (
                            column,
                            match column {
                                TableColumn::Machine => machine_col.next(),
                                TableColumn::Setup => None,
                                TableColumn::Catalysts => None,
                                TableColumn::Speed => None,
                                TableColumn::Consumed => consumed.take(),
                                TableColumn::ConsumedCount => consumed_amount.take(),
                                TableColumn::Produced => produced.take(),
                                TableColumn::ProducedCount => produced_amount.take(),
                                TableColumn::Time => None,
                                TableColumn::Eu => eu_col.next(),
                            },
                        )
                    })
                    .collect::<EnumMap<_, _>>();

                cells.values().any(|content| content.is_some()).then(|| {
                    Self::Cells(Box::new(cells.map(|_, content| {
                        content.map(|content| TableCell::Total { content })
                    })))
                })
            }))
        })
    }
}

fn product_amounts<'a>(
    product_counts: &'a [ProductCount],
    setup: &'a Setup,
    speed: &'a Rational,
) -> impl Iterator<Item = SetupTableCellContent> + 'a {
    product_counts.iter().map(move |product_count| {
        match setup.machines.speed_factor(setup.recipe.voltage()) {
            Ok(speed_factor) => SetupTableCellContent::ProductAmount(Box::new(
                Rational::from(product_count.count.get()) / setup.recipe.seconds()
                    * speed_factor
                    * speed,
            )),
            Err(error) => SetupTableCellContent::Error(error),
        }
    })
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
            self.processing_chain
                .setups
                .iter()
                .zip_eq(&self.processing_chain.weighted_speeds().speeds)
                .enumerate()
                .flat_map(|(index, (setup, speed))| {
                    TableRow::from_setup(view_mode, index, setup, speed)
                })
                .chain(TableRow::total(view_mode, &self.processing_chain))
                .collect::<Vec<_>>()
        })
    }
}
