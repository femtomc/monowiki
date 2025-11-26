//! Durability tiers for queries
//!
//! Queries are partitioned by expected change frequency. This allows
//! the incremental system to skip checking queries at higher durability
//! levels when lower-tier queries change.

use std::fmt;

/// Durability tiers determine how often a query is expected to change
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Durability {
    /// Changes on every edit (user content, cursor position)
    Volatile = 0,

    /// Changes on user actions (viewport, UI state)
    Session = 1,

    /// Changes on explicit reload (theme, macros, configuration)
    Durable = 2,

    /// Never changes (built-in functions, core library)
    Static = 3,
}

impl Durability {
    /// Returns true if this durability is at least as stable as `other`
    pub fn at_least(&self, other: Durability) -> bool {
        *self >= other
    }

    /// Returns the more volatile of two durabilities
    pub fn min(self, other: Durability) -> Durability {
        if self < other { self } else { other }
    }

    /// Returns the more durable of two durabilities
    pub fn max(self, other: Durability) -> Durability {
        if self > other { self } else { other }
    }
}

impl Default for Durability {
    fn default() -> Self {
        Durability::Volatile
    }
}

impl fmt::Display for Durability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Durability::Volatile => write!(f, "volatile"),
            Durability::Session => write!(f, "session"),
            Durability::Durable => write!(f, "durable"),
            Durability::Static => write!(f, "static"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_durability_ordering() {
        assert!(Durability::Volatile < Durability::Session);
        assert!(Durability::Session < Durability::Durable);
        assert!(Durability::Durable < Durability::Static);
    }

    #[test]
    fn test_at_least() {
        assert!(Durability::Static.at_least(Durability::Volatile));
        assert!(Durability::Durable.at_least(Durability::Session));
        assert!(!Durability::Volatile.at_least(Durability::Session));
    }

    #[test]
    fn test_min_max() {
        assert_eq!(
            Durability::Volatile.min(Durability::Durable),
            Durability::Volatile
        );
        assert_eq!(
            Durability::Volatile.max(Durability::Durable),
            Durability::Durable
        );
    }
}
