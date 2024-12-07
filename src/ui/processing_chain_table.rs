use std::{
    cell::OnceCell,
    cmp::Ordering,
    iter::{self, once, once_with, repeat_n},
    num::NonZeroU64,
};

use egui::{
    text::{CCursor, CCursorRange},
    Align, DragValue, Layout, Response, Separator, TextEdit, Ui, Widget,
};
use egui_extras::{Column, TableBuilder};
use enum_map::{Enum, EnumMap};
use enumset::{enum_set, EnumSet, EnumSetType};
use itertools::Itertools;
use log::debug;
use malachite::{
    num::{
        basic::traits::{One, Zero},
        conversion::{string::options::ToSciOptions, traits::ToSci},
    },
    Rational,
};

use crate::model::{
    machine::{ClockedMachine, Machines},
    processing_chain::{ProcessingChain, Setup},
    recipe::{Machine, Product, ProductCount, Recipe},
};

const HEADER_HEIGHT: f32 = 30.0;
const ROW_HEIGHT: f32 = 20.0;
const ROW_SEPARATOR_HEIGHT: f32 = 7.0;

#[derive(Clone, Debug, Default)]
pub struct ProcessingChainTable {
    processing_chain: ProcessingChain,
    rows: EnumMap<ViewMode, OnceCell<Vec<TableRow>>>,
    editing_cell: Option<((TableColumn, usize), Option<EditingBuffer>)>,
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
                        ui.heading(column.header())
                            .on_hover_text(column.header_hover(view_mode));
                    });
                }
            })
            .body(|body| {
                let rows = Self::rows(&self.rows, &self.processing_chain, view_mode);
                body.heterogeneous_rows(rows.iter().map(TableRow::height), |mut row| {
                    let row_index = row.index();
                    for column in columns {
                        row.col(|ui| {
                            match &rows[row_index] {
                                TableRow::Cells(cells) => {
                                    if let Some(cell) = &cells[column] {
                                        let cell_pos = (column, row_index);

                                        let mut tmp_editing_buffer = None;
                                        let editing_buffer = match &mut self.editing_cell {
                                            Some((editing_cell_pos, editing_buffer))
                                                if *editing_cell_pos == cell_pos =>
                                            {
                                                editing_buffer
                                            }
                                            _ => &mut tmp_editing_buffer,
                                        };

                                        if let Some(new_action) =
                                            cell.show(ui, &self.processing_chain, editing_buffer)
                                        {
                                            action.get_or_insert(new_action);
                                        };

                                        if tmp_editing_buffer.is_some() {
                                            self.editing_cell =
                                                Some((cell_pos, tmp_editing_buffer));
                                        }
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
            for view_mode in action.execute(&mut self.processing_chain) {
                self.rows[view_mode] = Default::default();
            }
        }
    }

    fn processing_chain(&self) -> &ProcessingChain {
        &self.processing_chain
    }

    fn processing_chain_mut(&mut self) -> &mut ProcessingChain {
        self.rows = Default::default();
        &mut self.processing_chain
    }

    fn rows<'a>(
        rows: &'a EnumMap<ViewMode, OnceCell<Vec<TableRow>>>,
        processing_chain: &ProcessingChain,
        view_mode: ViewMode,
    ) -> &'a [TableRow] {
        rows[view_mode].get_or_init(|| {
            let count = processing_chain.setups().len();
            debug!("Building {view_mode:?} table rows for {count} setups.");

            let unthrottled_speed = Rational::ONE;
            let speeds: &mut dyn Iterator<Item = _> = match view_mode {
                ViewMode::Recipe | ViewMode::Setup => &mut repeat_n(&unthrottled_speed, count),
                ViewMode::Speed => &mut processing_chain.weighted_speeds().speeds().iter(),
            };

            processing_chain
                .setups()
                .iter()
                .zip_eq(speeds)
                .enumerate()
                .flat_map(|(index, (setup, speed))| {
                    TableRow::from_setup(view_mode, index, setup, speed)
                })
                .chain(TableRow::total(view_mode, processing_chain))
                .collect::<Vec<_>>()
        })
    }
}

/// The mode at which the [`ProcessingChain`] is viewed.
#[derive(Debug, Hash, PartialOrd, Ord, Enum, EnumSetType)]
pub enum ViewMode {
    Recipe,
    Setup,
    Speed,
}

impl ViewMode {
    const NONE: EnumSet<Self> = EnumSet::empty();
    const CALCULATED: EnumSet<Self> = enum_set![ViewMode::Setup | ViewMode::Speed];
    const ALL: EnumSet<Self> = EnumSet::all();

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

        let mut consumed_col =
            (0..setup.recipe.consumed.len()).map(|index| SetupTableCellContent::Consumed { index });
        let mut consumed_count_col: Box<dyn Iterator<Item = _>> =
            match view_mode {
                ViewMode::Recipe => Box::new(
                    (0..setup.recipe.consumed.len())
                        .map(|index| SetupTableCellContent::ConsumedCount { index }),
                ),
                ViewMode::Setup | ViewMode::Speed => Box::new(
                    SetupTableCellContent::product_amounts(&setup.recipe.consumed, setup, speed),
                ),
            };

        let mut produced_col =
            (0..setup.recipe.produced.len()).map(|index| SetupTableCellContent::Produced { index });
        let mut produced_count_col: Box<dyn Iterator<Item = _>> =
            match view_mode {
                ViewMode::Recipe => Box::new(
                    (0..setup.recipe.produced.len())
                        .map(|index| SetupTableCellContent::ProducedCount { index }),
                ),
                ViewMode::Setup | ViewMode::Speed => Box::new(
                    SetupTableCellContent::product_amounts(&setup.recipe.produced, setup, speed),
                ),
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
            ViewMode::Setup => Some(processing_chain.products_with_unthrottled_speeds()),
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
                        TotalTableCellContent::Product(product),
                        TotalTableCellContent::ProductAmount(Box::new(-amount)),
                    )
                });

            let mut produced_col = products
                .products_per_sec
                .into_iter()
                .filter(|(_, amount)| *amount > 0)
                .map(|(product, amount)| {
                    (
                        TotalTableCellContent::Product(product),
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
    fn header(self) -> &'static str {
        match self {
            Self::Machine => "Machine 🏭",
            Self::Catalysts => "Catalysts 🔥",
            Self::Setup => "Setup 📜",
            Self::Speed => "Speed ⏱",
            Self::Consumed => "Consumed",
            Self::ConsumedCount => "📦",
            Self::Produced => "Produced",
            Self::ProducedCount => "📦",
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
    fn show(
        &self,
        ui: &mut Ui,
        processing_chain: &ProcessingChain,
        editing_buffer: &mut Option<EditingBuffer>,
    ) -> Option<Action> {
        match self {
            Self::Setup { index, content } => content
                .show(
                    &processing_chain.setups()[*index],
                    || &processing_chain.weighted_speeds().speeds()[*index],
                    editing_buffer,
                    ui,
                )
                .map(|property| Action::SetupProperty {
                    index: *index,
                    property,
                }),
            Self::Total { content } => {
                content.show(ui);
                None
            }
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

    fn show<'a>(
        &self,
        setup: &'a Setup,
        speed: impl FnOnce() -> &'a Rational,
        editing_buffer: &mut Option<EditingBuffer>,
        ui: &mut Ui,
    ) -> Option<SetupProperty> {
        match self {
            Self::Machine => editable_machine(&setup.recipe.machine, editing_buffer, ui),
            Self::Catalyst { index } => editable_product(
                &setup.recipe.catalysts[*index],
                editing_buffer,
                *index,
                ProductKind::Catalyst,
                ui,
            ),
            Self::SetupEco => {
                if let Machines::Eco(count) = setup.machines {
                    ui.label(format!("🏭 ×{count}"));
                    None
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
                    None
                } else {
                    unreachable!();
                }
            }
            Self::Time => editable_time(&setup.recipe, ui),
            Self::Speed => {
                let speed_percent = speed() * Rational::from(100);
                let mut options = ToSciOptions::default();
                options.set_scale(2);
                ui.label(format!("{}%", speed_percent.to_sci_with_options(options)));
                None
            }
            Self::EuPerTickRecipe => editable_eu_per_tick(setup.recipe.eu_per_tick, ui),
            Self::EuPerTick(eu) => {
                eu_per_tick(ui, eu);
                None
            }
            Self::Produced { index } => editable_product(
                &setup.recipe.produced[*index].product,
                editing_buffer,
                *index,
                ProductKind::Produced,
                ui,
            ),
            Self::Consumed { index } => editable_product(
                &setup.recipe.consumed[*index].product,
                editing_buffer,
                *index,
                ProductKind::Consumed,
                ui,
            ),
            Self::ProducedCount { index } => {
                editable_count(setup.recipe.produced[*index].count, ui, |count| {
                    SetupProperty::SetProducedCount {
                        index: *index,
                        count,
                    }
                })
            }
            Self::ConsumedCount { index } => {
                editable_count(setup.recipe.consumed[*index].count, ui, |count| {
                    SetupProperty::SetConsumedCount {
                        index: *index,
                        count,
                    }
                })
            }
            Self::ProductAmount(amount) => {
                product_amount(ui, amount);
                None
            }
            Self::PowerError => {
                ui.label("⚠")
                    .on_hover_text(match setup.recipe.eu_per_tick.cmp(&0) {
                        Ordering::Less => "This recipe requires a machine that consumes power.",
                        Ordering::Equal => "This recipe requires machine without voltage.",
                        Ordering::Greater => "This recipe requires a machine that produces power.",
                    });
                None
            }
        }
    }
}

fn editable_eu_per_tick(eu_per_tick: i64, ui: &mut Ui) -> Option<SetupProperty> {
    let mut new_eu_per_tick = eu_per_tick;
    ui.add(DragValue::new(&mut new_eu_per_tick).suffix(" EU/t"));
    (new_eu_per_tick != eu_per_tick).then_some(SetupProperty::SetEuPerTick {
        eu_per_tick: new_eu_per_tick,
    })
}

fn eu_per_tick(ui: &mut Ui, eu: &Rational) {
    let mut options = ToSciOptions::default();
    options.set_scale(2);
    ui.label(format!("{} EU/t", eu.to_sci_with_options(options)))
        .on_hover_ui(|ui| {
            ui.set_max_width(ui.spacing().tooltip_width);
            let dir = match eu.cmp(&Rational::ZERO) {
                Ordering::Less => "Consumes",
                Ordering::Equal => {
                    ui.label("Neither consumes nor produces EU.");
                    return;
                }
                Ordering::Greater => "Produces",
            };
            let (eu, ticks) = eu.numerator_and_denominator_ref();
            ui.label(format!("{dir} {eu} EU / {ticks} ticks"));
        });
}

fn editable_machine(
    machine: &Machine,
    editing_buffer: &mut Option<EditingBuffer>,
    ui: &mut Ui,
) -> Option<SetupProperty> {
    ui.label(&machine.name);
    None
}

fn editable_product(
    product: &Product,
    editing_buffer: &mut Option<EditingBuffer>,
    index: usize,
    kind: ProductKind,
    ui: &mut Ui,
) -> Option<SetupProperty> {
    if let Some(EditingBuffer { just_opened, text }) = editing_buffer {
        let mut edit = TextEdit::singleline(text).show(ui);
        if *just_opened {
            *just_opened = false;
            edit.state.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::default(),
                CCursor::new(text.chars().count()),
            )));
            edit.state.store(ui.ctx(), edit.response.id);
            edit.response.request_focus();
        }

        if edit.response.lost_focus() || edit.response.clicked_elsewhere() {
            let new_product_name = editing_buffer.take().expect("should be set").text;
            let trimmed_product_name = new_product_name.trim();
            if trimmed_product_name.is_empty() {
                return Some(SetupProperty::RemoveProduct { kind, index });
            }

            if trimmed_product_name != product.name {
                return Some(SetupProperty::RenameProduct {
                    kind,
                    index,
                    product: Product {
                        name: if trimmed_product_name.len() == new_product_name.len() {
                            new_product_name
                        } else {
                            trimmed_product_name.to_string()
                        },
                    },
                });
            }
        }
    } else {
        let label = ui.label(&product.name);
        if label.clicked() {
            *editing_buffer = Some(EditingBuffer {
                just_opened: true,
                text: product.name.to_string(),
            });
        }
        let mut action = None;
        label.context_menu(|ui| {
            if ui.button("Remove").clicked() {
                ui.close_menu();
                action = Some(SetupProperty::RemoveProduct { kind, index });
            }
        });
        if action.is_some() {
            return action;
        }
    }

    None
}

fn editable_count(
    count: NonZeroU64,
    ui: &mut Ui,
    into_property: impl FnOnce(NonZeroU64) -> SetupProperty,
) -> Option<SetupProperty> {
    let mut new_count = count;
    ui.add(DragValue::new(&mut new_count).prefix("×"));
    (new_count != count).then(|| into_property(new_count))
}

fn product_amount(ui: &mut Ui, amount: &Rational) {
    let mut options = ToSciOptions::default();
    options.set_scale(2);
    ui.label(format!("{}/s", amount.to_sci_with_options(options)))
        .on_hover_ui(|ui| {
            ui.set_max_width(ui.spacing().tooltip_width);
            let (count, sec) = amount.numerator_and_denominator_ref();
            ui.label(format!("{count} 📦 / {sec} s"));
        });
}

fn editable_time(recipe: &Recipe, ui: &mut Ui) -> Option<SetupProperty> {
    let mut ticks = recipe.ticks;
    ui.add(
        DragValue::new(&mut ticks)
            .custom_parser(|text| text.parse::<f64>().ok().map(|value| value * 20.0))
            .custom_formatter(|value, _| (value / 20.0).to_string())
            .suffix(" s"),
    );
    (ticks != recipe.ticks).then_some(SetupProperty::SetTime { ticks })
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum TotalTableCellContent {
    Header,
    /// Can be modified, which updates the name in _all_ [`Setup`]s.
    Product(Product),
    ProductAmount(Box<Rational>),
    EuPerTick(Box<Rational>),
}

impl TotalTableCellContent {
    fn show(&self, ui: &mut Ui) {
        match self {
            Self::Header => {
                ui.label("Total");
            }
            Self::Product(product) => {
                ui.label(&product.name);
            }
            Self::ProductAmount(amount) => {
                product_amount(ui, amount);
            }
            Self::EuPerTick(eu) => eu_per_tick(ui, eu),
        }
    }
}

enum Action {
    SetupProperty {
        index: usize,
        property: SetupProperty,
    },
    ReplaceProduct {
        old: Product,
        new: Product,
    },
}

impl Action {
    /// Performs the action on the given `processing_chain`.
    ///
    /// Returns which cached [`ProcessingChainTable::rows`] need to be invalidated.
    fn execute(self, processing_chain: &mut ProcessingChain) -> EnumSet<ViewMode> {
        match self {
            Action::SetupProperty { index, property } => property.apply(processing_chain, index),
            Action::ReplaceProduct { old, new } => {
                processing_chain.replace_product(&old, new);
                ViewMode::ALL
            }
        }
    }
}

enum ProductKind {
    Catalyst,
    Consumed,
    Produced,
}

enum SetupProperty {
    SetMachine {
        machine: Machine,
    },

    AddProduct {
        kind: ProductKind,
        product: Product,
    },
    RemoveProduct {
        kind: ProductKind,
        index: usize,
    },
    MoveProduct {
        kind: ProductKind,
        from: usize,
        to: usize,
    },
    RenameProduct {
        kind: ProductKind,
        index: usize,
        product: Product,
    },

    SetProducedCount {
        index: usize,
        count: NonZeroU64,
    },
    SetConsumedCount {
        index: usize,
        count: NonZeroU64,
    },

    SetTime {
        ticks: NonZeroU64,
    },
    SetEuPerTick {
        eu_per_tick: i64,
    },
}

impl SetupProperty {
    fn apply(
        self,
        processing_chain: &mut ProcessingChain,
        setup_index: usize,
    ) -> EnumSet<ViewMode> {
        match self {
            Self::SetMachine { machine } => {
                *processing_chain.machine_mut(setup_index) = machine;
                ViewMode::NONE
            }
            Self::AddProduct { kind, product } => {
                match kind {
                    ProductKind::Catalyst => {
                        processing_chain.catalysts_mut(setup_index).push(product);
                    }
                    ProductKind::Consumed => processing_chain.setups_mut()[setup_index]
                        .recipe
                        .consumed
                        .push(ProductCount {
                            product,
                            count: NonZeroU64::MIN,
                        }),
                    ProductKind::Produced => processing_chain.setups_mut()[setup_index]
                        .recipe
                        .produced
                        .push(ProductCount {
                            product,
                            count: NonZeroU64::MIN,
                        }),
                }
                ViewMode::ALL
            }
            Self::RemoveProduct { kind, index } => {
                match kind {
                    ProductKind::Catalyst => {
                        processing_chain.catalysts_mut(setup_index).remove(index);
                    }
                    ProductKind::Consumed => {
                        processing_chain.setups_mut()[setup_index]
                            .recipe
                            .consumed
                            .remove(index);
                    }
                    ProductKind::Produced => {
                        processing_chain.setups_mut()[setup_index]
                            .recipe
                            .produced
                            .remove(index);
                    }
                }
                ViewMode::ALL
            }
            Self::MoveProduct { kind, from, to } => {
                match kind {
                    ProductKind::Catalyst => {
                        move_item(processing_chain.catalysts_mut(setup_index), from, to);
                    }
                    ProductKind::Consumed => {
                        move_item(
                            &mut processing_chain.setups_mut()[setup_index].recipe.consumed,
                            from,
                            to,
                        );
                    }
                    ProductKind::Produced => {
                        move_item(
                            &mut processing_chain.setups_mut()[setup_index].recipe.produced,
                            from,
                            to,
                        );
                    }
                }
                ViewMode::NONE
            }
            Self::RenameProduct {
                kind,
                index,
                product,
            } => {
                match kind {
                    ProductKind::Catalyst => {
                        processing_chain.catalysts_mut(setup_index)[index] = product;
                    }
                    ProductKind::Consumed => {
                        processing_chain.setups_mut()[setup_index].recipe.consumed[index].product =
                            product;
                    }
                    ProductKind::Produced => {
                        processing_chain.setups_mut()[setup_index].recipe.produced[index].product =
                            product;
                    }
                }
                ViewMode::CALCULATED
            }
            Self::SetProducedCount { index, count } => {
                processing_chain.setups_mut()[setup_index].recipe.produced[index].count = count;
                ViewMode::CALCULATED
            }
            Self::SetConsumedCount { index, count } => {
                processing_chain.setups_mut()[setup_index].recipe.consumed[index].count = count;
                ViewMode::CALCULATED
            }
            Self::SetTime { ticks } => {
                processing_chain.setups_mut()[setup_index].recipe.ticks = ticks;
                ViewMode::CALCULATED
            }
            Self::SetEuPerTick { eu_per_tick } => {
                processing_chain.setups_mut()[setup_index]
                    .recipe
                    .eu_per_tick = eu_per_tick;
                ViewMode::CALCULATED
            }
        }
    }
}

fn move_item<T>(items: &mut [T], from: usize, to: usize) {
    match from.cmp(&to) {
        Ordering::Less => items[from..=to].rotate_left(1),
        Ordering::Equal => {}
        Ordering::Greater => items[to..=from].rotate_right(1),
    }
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct EditingBuffer {
    just_opened: bool,
    text: String,
}
