use std::{
    cmp::Ordering,
    collections::BTreeMap,
    fmt,
    num::{NonZeroI64, NonZeroU64},
    str::FromStr,
};

use enum_map::Enum;
use enumset::EnumSetType;
use malachite::{
    num::basic::traits::{One, Zero},
    Integer, Rational,
};
use serde::{
    de::{Error, Unexpected},
    Deserialize, Deserializer, Serialize, Serializer,
};
use thiserror::Error;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Machines {
    /// A fixed number of machines that don't require power and run at regular speed.
    Eco(u64),
    /// A collection of machines at certain [`Voltage`] levels.
    Power(ClockedMachines),
}

impl Machines {
    /// Returns how fast a recipe is produced for its given `recipe_voltage`.
    pub fn speed_factor(
        &self,
        recipe_voltage: Option<Voltage>,
    ) -> Result<Rational, MachinePowerError> {
        match (recipe_voltage, self) {
            (None, Self::Eco(count)) => Ok(Rational::from(*count)),
            (Some(recipe_voltage), Self::Power(clocked_machines)) => {
                Ok(clocked_machines.speed_factor(recipe_voltage))
            }
            (None, Self::Power(_)) => Err(MachinePowerError::RequiresEco),
            (Some(_), Self::Eco(_)) => Err(MachinePowerError::RequiresPower),
        }
    }

    pub fn eu_per_tick(&self, recipe_eu_per_tick: i64) -> Result<Integer, MachinePowerError> {
        match (recipe_eu_per_tick.try_into().ok(), self) {
            (None, Self::Eco(_)) => Ok(Integer::ZERO),
            (Some(recipe_eu_per_tick), Self::Power(clocked_machines)) => {
                Ok(clocked_machines.eu_per_tick(recipe_eu_per_tick))
            }
            (None, Self::Power(_)) => Err(MachinePowerError::RequiresEco),
            (Some(_), Self::Eco(_)) => Err(MachinePowerError::RequiresPower),
        }
    }

    pub fn into_clocked(&mut self) -> &mut ClockedMachines {
        match self {
            Machines::Power(_) => {}
            _ => *self = Self::Power(Default::default()),
        }

        match self {
            Machines::Power(clocked_machines) => clocked_machines,
            _ => unreachable!(),
        }
    }

    pub fn into_eco(&mut self) -> &mut u64 {
        match self {
            Machines::Eco(_) => {}
            _ => *self = Self::Eco(0),
        }

        match self {
            Machines::Eco(count) => count,
            _ => unreachable!(),
        }
    }
}

impl Default for Machines {
    fn default() -> Self {
        Self::Eco(0)
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Error)]
pub enum MachinePowerError {
    #[error("recipe requires machines that do not deal with power")]
    RequiresEco,
    #[error("recipe requires machines that deal with power")]
    RequiresPower,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClockedMachines {
    #[serde(flatten)]
    pub machines: BTreeMap<ClockedMachine, NonZeroU64>,
}

impl ClockedMachines {
    pub fn speed_factor(&self, recipe_voltage: Voltage) -> Rational {
        self.machines
            .iter()
            .map(|(clocked_machine, count)| {
                clocked_machine.underclocking.speed_factor(recipe_voltage)
                    * Rational::from(count.get())
            })
            .sum()
    }

    pub fn eu_per_tick(&self, recipe_eu_per_tick: NonZeroI64) -> Integer {
        let recipe_voltage = Voltage::from_signed_eu_per_tick(recipe_eu_per_tick);
        self.machines
            .iter()
            .map(|(clocked_machine, count)| {
                let eu = Integer::from(recipe_eu_per_tick.get())
                    << clocked_machine.underclocking.eu_factor_log2(recipe_voltage);
                assert!(
                    eu != 0,
                    "underclocking should not be able to result in less than 1 eu per tick"
                );
                eu * Integer::from(count.get())
            })
            .sum()
    }
}

/// The tier and clocking of some machine, e.g. a "**HV** Macerator" running at **LV**.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
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

impl PartialOrd for ClockedMachine {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ClockedMachine {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.tier, other.underclocking).cmp(&(other.tier, self.underclocking))
    }
}

impl ClockedMachine {
    pub fn new(tier: Voltage) -> Self {
        Self {
            tier,
            underclocking: tier,
        }
    }

