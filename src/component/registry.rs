//! Checked component registration table for one [`crate::world::World`].
//!
//! Typed and untyped tag registration share conflict detection for names, layouts, and storage
//! policy before dense [`ComponentId`] handles are issued.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::any::{type_name, TypeId};
use core::mem::{align_of, needs_drop, size_of};

use crate::component::{ComponentOptions, StorageKind};
use crate::world::WorldOwner;

/// Dense registry-local component handle scoped to one world owner.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ComponentId {
    owner: WorldOwner,
    index: u32,
}

/// Component registration conflict or policy violation.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistrationError {
    /// The same `TypeId` was registered with incompatible metadata.
    TypeConflict {
        /// Registration name associated with the conflict.
        name: String,
        /// Existing entry name.
        existing: String,
        /// Requested entry name.
        requested: String,
    },
    /// The registration name is already bound to different metadata.
    NameConflict {
        /// Registration name associated with the conflict.
        name: String,
        /// Existing entry name.
        existing: String,
        /// Requested entry name.
        requested: String,
    },
    /// Size, alignment, or owner metadata does not match an existing entry.
    LayoutConflict {
        /// Registration name associated with the conflict.
        name: String,
        /// Human-readable layout detail.
        detail: String,
    },
    /// Typed tag registration violated zero-sized non-dropping requirements.
    InvalidTag {
        /// Registration name associated with the conflict.
        name: String,
        /// Human-readable validation detail.
        detail: String,
    },
    /// Requested storage policy is incompatible with the component shape.
    UnsupportedStorage {
        /// Registration name associated with the conflict.
        name: String,
        /// Human-readable policy detail.
        detail: String,
    },
}

struct ComponentEntry {
    name: String,
    type_id: Option<TypeId>,
    is_tag: bool,
    storage: StorageKind,
    size: usize,
    align: usize,
}

/// Checked component registration table.
pub(crate) struct ComponentRegistry {
    entries: Vec<ComponentEntry>,
}

impl ComponentId {
    pub(crate) fn new(owner: WorldOwner, index: u32) -> Self {
        Self { owner, index }
    }

    /// Dense registry index for diagnostics and lifecycle event wiring.
    pub fn index(&self) -> usize {
        self.index as usize
    }

