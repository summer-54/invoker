pub mod providers;

use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id(u128);

impl FromStr for Id {
    type Err = <u128 as FromStr>::Err;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        s.parse::<u128>().map(Self)
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
