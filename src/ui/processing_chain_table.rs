use std::{
    cell::OnceCell,
    cmp::Ordering,
    iter::{self, once, once_with},
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

use crate::model::{
    machine::{ClockedMachine, Machines},
    processing_chain::{ProcessingChain, Setup},
    recipe::{Machine, Product, ProductCount},
};

const HEADER_HEIGHT: f32 = 30.0;
const ROW_HEIGHT: f32 = 20.0;
const ROW_SEPARATOR_HEIGHT: f32 = 7.0;

#[derive(Clone, Debug, Default)]
pub struct ProcessingChainTable {
    processing_chain: ProcessingChain,
    rows: EnumMap<ViewMode, OnceCell<Vec<TableRow>>>,
}

impl ProcessingChainTable {
    pub fn new(processing_chain: ProcessingChain) -> Self {
        Self {
            processing_chain,
            ..Default::default()
        }
    }

    pub fn show(&mut self, view_mode: ViewMode, ui: &mut Ui) {
        let columns = view_mode.columns();
        let mut table_builder = TableBuilder::new(ui)
            .id_salt(view_mode)
            .cell_layout(Layout::right_to_left(Align::Center))
            .striped(true);

        for column in columns {
            table_builder = table_builder.column(column.table_builder_column());
        }

        let mut action = None;

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
                let rows = self.rows(view_mode);
                body.heterogeneous_rows(rows.iter().map(TableRow::height), |mut row| {
                    let index = row.index();
                    for column in columns {
                        row.col(|ui| {
                            match &rows[index] {
                                TableRow::Cells(cells) => {
                                    if let Some(cell) = &cells[column] {
                                        if let Some(new_action) =
                                            cell.show(ui, &self.processing_chain)
                                        {
                                            if action.is_none() {
                                                action = Some(new_action);
                                            }
                                        };
                                    }
                                }
                                TableRow::Separator => {
                                    ui.add(Separator::default().horizontal());
                                }
                            };
                        });
                    }
                });
            });

        if let Some(action) = action {
            action.execute(&mut self.processing_chain);
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
                .setups()
                .iter()
                .zip_eq(self.processing_chain.weighted_speeds().speeds())
                .enumerate()
                .flat_map(|(index, (setup, speed))| {
                    TableRow::from_setup(view_mode, index, setup, speed)
                })
                .chain(TableRow::total(view_mode, &self.processing_chain))
                .collect::<Vec<_>>()
        })
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

        let mut catalysts_col = (0..setup.recipe.catalysts.len())
            .map(|index| SetupTableCellContent::Catalyst { index });

        let mut speed_col = once(SetupTableCellContent::Speed);

        const FULL_SPEED: &Rational = &Rational::ONE;

        let mut consumed_col =
            (0..setup.recipe.consumed.len()).map(|index| SetupTableCellContent::Consumed { index });
        let mut consumed_count_col: Box<dyn Iterator<Item = _>> = match view_mode {
            ViewMode::Recipe => Box::new(
                (0..setup.recipe.consumed.len())
                    .map(|index| SetupTableCellContent::ConsumedCount { index }),
            ),
            ViewMode::Setup => Box::new(SetupTableCellContent::product_amounts(
                &setup.recipe.consumed,
                setup,
                FULL_SPEED,
            )),
            ViewMode::Speed => Box::new(SetupTableCellContent::product_amounts(
                &setup.recipe.consumed,
                setup,
                speed,
            )),
        };

        let mut produced_col =
            (0..setup.recipe.produced.len()).map(|index| SetupTableCellContent::Produced { index });
        let mut produced_count_col: Box<dyn Iterator<Item = _>> = match view_mode {
            ViewMode::Recipe => Box::new(
                (0..setup.recipe.produced.len())
                    .map(|index| SetupTableCellContent::ProducedCount { index }),
            ),
            ViewMode::Setup => Box::new(SetupTableCellContent::product_amounts(
                &setup.recipe.produced,
                setup,
                FULL_SPEED,
            )),
            ViewMode::Speed => Box::new(SetupTableCellContent::product_amounts(
                &setup.recipe.produced,
                setup,
                speed,
            )),
        };

        let mut time_col = once(SetupTableCellContent::Time);

        let mut eu_col = once_with(move || match view_mode {
            ViewMode::Recipe => SetupTableCellContent::EuPerTickRecipe,
            ViewMode::Setup => match setup.machines.eu_per_tick(setup.recipe.eu_per_tick) {
                Ok(eu) => SetupTableCellContent::EuPerTick(Box::new(eu.into())),
                Err(_) => SetupTableCellContent::PowerError,
            },
            ViewMode::Speed => match setup.machines.eu_per_tick(setup.recipe.eu_per_tick) {
                Ok(eu) => SetupTableCellContent::EuPerTick(Box::new(Rational::from(eu) * speed)),
                Err(_) => SetupTableCellContent::PowerError,
            },
        });

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
                        TotalTableCellContent::ProductAmount(Box::new(-amount)),
                    )
                });

            let mut produced_col = products
                .products_per_sec
                .into_iter()
                .filter(|(_, amount)| *amount > 0)
                .map(|(product, amount)| {
                    (
                        TotalTableCellContent::Product(product.name.clone()),
                        TotalTableCellContent::ProductAmount(Box::new(amount)),
                    )
                });

            let mut eu_col =
                once_with(|| TotalTableCellContent::EuPerTick(Box::new(products.eu_per_tick)));

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

