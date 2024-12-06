use std::{
    cell::{LazyCell, OnceCell},
    collections::{BTreeMap, BTreeSet},
};

use bitvec::vec::BitVec;
use itertools::Itertools;
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};
use serde::{Deserialize, Serialize};

use super::{
    machine::{MachinePowerError, Machines},
    recipe::{Machine, Product, Recipe},
};
use crate::math::nullspace::nullspace;

/// Consists of various machines that are processing [`Product`]s using specific [`Recipe`]s.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessingChain {
    /// The collection of all [`Setup`]s in this [`ProcessingChain`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    setups: Vec<Setup>,
    /// [`Product`]s that are explicitly set to input/output of the entire [`ProcessingChain`].
    ///
    /// These [`Product`]s will not be forced to net-zero when solving for machine speeds.
    /// Any [`Product`] that is either only produced or only consumed is treated as such
    /// implicitly, as the producing/consuming machines would not be able to run at all.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    explicit_io: BTreeSet<Product>,
    /// Caches various information about the [`ProcessingChain`].
    ///
    /// Whenever fields are updated relevant cached values are invalidated.
    #[serde(skip)]
    cache: Cache,
}

impl ProcessingChain {
    pub fn setups(&self) -> &[Setup] {
        &self.setups
    }

    pub fn setups_mut(&mut self) -> &mut Vec<Setup> {
        self.cache = Cache::default();
        &mut self.setups
    }

    pub fn machine_mut(&mut self, index: usize) -> &mut Machine {
        &mut self.setups[index].recipe.machine
    }

    pub fn catalysts_mut(&mut self, index: usize) -> &mut Vec<Product> {
        &mut self.setups[index].recipe.catalysts
    }

    /// Updates a [`Setup::weight`], which only invalidates the cached [`WeightedSpeeds`].
    pub fn set_weight(&mut self, index: usize, weight: Weight) {
        self.cache.weighted_speeds.take();
        self.setups[index].weight = weight;
    }

    pub fn explicit_ui(&self) -> &BTreeSet<Product> {
        &self.explicit_io
    }

    pub fn explicit_io_mut(&mut self) -> &mut BTreeSet<Product> {
        self.cache = Cache::default();
        &mut self.explicit_io
    }

    pub fn products(&self) -> BTreeSet<&Product> {
        self.setups
            .iter()
            .flat_map(|setup| setup.recipe.products())
            .collect()
    }

    /// Returns the total [`Products`] assuming all machines are running at normal speed.
    pub fn products_with_max_speeds(&self) -> Products {
        let speed = Rational::ONE;
        self.products_with_speed_callback(|_| &speed)
    }

    /// Returns the total [`Products`] assuming recipes are running at the given `speeds`.
    pub fn products_with_speeds(&self, weighted_speeds: &WeightedSpeeds) -> Products {
        self.products_with_speed_callback(|index| &weighted_speeds.speeds[index])
    }

    pub fn speeds(&self) -> &Speeds {
        self.cache.speeds.get_or_init(|| Speeds::new(self))
    }

    pub fn weighted_speeds(&self) -> &WeightedSpeeds {
        self.cache
            .weighted_speeds
            .get_or_init(|| WeightedSpeeds::new(self.speeds(), &self.setups))
    }

    pub fn replace_product(&mut self, old: &Product, new: Product) {
        for setup in self.setups_mut() {
            setup.recipe.replace_product(old, &new);
        }

        if self.explicit_io_mut().remove(old) {
            self.explicit_io_mut().insert(new);
        }
    }

    /// Returns the total [`Products`] assuming recipes are running at certain speeds.
    ///
    /// Setups with [`MachinePowerError`] are ignored.
    fn products_with_speed_callback<'a>(
        &self,
        setup_speed: impl Fn(usize) -> &'a Rational,
    ) -> Products {
        self.setups
            .iter()
            .enumerate()
            .fold(Default::default(), |mut acc, (index, setup)| {
                let speed = setup_speed(index);

                for (product, count) in setup.products_per_sec_filter_ok() {
                    *acc.products_per_sec.entry(product.clone()).or_default() += count * speed;
                }

                if let Ok(eu_per_tick) = setup.machines.eu_per_tick(setup.recipe.eu_per_tick) {
                    acc.eu_per_tick += Rational::from(eu_per_tick) * speed;
                }

                acc
            })
    }
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Products {
    pub eu_per_tick: Rational,
    pub products_per_sec: BTreeMap<Product, Rational>,
}