    pub fn with_underclocking(tier: Voltage, underclocking: Voltage) -> Self {
        assert!(underclocking <= tier);
        Self {
            tier,
            underclocking,
        }
    }

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

#[derive(Debug, Hash, PartialOrd, Ord, Enum, EnumSetType)]
pub enum Voltage {
    UltraLow,
    Low,
    Medium,
    High,
    Extreme,
    Insane,
    Ludicrous,
    Zpm,
    Ultimate,
    UltraHigh,
    UltraExcessive,
    UltraImmense,
    UltraExtreme,
    Overpowered,
    Maximum,
}

impl Voltage {
    const ULV: &str = "ULV";
    const LV: &str = "LV";
    const MV: &str = "MV";
    const HV: &str = "HV";
    const EV: &str = "EV";
    const IV: &str = "IV";
    const LU_V: &str = "LuV";
    const ZPM: &str = "ZPM";
    const UV: &str = "UV";
    const UHV: &str = "UHV";
    const UEV: &str = "UEV";
    const UIV: &str = "UIV";
    const UXV: &str = "UXV";
    const OP_V: &str = "OpV";
    const MAX: &str = "MAX";

    pub const fn acronym(self) -> &'static str {
        match self {
            Self::UltraLow => Self::ULV,
            Self::Low => Self::LV,
            Self::Medium => Self::MV,
            Self::High => Self::HV,
            Self::Extreme => Self::EV,
            Self::Insane => Self::IV,
            Self::Ludicrous => Self::LU_V,
            Self::Zpm => Self::ZPM,
            Self::Ultimate => Self::UV,
            Self::UltraHigh => Self::UHV,
            Self::UltraExcessive => Self::UEV,
            Self::UltraImmense => Self::UIV,
            Self::UltraExtreme => Self::UXV,
            Self::Overpowered => Self::OP_V,
            Self::Maximum => Self::MAX,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::UltraLow => "Ultra Low Voltage",
            Self::Low => "Low Voltage",
            Self::Medium => "Medium Voltage",
            Self::High => "High Voltage",
            Self::Extreme => "Extreme Voltage",
            Self::Insane => "Insane Voltage",
            Self::Ludicrous => "Ludicrous Voltage",
            Self::Zpm => "ZPM Voltage",
            Self::Ultimate => "Ultimate Voltage",
            Self::UltraHigh => "Ultra High Voltage",
            Self::UltraExcessive => "Ultra Excessive Voltage",
            Self::UltraImmense => "Ultra Immense Voltage",
            Self::UltraExtreme => "Ultra Extreme Voltage",
            Self::Overpowered => "Overpowered Voltage",
            Self::Maximum => "Maximum Voltage",
        }
    }

    pub fn from_eu_per_tick(eu_per_tick: NonZeroU64) -> Self {
        Self::from_usize(
            (eu_per_tick.ilog2().saturating_sub(3))
                .div_ceil(2)
                .try_into()
                .unwrap_or(usize::MAX),
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

    /// How much faster (or slower) a machine is running for a given `recipe_voltage`.
    ///
    /// E.g. [`Voltage::High`] will run four times faster for a `recipe_voltage` of
    /// [`Voltage::Low`].
    pub fn speed_factor(self, recipe_voltage: Voltage) -> Rational {
        Rational::ONE << self.overclocking_steps(recipe_voltage)
    }

    /// How much more energy a machine is consuming for a given `recipe_voltage` in `log2`.
    ///
    /// E.g. [`Voltage::High`] will require sixteen times more energy for a `recipe_voltage` of
    /// [`Voltage::Low`].
    pub fn eu_factor_log2(self, recipe_voltage: Voltage) -> i8 {
        2 * self.overclocking_steps(recipe_voltage)
    }

    /// The number of overclocking steps from the given `recipe_voltage`.
    ///
    /// E.g. [`Voltage::High`] is `2` steps over a `recipe_voltage` of [`Voltage::Low`].
    pub fn overclocking_steps(self, recipe_voltage: Voltage) -> i8 {
        self as i8 - recipe_voltage as i8
    }
}

impl fmt::Display for Voltage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.acronym())
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
            Self::IV => Ok(Self::Insane),
            Self::LU_V => Ok(Self::Ludicrous),
            Self::ZPM => Ok(Self::Zpm),
            Self::UV => Ok(Self::Ultimate),
            Self::UHV => Ok(Self::UltraHigh),
            Self::UEV => Ok(Self::UltraExcessive),
            Self::UIV => Ok(Self::UltraImmense),
            Self::UXV => Ok(Self::UltraExtreme),
            Self::OP_V => Ok(Self::Overpowered),
            Self::MAX => Ok(Self::Maximum),
            _ => Err(VoltageFromStrError),
        }
    }
}
