use core::fmt;

/// Opaque entity handle relative to one [`crate::world::World`].
///
/// Copyable, orderable, and hashable for deterministic diagnostics. Stale handles
/// are rejected after despawn, including before the slot is reused. There is no
/// public raw constructor or bit conversion.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct EntityId(u64);

impl EntityId {
    pub(crate) const fn from_parts(slot: u32, generation: u32) -> Self {
        Self(((generation as u64) << 32) | slot as u64)
    }

    pub(crate) fn slot(self) -> u32 {
        (self.0 & 0xFFFF_FFFF) as u32
    }

    pub(crate) fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }
}

impl fmt::Debug for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EntityId")
            .field(&self.slot())
            .field(&self.generation())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    #[test]
    fn entity_id_is_eight_bytes() {
        assert_eq!(size_of::<EntityId>(), 8);
        assert_eq!(align_of::<EntityId>(), 8);
    }
}
