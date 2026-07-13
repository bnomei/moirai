/// Where component data lives in the world.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum StorageKind {
    /// Per-entity sparse set.
    Sparse,
    /// Archetype column storage (Phase 3).
    Table,
}

/// Registration-time storage and tag policy.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ComponentOptions {
    storage: StorageKind,
    is_tag: bool,
}

impl ComponentOptions {
    pub const fn sparse() -> Self {
        Self {
            storage: StorageKind::Sparse,
            is_tag: false,
        }
    }

    pub const fn table() -> Self {
        Self {
            storage: StorageKind::Table,
            is_tag: false,
        }
    }

    pub const fn tag() -> Self {
        Self {
            storage: StorageKind::Sparse,
            is_tag: true,
        }
    }

    #[cfg(test)]
    pub(crate) const fn test_tag_table() -> Self {
        Self {
            storage: StorageKind::Table,
            is_tag: true,
        }
    }

    pub(crate) fn storage(self) -> StorageKind {
        self.storage
    }

    pub(crate) fn is_tag(self) -> bool {
        self.is_tag
    }
}