/// A set of machines that all produce the same [`Recipe`].
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    /// The recipe that this [`Setup`] is processing.
    pub recipe: Recipe,
    /// The number of machines per [`Voltage`] tier.
    pub machines: Machines,
    /// Used if another [`Setup`] also produces/consumes the same [`Product`].
    ///
    /// When multiple machines share a product, each machine's share is determined by both its
    /// production/consumption rate and its weight.
    ///
    /// E.g. if machine `A` consumes `2P/sec` and machine `B` consumes `3P/sec`:
    ///
    /// - **If both have the same (non-zero) weight:**
    ///   - `A` gets `2/5` and `B` gets `3/5`.
    ///   - This is because their share is proportional to their consumption rate:
    ///     - The total consumption rate is `2 + 3 = 5P/sec`.
    ///     - `A`'s share = 2 out of 5 (`2/5`), and `B`'s share = 3 out of 5 (`3/5`).
    ///
    /// - **If `A` has twice the weight:**
    ///   - Each machine's **effective weight** is the product of its weight and its consumption
    ///     rate.
    ///   - For `A` (weight `2`, consumption rate `2P/sec`):
    ///     - Effective weight = `2 * 2 = 4`.
    ///   - For `B` (weight `1`, consumption rate `3P/sec`):
    ///     - Effective weight = `1 * 3 = 3`.
    ///   - Total effective weight = `4 + 3 = 7`.
    ///   - `A`'s share = `4/7` and `B`'s share = `3/7`.
    ///
    /// - **If `B` has zero weight:**
    ///   - `A` gets 100% of the product, since `B` contributes no effective weight.
    ///   - `A`'s share = `1` (100% of the product).
    ///
    /// **Note:** Setting a weight to zero effectively disables the [`Setup`],
    /// preventing the machine from contributing to the product allocation. This is useful for
    /// temporarily stopping a machine from participating in the allocation process.
    #[serde(default)]
    pub weight: Weight,
}

impl Setup {
    /// How fast this [`Setup`] can process recipes.
    pub fn speed_factor(&self) -> Result<Rational, MachinePowerError> {
        self.machines.speed_factor(self.recipe.voltage())
    }

    fn products_per_sec_filter_ok(&self) -> impl Iterator<Item = (&Product, Rational)> {
        self.products_per_sec()
            .filter_map(|(product, amount)| amount.ok().map(|amount| (product, amount)))
    }

    fn products_per_sec(
        &self,
    ) -> impl Iterator<Item = (&Product, Result<Rational, MachinePowerError>)> {
        let speed_factor = LazyCell::new(|| self.speed_factor());
        self.recipe
            .products_per_sec()
            .map(move |(product, amount)| {
                (
                    product,
                    speed_factor
                        .as_ref()
                        .map(|speed_factor| amount * speed_factor)
                        .map_err(|error| *error),
                )
            })
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Weight(pub u64);

impl Default for Weight {
    fn default() -> Self {
        Self(1)
    }
}

#[derive(Clone, Debug, Default)]
struct Cache {
    /// Does not change if only weights change.
    speeds: OnceCell<Speeds>,
    weighted_speeds: OnceCell<WeightedSpeeds>,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Speeds {
    weighted_setups: BitVec,
    speeds: Vec<Rational>,
}

impl Speeds {
    /// TODO
    ///
    /// Any [`Setup`]s with a [`MachinePowerError`] are ignored.
    fn new(processing_chain: &ProcessingChain) -> Self {
        let processing_chains = processing_chain.setups.len();

        let setup_products_per_sec = processing_chain
            .setups
            .iter()
            .map(|setup| {
                setup
                    .products_per_sec_filter_ok()
                    .collect::<BTreeMap<_, _>>()
            })
            .collect_vec();

        let matrix = processing_chain
            .products()
            .into_iter()
            .filter(|product| {
                !processing_chain.explicit_io.contains(product)
                    && processing_chain
                        .setups
                        .iter()
                        .any(|setup| setup.recipe.consumes(product))
                    && processing_chain
                        .setups
                        .iter()
                        .any(|setup| setup.recipe.produces(product))
            })
            .flat_map(|product| {
                (0..processing_chains).map(|setup_index| {
                    setup_products_per_sec[setup_index]
                        .get(product)
                        .cloned()
                        .unwrap_or_default()
                })
            })
            .collect_vec();

        let (weighted_setups, speeds) = nullspace(matrix, processing_chains);
        Self {
            weighted_setups,
            speeds,
        }
    }
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct WeightedSpeeds {
    speeds: Vec<Rational>,
}

impl WeightedSpeeds {
    fn new(speeds: &Speeds, setups: &[Setup]) -> Self {
        let mut speeds = speeds
            .speeds
            .chunks_exact(setups.len())
            .zip(
                speeds
                    .weighted_setups
                    .iter_ones()
                    .map(|index| &setups[index]),
            )
            .fold(
                vec![Rational::ONE; setups.len()],
                |mut acc, (speeds, setup)| {
                    for (acc_speed, speed) in acc.iter_mut().zip_eq(speeds) {
                        *acc_speed *= speed * Rational::from(setup.weight.0);
                    }
                    acc
                },
            );

        if let Some(max_speed) = speeds.iter().max().cloned() {
            if max_speed != Rational::ZERO {
                for speed in &mut speeds {
                    *speed /= &max_speed;
                }
            }
        }

        Self { speeds }
    }

    pub fn speeds(&self) -> &[Rational] {
        &self.speeds
    }
}
