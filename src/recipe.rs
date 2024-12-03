use std::{collections::BTreeMap, num::NonZeroU64};

use malachite::{Integer, Rational};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub machine: Machine,
    pub ticks: NonZeroU64,
    #[serde(default)]
    pub catalysts: Vec<Product>,
    #[serde(default)]
    pub consumed: Vec<ProductCount>,
    #[serde(default)]
    pub produced: Vec<ProductCount>,
}

impl Recipe {
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
