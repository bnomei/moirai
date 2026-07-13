use core::fmt;

/// Opaque entity handle relative to one [`crate::world::World`].
///
/// Copyable, orderable, and hashable for deterministic diagnostics. Stale handles
/// are rejected after despawn, including before the slot is reused. There is no
/// public raw constructor or bit conversion.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(C)]
pub struct EntityId {
    owner: u32,
    packed: u64,
}

impl EntityId {
    pub(crate) const fn from_parts(slot: u32, generation: u32) -> Self {
        Self::from_owned_parts(0, slot, generation)
    }

    pub(crate) const fn from_owned_parts(owner: u32, slot: u32, generation: u32) -> Self {
        Self {
            owner,
            packed: ((generation as u64) << 32) | slot as u64,
        }
    }

    pub(crate) const fn owner(self) -> u32 {
        self.owner
    }

    pub(crate) const fn slot(self) -> u32 {
        (self.packed & 0xFFFF_FFFF) as u32
    }

    pub(crate) const fn generation(self) -> u32 {
        (self.packed >> 32) as u32
    }

    #[cfg(test)]
    pub(crate) const fn with_generation(self, generation: u32) -> Self {
        Self::from_owned_parts(self.owner, self.slot(), generation)
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
    fn entity_id_carries_private_owner_and_packed_position() {
        assert_eq!(size_of::<EntityId>(), 16);
        assert_eq!(align_of::<EntityId>(), 8);
    }

    #[test]
    fn owner_participates_in_identity_but_not_debug_output() {
        let a = EntityId::from_owned_parts(1, 2, 3);
        let b = EntityId::from_owned_parts(2, 2, 3);
        assert_ne!(a, b);
        assert_eq!(alloc::format!("{a:?}"), "EntityId(2, 3)");
    }
}
