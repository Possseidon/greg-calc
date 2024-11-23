pub mod ui;

use std::collections::BTreeMap;

use num_rational::Rational64;
use serde::{Deserialize, Serialize};

use crate::config::{Product, Recipe};

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProcessingChain {
    pub machines: Vec<Setup>,
}

impl ProcessingChain {
    /// Calculates how fast each [`Setup`] is running.
    ///
    /// At least one of the recipes will run at regular speed, i.e. `100%`. Other recipes will
    /// slow down due to either not having enough input from previous recipes or producing more than
    /// the followup machines can process.
    ///
    /// This assumes that machines are always either stopped or run at a 100%, i.e. there is no e.g.
    /// warmup phase.
    ///
    /// Additionally, any [`Recipe`]s producing a [`Product`] matching the `allow_overproduction`
    /// predicate will not be slowed down due to consuming machines not being able to keep up.
    pub fn speeds(&self, allow_overproduction: impl Fn(&Product) -> bool) -> Speeds {
        let mut speeds = Speeds {
            machines: self
                .machines
                .iter()
                .map(|setup| (setup.recipe.clone(), 1.into()))
                .collect(),
        };

        while let Some((product, ProductPerSecond { consumed, produced })) = self
            .products(&speeds)
            .products
            .iter()
            .filter(|(_, product_per_tick)| {
                product_per_tick.is_produced() && product_per_tick.is_consumed()
            })
            .find(|(product, product_per_tick)| {
                product_per_tick.is_underproduced()
                    || product_per_tick.is_overproduced() && !allow_overproduction(product)
            })
        {
            let (throttle, requires_throttling): (_, fn(_, _) -> _) = if produced > consumed {
                (consumed / produced, Recipe::produces)
            } else {
                (produced / consumed, Recipe::consumes)
            };

            for (_, speed) in speeds
                .machines
                .iter_mut()
                .filter(|(recipe, _)| requires_throttling(recipe, product))
            {
                *speed *= throttle
            }
        }

        speeds
    }

    /// Returns the total [`ProductsPerTick`] assuming recipes are running at the given `speeds`.
    pub fn products(&self, speeds: &Speeds) -> Products {
        self.products_with_speed_callback(|recipe| speeds.machines[recipe])
    }

    /// Returns the total [`ProductsPerTick`] assuming recipes are running at certain speeds.
    fn products_with_speed_callback(
        &self,
        recipe_speed: impl Fn(&Recipe) -> Rational64,
    ) -> Products {
        self.machines
            .iter()
            .fold(Default::default(), |mut acc, setup| {
                let recipe = &setup.recipe;
                let speed = recipe_speed(recipe);

                let speed_factor = setup.speed_factor();
                let seconds = recipe.seconds();
                for (product, count) in recipe.products() {
                    acc.products
                        .entry(product.clone())
                        .or_default()
                        .add(speed_factor * count * speed / seconds);
                }

                acc.eu_per_tick += setup.eu_factor() * recipe.eu_per_tick * speed;

                acc
            })
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    pub recipe: Recipe,
    pub machines: BTreeMap<Overclocking, u32>,
}

impl Setup {
    /// How fast this [`Setup`] can process recipes.
    pub fn speed_factor(&self) -> Rational64 {
        self.machines
            .iter()
            .map(|(overclocking, count)| overclocking.speed_factor() * i64::from(*count))
            .sum()
    }

    /// How much more EU this [`Setup`] uses.
    pub fn eu_factor(&self) -> Rational64 {
        self.machines
            .iter()
            .map(|(overclocking, count)| overclocking.eu_factor() * i64::from(*count))
            .sum()
    }
}

#[derive(
    Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Overclocking(pub i8);

impl Overclocking {
    /// How much faster recipes are processed for this [`Overclocking`].
    pub fn speed_factor(self) -> Rational64 {
        Rational64::from(2).pow(self.0.into())
    }

    /// How much more EU is required for this [`Overclocking`].
    pub fn eu_factor(self) -> Rational64 {
        Rational64::from(4).pow(self.0.into())
    }
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Speeds {
    pub machines: BTreeMap<Recipe, Rational64>,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Products {
    pub eu_per_tick: Rational64,
    pub products: BTreeMap<Product, ProductPerSecond>,
}

#[derive(
    Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(deny_unknown_fields)]
pub struct ProductPerSecond {
    /// Always positive.
    consumed: Rational64,
    /// Always positive.
    produced: Rational64,
}

impl ProductPerSecond {
    pub fn total(self) -> Rational64 {
        self.produced - self.consumed
    }

    pub fn add(&mut self, count: Rational64) {
        if count > Rational64::ZERO {
            self.produced += count;
        } else {
            self.consumed -= count;
        }
    }

    fn is_produced(&self) -> bool {
        self.produced != Rational64::ZERO
    }

    fn is_consumed(&self) -> bool {
        self.consumed != Rational64::ZERO
    }

    fn is_catalyst(self) -> bool {
        !self.is_produced() && !self.is_consumed()
    }

    fn is_underproduced(self) -> bool {
        self.produced < self.consumed
    }

    fn is_overproduced(self) -> bool {
        self.produced > self.consumed
    }
}
