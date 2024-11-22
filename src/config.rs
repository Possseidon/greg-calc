use std::{collections::BTreeMap, num::NonZeroU64};

use itertools::Itertools;
use num_rational::Rational64;
use serde::{Deserialize, Serialize};

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
pub struct Recipe {
    pub machine: Machine,
    pub ticks: u64,
    #[serde(default)]
    pub eu_per_tick: i64,
    #[serde(default)]
    pub catalysts: Vec<Product>,
    #[serde(default)]
    pub consumed: Vec<(Product, NonZeroU64)>,
    #[serde(default)]
    pub produced: Vec<(Product, NonZeroU64)>,
}

impl Recipe {
    /// Returns the minimum required [`Voltage`] based on [`Self::eu_per_tick`].
    pub fn voltage(&self) -> Option<Voltage> {
        Some(Voltage::from_eu_per_tick(NonZeroU64::new(
            self.eu_per_tick.unsigned_abs(),
        )?))
    }

    pub fn produces(&self, product: &Product) -> bool {
        self.produced.iter().any(|(current, _)| current == product)
    }

    pub fn consumes(&self, product: &Product) -> bool {
        self.consumed.iter().any(|(current, _)| current == product)
    }

    pub fn products(&self) -> impl Iterator<Item = (&Product, Rational64)> {
        Iterator::chain(
            self.produced.iter().map(|(product, count)| {
                (
                    product,
                    Rational64::from_integer(count.get().try_into().unwrap()),
                )
            }),
            self.consumed.iter().map(|(product, count)| {
                (
                    product,
                    -Rational64::from_integer(count.get().try_into().unwrap()),
                )
            }),
        )
    }

    pub fn total_eu(&self) -> i64 {
        i64::try_from(self.ticks).unwrap() * self.eu_per_tick
    }
}

pub enum Voltage {
    /// Up to `8EU/t`.
    UltraLow,
    /// Up to `32EU/t`.
    Low,
    /// Up to `128EU/t`.
    Medium,
    /// Up to `512EU/t`.
    High,
    /// Up to `2048EU/t`.
    Extreme,
    // TODO: rest...
    /// Up to what?
    Max,
}

impl Voltage {
    pub const fn from_eu_per_tick(eu_per_tick: NonZeroU64) -> Self {
        match (eu_per_tick.ilog2() - 3).div_ceil(2) {
            0 => Self::UltraLow,
            1 => Self::Low,
            2 => Self::Medium,
            3 => Self::High,
            4 => Self::Extreme,
            _ => Self::Max,
        }
    }

    pub const fn max_eu_per_tick(self) -> NonZeroU64 {
        const TWO: NonZeroU64 = if let Some(two) = NonZeroU64::new(2) {
            two
        } else {
            unreachable!();
        };

        if let Some(eu_per_tick) = TWO.checked_pow(self as u32 * 2 + 3) {
            eu_per_tick
        } else {
            panic!("should not overflow");
        }
    }
}