    pub(crate) fn validate_owner(&self, owner: &WorldOwner) -> Result<(), RegistrationError> {
        if self.owner.same(owner) {
            Ok(())
        } else {
            Err(RegistrationError::LayoutConflict {
                name: String::from("<component>"),
                detail: String::from("component id belongs to a different world"),
            })
        }
    }
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn register_typed<T: 'static>(
        &mut self,
        owner: &WorldOwner,
        name: Option<&str>,
        options: ComponentOptions,
    ) -> Result<ComponentId, RegistrationError> {
        let name = name.unwrap_or(type_name::<T>()).to_string();
        if options.is_tag() {
            Self::validate_typed_tag::<T>(&name)?;
        }
        if options.storage() == StorageKind::Table && options.is_tag() {
            return Err(RegistrationError::UnsupportedStorage {
                name,
                detail: String::from("tag components cannot use table storage"),
            });
        }
        self.register_inner(
            owner,
            name,
            Some(TypeId::of::<T>()),
            options,
            size_of::<T>(),
            align_of::<T>(),
        )
    }

    pub fn register_untyped(
        &mut self,
        owner: &WorldOwner,
        name: &str,
        options: ComponentOptions,
    ) -> Result<ComponentId, RegistrationError> {
        if !options.is_tag() {
            return Err(RegistrationError::LayoutConflict {
                name: name.to_string(),
                detail: String::from("untyped registration requires tag options"),
            });
        }
        if options.storage() == StorageKind::Table {
            return Err(RegistrationError::UnsupportedStorage {
                name: name.to_string(),
                detail: String::from("untyped tags cannot use table storage"),
            });
        }
        self.register_inner(owner, name.to_string(), None, options, 0, 1)
    }

    pub fn storage_kind(&self, id: &ComponentId) -> Option<StorageKind> {
        self.entries.get(id.index()).map(|entry| entry.storage)
    }

    #[allow(dead_code)]
    pub fn is_tag(&self, id: &ComponentId) -> Option<bool> {
        self.entries.get(id.index()).map(|entry| entry.is_tag)
    }

    pub(crate) fn entry_is_tag(&self, index: usize) -> bool {
        self.entries.get(index).is_some_and(|entry| entry.is_tag)
    }

    pub(crate) fn entry_is_table(&self, index: usize) -> bool {
        self.entries
            .get(index)
            .is_some_and(|entry| entry.storage == StorageKind::Table)
    }

    pub(crate) fn component_name(&self, id: &ComponentId) -> String {
        self.entries
            .get(id.index())
            .map(|entry| entry.name.clone())
            .unwrap_or_else(|| String::from("<unknown component>"))
    }

    pub(crate) fn type_id_for_index(&self, index: usize) -> Option<TypeId> {
        self.entries.get(index).and_then(|entry| entry.type_id)
    }

    pub(crate) fn index_of_type(&self, type_id: TypeId) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| entry.type_id == Some(type_id))
    }

    pub(crate) fn id_of<T: 'static>(&self, owner: &WorldOwner) -> Option<ComponentId> {
        let type_id = TypeId::of::<T>();
        self.entries
            .iter()
            .position(|entry| entry.type_id == Some(type_id))
            .map(|index| ComponentId::new(owner.clone(), index as u32))
    }

    fn register_inner(
        &mut self,
        owner: &WorldOwner,
        name: String,
        type_id: Option<TypeId>,
        options: ComponentOptions,
        size: usize,
        align: usize,
    ) -> Result<ComponentId, RegistrationError> {
        if let Some(existing) = self.find_exact(type_id, &name, options, size, align) {
            return Ok(ComponentId::new(owner.clone(), existing as u32));
        }

        if let Some((index, reason)) = self.find_conflict(type_id, &name, options, size, align) {
            let entry = &self.entries[index];
            return Err(match reason {
                ConflictKind::Type => RegistrationError::TypeConflict {
                    name: name.clone(),
                    existing: entry.name.clone(),
                    requested: name,
                },
                ConflictKind::Name => RegistrationError::NameConflict {
                    name: name.clone(),
                    existing: entry.name.clone(),
                    requested: name,
                },
                ConflictKind::Layout => RegistrationError::LayoutConflict {
                    name: name.clone(),
                    detail: format!(
                        "existing={}:{}x{} {:?} {:?}; requested={}:{}x{} {:?} {:?}",
                        entry.name,
                        entry.size,
                        entry.align,
                        entry.type_id,
                        entry.storage,
                        name,
                        size,
                        align,
                        type_id,
                        options.storage()
                    ),
                },
            });
        }

        let index = self.entries.len() as u32;
        self.entries.push(ComponentEntry {
            name,
            type_id,
            is_tag: options.is_tag(),
            storage: options.storage(),
            size,
            align,
        });
        Ok(ComponentId::new(owner.clone(), index))
    }

    fn find_exact(
        &self,
        type_id: Option<TypeId>,
        name: &str,
        options: ComponentOptions,
        size: usize,
        align: usize,
    ) -> Option<usize> {
        self.entries.iter().position(|entry| {
            entry.name == name
                && entry.type_id == type_id
                && entry.is_tag == options.is_tag()
                && entry.storage == options.storage()
                && entry.size == size
                && entry.align == align
        })
    }

    fn find_conflict(
        &self,
        type_id: Option<TypeId>,
        name: &str,
        options: ComponentOptions,
        size: usize,
        align: usize,
    ) -> Option<(usize, ConflictKind)> {
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.type_id == type_id && type_id.is_some() {
                if entry.size != size || entry.align != align {
                    return Some((index, ConflictKind::Layout));
                } else if entry.name != name
                    || entry.is_tag != options.is_tag()
                    || entry.storage != options.storage()
                {
                    return Some((index, ConflictKind::Type));
                }
            } else if entry.name == name
                && (entry.type_id != type_id
                    || entry.is_tag != options.is_tag()
                    || entry.storage != options.storage()
                    || entry.size != size
                    || entry.align != align)
            {
                return Some((index, ConflictKind::Name));
            }
        }
        None
    }

    #[cfg(test)]
    pub(crate) fn find_conflict_for_test(
        &self,
        type_id: Option<TypeId>,
        name: &str,
        options: ComponentOptions,
        size: usize,
        align: usize,
    ) -> bool {
        self.find_conflict(type_id, name, options, size, align)
            .is_some()
    }

    fn validate_typed_tag<T: 'static>(name: &str) -> Result<(), RegistrationError> {
        if size_of::<T>() != 0 || needs_drop::<T>() {
            return Err(RegistrationError::InvalidTag {
                name: name.to_string(),
                detail: String::from("typed tag components must be zero-sized and non-dropping"),
            });
        }
        Ok(())
    }
}

enum ConflictKind {
    Type,
    Name,
    Layout,
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "std")]
impl core::fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TypeConflict {
                name,
                existing,
                requested,
            } => write!(
                f,
                "component type conflict for {name}: existing={existing}, requested={requested}"
            ),
            Self::NameConflict {
                name,
                existing,
                requested,
            } => write!(
                f,
                "component name conflict for {name}: existing={existing}, requested={requested}"
            ),
            Self::LayoutConflict { name, detail } => {
                write!(f, "component layout conflict for {name}: {detail}")
            }
            Self::InvalidTag { name, detail } => {
                write!(f, "invalid tag component {name}: {detail}")
            }
            Self::UnsupportedStorage { name, detail } => {
                write!(f, "unsupported storage for {name}: {detail}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RegistrationError {}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod default_tests {
    use super::ComponentRegistry;

    #[test]
    fn default_registry_is_empty() {
        assert_eq!(ComponentRegistry::default().len(), 0);
    }
}
