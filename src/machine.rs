use std::{
    collections::BTreeMap,
    fmt,
    num::{NonZeroI64, NonZeroU64},
    str::FromStr,
};

use enum_map::Enum;
use malachite::{num::basic::traits::One, Integer, Rational};
use serde::{
    de::{Error, Unexpected},
    Deserialize, Deserializer, Serialize, Serializer,
};
use thiserror::Error;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Machines {
    /// A fixed number of machines that don't require power and run at regular speed.
    Count(u64),
    /// A collection of machines at certain [`Voltage`] levels.
    Overclocked(ClockedMachines),
}

impl Machines {
    pub fn speed_factor(&self) -> Rational {
        match self {
            Self::Count(_) => Rational::ONE,
            Self::Overclocked(overclocked_machines) => overclocked_machines.speed_factor(),
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClockedMachines {
    pub base_eu_per_tick: NonZeroI64,
    #[serde(flatten)]
    pub machines: BTreeMap<ClockedMachine, NonZeroU64>,
}

impl ClockedMachines {
    /// Returns the minimum required [`Voltage`] based on [`Self::eu_per_tick`].
    ///
    /// Returns [`None`] if the recipe neither consumes nor produces power.
    pub fn base_voltage(&self) -> Voltage {
        Voltage::from_signed_eu_per_tick(self.base_eu_per_tick)
    }

    pub fn speed_factor(&self) -> Rational {
        let base_voltage = self.base_voltage();
        self.machines
            .iter()
            .map(|(clocked_machine, count)| {
                clocked_machine.underclocking.speed_factor(base_voltage)
                    * Rational::from(count.get())
            })
            .sum()
    }

    pub fn eu_per_tick(&self) -> Integer {
        let base_voltage = self.base_voltage();
        self.machines
            .iter()
            .map(|(clocked_machine, count)| {
                let eu = Integer::from(self.base_eu_per_tick.get())
                    << clocked_machine.underclocking.eu_factor_log2(base_voltage);
                assert!(
                    eu > 0,
                    "underclocking should not be able to result in less than 1 eu per tick"
                );
                eu * Integer::from(count.get())
            })
            .sum()
    }
}

/// The tier and clocking of some machine, e.g. a "**HV** Macerator" running at **LV**.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClockedMachine {
    /// The [`Voltage`] tier of the machine.
    ///
    /// Not used by calculations, since only the [`Self::clocking`] is relevant for processing
    /// speed and power consumption.
    tier: Voltage,
    /// The [`Voltage`] that the machine is underclocked to/running at.
    ///
    /// Must not be greater than [`Self::tier`] since machines cannot be overclocked. Only
    /// recipes can be overclocked by using a higher [`Self::tier`] of machine.
    underclocking: Voltage,
}

impl ClockedMachine {
    pub fn tier(&self) -> Voltage {
        self.tier
    }

    pub fn underclocking(&self) -> Voltage {
        self.underclocking
    }
}

impl Serialize for ClockedMachine {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.tier == self.underclocking {
            serializer.collect_str(&format_args!("{}", self.tier))
        } else {
            serializer.collect_str(&format_args!("{}@{}", self.tier, self.underclocking))
        }
    }
}

impl<'de> Deserialize<'de> for ClockedMachine {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let clocked_machine: &str = Deserialize::deserialize(deserializer)?;

        let (tier_str, clocking_str) = clocked_machine
            .split_once('@')
            .unwrap_or((clocked_machine, clocked_machine));

        let parse_voltage = |voltage: &str| {
            voltage.parse().map_err(|_| {
                D::Error::invalid_value(Unexpected::Str(voltage), &"machine voltage tier")
            })
        };

        let tier = parse_voltage(tier_str)?;
        let clocking = parse_voltage(clocking_str)?;

        if clocking <= tier {
            Ok(Self {
                tier,
                underclocking: clocking,
            })
        } else {
            Err(D::Error::invalid_value(
                Unexpected::Str(clocking_str),
                &"clocking to not be greater than the machine tier",
            ))
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Enum)]
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
    const ULV: &str = "ULV";
    const LV: &str = "LV";
    const MV: &str = "MV";
    const HV: &str = "HV";
    const EV: &str = "EV";
    const MAX: &str = "MAX";

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UltraLow => Self::ULV,
            Self::Low => Self::LV,
            Self::Medium => Self::MV,
            Self::High => Self::HV,
            Self::Extreme => Self::EV,
            Self::Max => Self::MAX,
        }
    }

    pub const fn from_index(index: u8) -> Self {
        match index {
            0 => Self::UltraLow,
            1 => Self::Low,
            2 => Self::Medium,
            3 => Self::High,
            4 => Self::Extreme,
            _ => Self::Max,
        }
    }

    pub fn from_eu_per_tick(eu_per_tick: NonZeroU64) -> Self {
        Self::from_index(
            (eu_per_tick.ilog2() - 3)
                .div_ceil(2)
                .try_into()
                .unwrap_or(u8::MAX),
        )
    }

    pub fn from_signed_eu_per_tick(eu_per_tick: NonZeroI64) -> Self {
        Self::from_eu_per_tick(eu_per_tick.unsigned_abs())
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

    /// How much faster (or slower) a machine is running for a given `base_voltage`.
    ///
    /// E.g. [`Voltage::High`] will run four times faster for a `base_voltage` of [`Voltage::Low`].
    pub fn speed_factor(self, base_voltage: Voltage) -> Rational {
        Rational::ONE << self.overclocking_steps(base_voltage)
    }

    /// How much more energy a machine is consuming for a given `base_voltage` in `log2`.
    ///
    /// E.g. [`Voltage::High`] will require sixteen times more energy for a `base_voltage` of
    /// [`Voltage::Low`].
    pub fn eu_factor_log2(self, base_voltage: Voltage) -> i8 {
        2 * self.overclocking_steps(base_voltage)
    }

    /// The number of overclocking steps from the given `base_voltage`.
    ///
    /// E.g. [`Voltage::High`] is `2` steps over a `base_voltage` of [`Voltage::Low`].
    pub fn overclocking_steps(self, base_voltage: Voltage) -> i8 {
        self as i8 - base_voltage as i8
    }
}

impl fmt::Display for Voltage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Error)]
#[error("invalid voltage; should be \"LV\", \"MV\", etc...")]
pub struct VoltageFromStrError;

impl FromStr for Voltage {
    type Err = VoltageFromStrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            Self::ULV => Ok(Self::UltraLow),
            Self::LV => Ok(Self::Low),
            Self::MV => Ok(Self::Medium),
            Self::HV => Ok(Self::High),
            Self::EV => Ok(Self::Extreme),
            Self::MAX => Ok(Self::Max),
            _ => Err(VoltageFromStrError),
        }
    }
}
