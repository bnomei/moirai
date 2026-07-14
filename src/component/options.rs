/// Where registered component data is stored in the world.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum StorageKind {
    /// Per-entity sparse set; default for tags and ordinary data components today.
    Sparse,
    /// Archetype column storage for table-backed components (Phase 3).
    Table,
}

/// Registration-time storage layout and tag policy for one component type.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ComponentOptions {
    storage: StorageKind,
    is_tag: bool,
}

impl ComponentOptions {
    /// Default sparse-set storage for ordinary data components.
    pub const fn sparse() -> Self {
        Self {
            storage: StorageKind::Sparse,
            is_tag: false,
        }
    }

    /// Archetype column storage for data components (Phase 3 table backend).
    pub const fn table() -> Self {
        Self {
            storage: StorageKind::Table,
            is_tag: false,
        }
    }

    /// Zero-sized marker component stored in sparse sets.
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