impl TableCell {
    fn show(&self, ui: &mut Ui, processing_chain: &ProcessingChain) -> Option<Action> {
        match self {
            Self::Setup { index, content } => content.show(
                ui,
                &processing_chain.setups()[*index],
                &processing_chain.weighted_speeds().speeds()[*index],
            ),
            Self::Total { content } => content.show(ui),
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum SetupTableCellContent {
    Machine,
    Catalyst { index: usize },
    SetupEco,
    SetupPower { clocked_machine: ClockedMachine },
    Time,
    Speed,
    EuPerTickRecipe,
    EuPerTick(Box<Rational>),
    Produced { index: usize },
    Consumed { index: usize },
    ProducedCount { index: usize },
    ConsumedCount { index: usize },
    ProductAmount(Box<Rational>),
    PowerError,
}

impl SetupTableCellContent {
    fn product_amounts<'a>(
        product_counts: &'a [ProductCount],
        setup: &'a Setup,
        speed: &'a Rational,
    ) -> impl Iterator<Item = Self> + 'a {
        product_counts.iter().map(move |product_count| {
            match setup.machines.speed_factor(setup.recipe.voltage()) {
                Ok(speed_factor) => Self::ProductAmount(Box::new(
                    Rational::from(product_count.count.get()) / setup.recipe.seconds()
                        * speed_factor
                        * speed,
                )),
                Err(_) => Self::PowerError,
            }
        })
    }

    fn show(&self, ui: &mut Ui, setup: &Setup, speed: &Rational) -> Option<Action> {
        match self {
            Self::Machine => {
                ui.label(&setup.recipe.machine.name);
            }
            Self::Catalyst { index } => {
                ui.label(&setup.recipe.catalysts[*index].name);
            }
            Self::SetupEco => {
                if let Machines::Eco(count) = setup.machines {
                    ui.label(format!("🏭 ×{count}"));
                } else {
                    unreachable!();
                }
            }
            Self::SetupPower { clocked_machine } => {
                if let Machines::Power(clocked_machines) = &setup.machines {
                    let count = clocked_machines.machines[clocked_machine];
                    let tier = clocked_machine.tier();
                    let underclocking = clocked_machine.underclocking();
                    if tier == underclocking {
                        ui.label(format!("🏭{tier} ×{count}"));
                    } else {
                        ui.label(format!("🏭{tier}⤵{underclocking} ×{count}"));
                    }
                } else {
                    unreachable!();
                }
            }
            Self::Time => {
                let mut options = ToSciOptions::default();
                options.set_scale(2);
                ui.label(format!(
                    "{} sec",
                    setup.recipe.seconds().to_sci_with_options(options)
                ));
            }
            Self::Speed => {
                let speed_percent = speed * Rational::from(100);
                let mut options = ToSciOptions::default();
                options.set_scale(2);
                ui.label(format!("{}%", speed_percent.to_sci_with_options(options)));
            }
            Self::EuPerTickRecipe => {
                ui.label(format!("{} EU/t", setup.recipe.eu_per_tick));
            }
            Self::EuPerTick(eu) => {
                let mut options = ToSciOptions::default();
                options.set_scale(2);
                ui.label(format!("{} EU/t", eu.to_sci_with_options(options)));
            }
            Self::Produced { index } => {
                ui.label(&setup.recipe.produced[*index].product.name);
            }
            Self::Consumed { index } => {
                ui.label(&setup.recipe.consumed[*index].product.name);
            }
            Self::ProducedCount { index } => {
                let count = setup.recipe.produced[*index].count;
                ui.label(format!("×{count}"));
            }
            Self::ConsumedCount { index } => {
                let count = setup.recipe.consumed[*index].count;
                ui.label(format!("×{count}"));
            }
            Self::ProductAmount(amount) => {
                let mut options = ToSciOptions::default();
                options.set_scale(2);
                ui.label(amount.to_sci_with_options(options).to_string())
                    .on_hover_ui(|ui| {
                        ui.label(amount.to_string());
                    });
            }
            Self::PowerError => {
                ui.label("⚠")
                    .on_hover_text(match setup.recipe.eu_per_tick.cmp(&0) {
                        Ordering::Less => "This recipe requires a machine that consumes power.",
                        Ordering::Equal => "This recipe requires machine without voltage.",
                        Ordering::Greater => "This recipe requires a machine that produces power.",
                    });
            }
        }

        None
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum TotalTableCellContent {
    Header,
    /// Can be modified, which updates the name in _all_ [`Setup`]s.
    Product(String),
    ProductAmount(Box<Rational>),
    EuPerTick(Box<Rational>),
}

impl TotalTableCellContent {
    fn show(&self, ui: &mut Ui) -> Option<Action> {
        match self {
            Self::Header => {
                ui.label("Total");
            }
            Self::Product(product) => {
                ui.label(product);
            }
            Self::ProductAmount(amount) => {
                let mut options = ToSciOptions::default();
                options.set_scale(2);
                ui.label(amount.to_sci_with_options(options).to_string());
            }
            Self::EuPerTick(eu) => {
                let mut options = ToSciOptions::default();
                options.set_scale(2);
                ui.label(format!("{} EU/t", eu.to_sci_with_options(options)));
            }
        }

        None
    }
}

enum Action {
    SetMachine {
        setup_index: usize,
        machine: Machine,
    },
    SetProduced {
        setup_index: usize,
        product_index: usize,
        product: Product,
    },
    SetProducedCount {
        setup_index: usize,
        product_index: usize,
        count: NonZeroU64,
    },
    SetConsumed {
        setup_index: usize,
        product_index: usize,
        product: Product,
    },
    SetConsumedCount {
        setup_index: usize,
        product_index: usize,
        count: NonZeroU64,
    },
    ReplaceProduct {
        old: Product,
        new: Product,
    },
}

impl Action {
    fn execute(self, processing_chain: &mut ProcessingChain) {
        match self {
            Self::SetMachine {
                setup_index,
                machine,
            } => {
                processing_chain.setups_mut()[setup_index].recipe.machine = machine;
            }
            Self::SetProduced {
                setup_index,
                product_index,
                product,
            } => {
                processing_chain.setups_mut()[setup_index].recipe.produced[product_index].product =
                    product;
            }
            Self::SetProducedCount {
                setup_index,
                product_index,
                count,
            } => {
                processing_chain.setups_mut()[setup_index].recipe.produced[product_index].count =
                    count;
            }
            Self::SetConsumed {
                setup_index,
                product_index,
                product,
            } => {
                processing_chain.setups_mut()[setup_index].recipe.consumed[product_index].product =
                    product;
            }
            Self::SetConsumedCount {
                setup_index,
                product_index,
                count,
            } => {
                processing_chain.setups_mut()[setup_index].recipe.consumed[product_index].count =
                    count;
            }
            Self::ReplaceProduct { old, new } => {
                processing_chain.replace_product(&old, new);
            }
        }
    }
}
