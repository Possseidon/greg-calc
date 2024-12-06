use std::{collections::BTreeMap, num::NonZeroU64};

use malachite::{Integer, Rational};
use serde::{Deserialize, Serialize};

use super::machine::Voltage;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub machine: Machine,
    pub ticks: NonZeroU64,
    #[serde(default)]
    pub eu_per_tick: i64,
    #[serde(default)]
    pub catalysts: Vec<Product>,
    #[serde(default)]
    pub consumed: Vec<ProductCount>,
    #[serde(default)]
    pub produced: Vec<ProductCount>,
}

impl Recipe {
    pub fn total_eu(&self) -> Integer {
        Integer::from(self.ticks.get()) * Integer::from(self.eu_per_tick)
    }

    /// Returns the minimum required [`Voltage`] based on [`Self::eu_per_tick`].
    ///
    /// Returns [`None`] if the recipe neither consumes nor produces power.
    pub fn voltage(&self) -> Option<Voltage> {
        Some(Voltage::from_signed_eu_per_tick(
            self.eu_per_tick.try_into().ok()?,
        ))
    }

    pub fn products(&self) -> impl Iterator<Item = &Product> {
        let consumed = self
            .consumed
            .iter()
            .map(|ProductCount { product, .. }| product);

        let produced = self
            .produced
            .iter()
            .map(|ProductCount { product, .. }| product);

        consumed.chain(produced)
    }

    pub fn product_counts(&self) -> BTreeMap<&Product, Integer> {
        let consumed = self
            .consumed
            .iter()
            .map(|ProductCount { product, count }| (product, -Integer::from(count.get())));

        let produced = self
            .produced
            .iter()
            .map(|ProductCount { product, count }| (product, Integer::from(count.get())));

        consumed
            .chain(produced)
            .fold(Default::default(), |mut acc, (product, count)| {
                *acc.entry(product).or_default() += count;
                acc
            })
    }

    pub fn products_per_sec(&self) -> impl Iterator<Item = (&Product, Rational)> {
        let seconds = self.seconds();
        self.product_counts()
            .into_iter()
            .map(move |(product, count)| (product, Rational::from(count) / &seconds))
    }

    pub fn produces(&self, product: &Product) -> bool {
        self.produced
            .iter()
            .any(|product_count| product_count.product == *product)
    }

    pub fn consumes(&self, product: &Product) -> bool {
        self.consumed
            .iter()
            .any(|product_count| product_count.product == *product)
    }

    pub const fn seconds(&self) -> Rational {
        Rational::const_from_unsigneds(self.ticks.get(), 20)
    }

    pub fn replace_product(&mut self, old: &Product, new: &Product) {
        for product in self
            .consumed
            .iter_mut()
            .chain(&mut self.produced)
            .map(|ProductCount { product, .. }| product)
            .chain(&mut self.catalysts)
            .filter(|product| *product == old)
        {
            *product = new.clone();
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Machine {
    pub name: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Product {
    pub name: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductCount {
    pub product: Product,
    pub count: NonZeroU64,
}
