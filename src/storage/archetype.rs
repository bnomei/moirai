use crate::component::{ComponentId, ComponentRegistry, StorageKind};
use crate::entity::EntityId;
use crate::storage::table::{ErasedTableColumn, TypedTableColumn};
use crate::time::ChangeTick;
use alloc::boxed::Box;
use alloc::vec::Vec;

pub(crate) type TableColumnFactory = fn() -> Box<dyn ErasedTableColumn>;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Signature {
    components: Vec<u32>,
}

impl Signature {
    fn empty() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    fn contains(&self, component_index: u32) -> bool {
        self.components.binary_search(&component_index).is_ok()
    }

    fn with_added(mut self, component_index: u32) -> Self {
        if let Err(insert_at) = self.components.binary_search(&component_index) {
            self.components.insert(insert_at, component_index);
        }
        self
    }

    fn with_removed(mut self, component_index: u32) -> Self {
        if let Ok(remove_at) = self.components.binary_search(&component_index) {
            self.components.remove(remove_at);
        }
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Location {
    archetype: u32,
    row: u32,
}

pub(crate) struct ArchetypeStorage {
    column_factories: Vec<Option<TableColumnFactory>>,
    signatures: Vec<Signature>,
    columns: Vec<Vec<Box<dyn ErasedTableColumn>>>,
    entity_slots: Vec<Vec<u32>>,
    locations: Vec<Option<Location>>,
}

impl ArchetypeStorage {
    pub fn new(column_factories: Vec<Option<TableColumnFactory>>) -> Self {
        Self {
            column_factories,
            signatures: Vec::new(),
            columns: Vec::new(),
            entity_slots: Vec::new(),
            locations: Vec::new(),
        }
    }

    pub fn insert_table<T: Clone + 'static>(
        &mut self,
        entity: EntityId,
        component_index: u32,
        value: T,
        tick: ChangeTick,
    ) -> Option<T> {
        let current = self.signature_for(entity);
        if current.contains(component_index) {
            let location = self.location(entity).expect("entity located");
            let archetype = location.archetype as usize;
            let row = location.row as usize;
            let column = self.column_position(archetype, component_index);
            return self.columns[archetype][column]
                .replace_value(row, Box::new(value), tick)
                .map(|boxed| *boxed.downcast::<T>().expect("type match"));
        }

        let destination = current.with_added(component_index);
        let row = self.place_entity(entity, &destination, tick) as usize;
        let archetype = self.location(entity).expect("placed").archetype as usize;
        let column = self.column_position(archetype, component_index);
        if self.columns[archetype][column].len() <= row {
            self.columns[archetype][column].append_value(Box::new(value), tick);
        } else {
            let _ = self.columns[archetype][column].replace_value(row, Box::new(value), tick);
        }
        None
    }

    #[cfg(test)]
    pub(crate) fn write_migration_column_for_test<T: Clone + 'static>(
        &mut self,
        archetype: usize,
        component_index: u32,
        row: usize,
        value: T,
        tick: ChangeTick,
    ) {
        let column = self.column_position(archetype, component_index);
        if self.columns[archetype][column].len() <= row {
            self.columns[archetype][column].append_value(Box::new(value), tick);
        } else {
            let _ = self.columns[archetype][column].replace_value(row, Box::new(value), tick);
        }
    }

    pub fn get_table<T: Clone + 'static>(
        &self,
        entity: EntityId,
        component_index: u32,
    ) -> Option<&T> {
        let location = self.location(entity)?;
        let archetype = location.archetype as usize;
        if !self.signatures[archetype].contains(component_index) {
            return None;
        }
        let row = location.row as usize;
        let column = self.column_position(archetype, component_index);
        self.columns[archetype][column]
            .get_value(row)?
            .downcast_ref::<T>()
    }

    pub(crate) fn get_two_table_mut<TA: Clone + 'static, TB: Clone + 'static>(
        &mut self,
        entity: EntityId,
        index_a: u32,
        index_b: u32,
        tick: ChangeTick,
    ) -> Option<(&mut TA, &mut TB)> {
        if index_a == index_b {
            return None;
        }
        let location = self.location(entity)?;
        let archetype = location.archetype as usize;
        if !self.signatures[archetype].contains(index_a)
            || !self.signatures[archetype].contains(index_b)
        {
            return None;
        }
        let row = location.row as usize;
        let col_a = self.column_position(archetype, index_a);
        let col_b = self.column_position(archetype, index_b);
        let columns = &mut self.columns[archetype];
        if col_a < col_b {
            let (left, right) = columns.split_at_mut(col_b);
            let a = left[col_a].get_value_mut(row, tick)?.downcast_mut::<TA>()?;
            let b = right[0].get_value_mut(row, tick)?.downcast_mut::<TB>()?;
            Some((a, b))
        } else {
            let (left, right) = columns.split_at_mut(col_a);
            let b = left[col_b].get_value_mut(row, tick)?.downcast_mut::<TB>()?;
            let a = right[0].get_value_mut(row, tick)?.downcast_mut::<TA>()?;
            Some((a, b))
        }
    }

    pub fn get_table_mut<T: Clone + 'static>(
        &mut self,
        entity: EntityId,
        component_index: u32,
        tick: ChangeTick,
    ) -> Option<&mut T> {
        let location = self.location(entity)?;
        let archetype = location.archetype as usize;
        if !self.signatures[archetype].contains(component_index) {
            return None;
        }
        let row = location.row as usize;
        let column = self.column_position(archetype, component_index);
        self.columns[archetype][column]
            .get_value_mut(row, tick)?
            .downcast_mut::<T>()
    }

    pub fn remove_table_index(&mut self, entity: EntityId, component_index: u32) -> bool {
        if !self.signature_for(entity).contains(component_index) {
            return false;
        }
        let location = self.location(entity).expect("entity located");
        let archetype = location.archetype as usize;
        let row = location.row as usize;
        let column = self.column_position(archetype, component_index);
        let _ = self.columns[archetype][column].take_value(row);
        let destination = self.signature_for(entity).with_removed(component_index);
        self.rehome_entity(entity, &destination);
        true
    }

    pub fn remove_table<T: Clone + 'static>(
        &mut self,
        entity: EntityId,
        component_index: u32,
    ) -> Option<T> {
        if !self.signature_for(entity).contains(component_index) {
            return None;
        }
        let location = self.location(entity)?;
        let archetype = location.archetype as usize;
        let row = location.row as usize;
        let column = self.column_position(archetype, component_index);
        let removed = self.columns[archetype][column]
            .take_value(row)?
            .downcast::<T>()
            .ok()
            .map(|value| *value);
        let destination = self.signature_for(entity).with_removed(component_index);
        self.rehome_entity(entity, &destination);
        removed
    }

    pub fn table_component_indices(&self, entity: EntityId) -> Vec<u32> {
        self.signature_for(entity).components.clone()
    }

    pub(crate) fn has_component(&self, entity: EntityId, component_index: u32) -> bool {
        let Some(location) = self.location(entity) else {
            return false;
        };
        self.signatures[location.archetype as usize].contains(component_index)
    }

    pub(crate) fn archetypes_with_component(&self, component_index: u32) -> Vec<usize> {
        self.signatures
            .iter()
            .enumerate()
            .filter(|(_, signature)| signature.contains(component_index))
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn entity_slots(&self, archetype: usize) -> &[u32] {
        &self.entity_slots[archetype]
    }

    pub(crate) fn table_added_tick(
        &self,
        entity: EntityId,
        component_index: u32,
    ) -> Option<ChangeTick> {
        let location = self.location(entity)?;
        let archetype = location.archetype as usize;
        if !self.signatures[archetype].contains(component_index) {
            return None;
        }
        let row = location.row as usize;
        let column = self.column_position(archetype, component_index);
        self.columns[archetype][column].added_tick(row)
    }

    pub(crate) fn table_changed_tick(
        &self,
        entity: EntityId,
        component_index: u32,
    ) -> Option<ChangeTick> {
        let location = self.location(entity)?;
        let archetype = location.archetype as usize;
        if !self.signatures[archetype].contains(component_index) {
            return None;
        }
        let row = location.row as usize;
        let column = self.column_position(archetype, component_index);
        self.columns[archetype][column].changed_tick(row)
    }

    pub fn remove_entity(&mut self, entity: EntityId) {
        let Some(location) = self.location(entity) else {
            self.clear_location(entity);
            return;
        };
        self.remove_row(location);
        self.clear_location(entity);
    }

    fn place_entity(&mut self, entity: EntityId, signature: &Signature, tick: ChangeTick) -> u32 {
        if signature.components.is_empty() {
            if let Some(old) = self.location(entity) {
                self.remove_row(old);
            }
            self.clear_location(entity);
            return 0;
        }

        let dest = self.find_or_create_archetype(signature);
        if let Some(source) = self.location(entity) {
            if source.archetype == dest {
                return source.row;
            }
            return self.migrate_entity(entity, source, dest, tick);
        }

        let row = self.entity_slots[dest as usize].len() as u32;
        self.entity_slots[dest as usize].push(entity.slot());
        *self.ensure_location_slot(entity) = Some(Location {
            archetype: dest,
            row,
        });
        row
    }

    fn rehome_entity(&mut self, entity: EntityId, signature: &Signature) {
        let _ = self.place_entity(entity, signature, ChangeTick::ZERO);
    }

    fn migrate_entity(
        &mut self,
        entity: EntityId,
        source: Location,
        dest_archetype: u32,
        tick: ChangeTick,
    ) -> u32 {
        let source_archetype = source.archetype as usize;
        let source_row = source.row as usize;
        let dest_archetype = dest_archetype as usize;
        let dest_row = self.entity_slots[dest_archetype].len() as u32;
        self.entity_slots[dest_archetype].push(entity.slot());

        let shared: Vec<u32> = self.signatures[source_archetype]
            .components
            .iter()
            .copied()
            .filter(|index| self.signatures[dest_archetype].contains(*index))
            .collect();
        for component_index in shared {
            let src_col = self.column_position(source_archetype, component_index);
            let dst_col = self.column_position(dest_archetype, component_index);
            append_row_between(
                &mut self.columns,
                source_archetype,
                src_col,
                source_row,
                dest_archetype,
                dst_col,
            );
        }

        self.remove_row(source);
        *self.ensure_location_slot(entity) = Some(Location {
            archetype: dest_archetype as u32,
            row: dest_row,
        });
        let _ = tick;
        dest_row
    }

    fn find_or_create_archetype(&mut self, signature: &Signature) -> u32 {
        if let Some(index) = self
            .signatures
            .iter()
            .position(|existing| existing == signature)
        {
            return index as u32;
        }

        let mut columns = Vec::new();
        for component_index in &signature.components {
            let factory =
                self.column_factories[*component_index as usize].expect("table component factory");
            columns.push(factory());
        }

        let index = self.signatures.len() as u32;
        self.signatures.push(signature.clone());
        self.columns.push(columns);
        self.entity_slots.push(Vec::new());
        index
    }

    fn remove_row(&mut self, location: Location) {
        let archetype = location.archetype as usize;
        let row = location.row as usize;
        let moved_slot = self.entity_slots[archetype].swap_remove(row);
        for column in &mut self.columns[archetype] {
            column.swap_remove_row(row);
        }
        if row < self.entity_slots[archetype].len() {
            let repaired_slot = self.entity_slots[archetype][row];
            self.locations[repaired_slot as usize] = Some(Location {
                archetype: location.archetype,
                row: row as u32,
            });
        }
        let _ = moved_slot;
    }

    fn column_position(&self, archetype: usize, component_index: u32) -> usize {
        self.signatures[archetype]
            .components
            .iter()
            .position(|index| *index == component_index)
            .expect("component in archetype")
    }

    fn signature_for(&self, entity: EntityId) -> Signature {
        self.location(entity)
            .map(|location| self.signatures[location.archetype as usize].clone())
            .unwrap_or_else(Signature::empty)
    }

    fn location(&self, entity: EntityId) -> Option<Location> {
        self.locations
            .get(entity.slot() as usize)
            .and_then(|location| *location)
    }

    fn ensure_location_slot(&mut self, entity: EntityId) -> &mut Option<Location> {
        let slot = entity.slot() as usize;
        while self.locations.len() <= slot {
            self.locations.push(None);
        }
        &mut self.locations[slot]
    }

    fn clear_location(&mut self, entity: EntityId) {
        if let Some(slot) = self.locations.get_mut(entity.slot() as usize) {
            *slot = None;
        }
    }
}

pub(crate) fn table_column_factory<T: Clone + 'static>() -> TableColumnFactory {
    fn factory<T: Clone + 'static>() -> Box<dyn ErasedTableColumn> {
        Box::new(TypedTableColumn::<T>::new())
    }
    factory::<T>
}

fn append_row_between(
    columns: &mut [Vec<Box<dyn ErasedTableColumn>>],
    source_archetype: usize,
    source_column: usize,
    source_row: usize,
    dest_archetype: usize,
    dest_column: usize,
) {
    if source_archetype < dest_archetype {
        let (left, right) = columns.split_at_mut(dest_archetype);
        left[source_archetype][source_column]
            .append_row_from(source_row, right[0][dest_column].as_mut());
    } else {
        let (left, right) = columns.split_at_mut(source_archetype);
        right[0][source_column]
            .append_row_from(source_row, left[dest_archetype][dest_column].as_mut());
    }
}

impl ComponentRegistry {
    pub(crate) fn is_table_component(&self, component_id: &ComponentId) -> bool {
        matches!(self.storage_kind(component_id), Some(StorageKind::Table))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::ChangeTick;
    use alloc::vec;

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Health(i32);

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Mana(i32);

    fn entity(slot: u32) -> EntityId {
        EntityId::from_parts(slot, 1)
    }

    fn table_storage() -> ArchetypeStorage {
        let mut factories = vec![None, None];
        factories[0] = Some(table_column_factory::<Health>());
        factories[1] = Some(table_column_factory::<Mana>());
        ArchetypeStorage::new(factories)
    }

    #[test]
    fn insert_replace_and_remove_table_paths() {
        let mut storage = table_storage();
        let tick = ChangeTick::from_raw(1);
        let later = ChangeTick::from_raw(2);
        assert!(storage
            .insert_table(entity(1), 0, Health(3), tick)
            .is_none());
        assert_eq!(
            storage.insert_table(entity(1), 0, Health(9), later),
            Some(Health(3))
        );
        assert_eq!(
            storage.get_table::<Health>(entity(1), 0).map(|h| h.0),
            Some(9)
        );
        assert!(!storage.remove_table_index(entity(1), 1));
        storage.insert_table(entity(1), 1, Mana(2), later);
        assert!(storage.remove_table_index(entity(1), 1));
        assert!(!storage.has_component(entity(1), 1));
        assert!(storage.table_added_tick(entity(1), 0).is_some());
        assert!(storage
            .get_table_mut::<Health>(entity(1), 0, later)
            .is_some());
    }

    #[test]
    fn get_two_table_mut_rejects_duplicate_index_and_missing_components() {
        let mut storage = table_storage();
        let tick = ChangeTick::from_raw(1);
        storage.insert_table(entity(2), 0, Health(1), tick);
        assert!(storage
            .get_two_table_mut::<Health, Mana>(entity(2), 0, 0, tick)
            .is_none());
        assert!(storage
            .get_two_table_mut::<Health, Mana>(entity(2), 0, 1, tick)
            .is_none());
        storage.insert_table(entity(2), 1, Mana(4), tick);
        let (health, mana) = storage
            .get_two_table_mut::<Health, Mana>(entity(2), 0, 1, tick)
            .expect("pair");
        assert_eq!(health.0, 1);
        assert_eq!(mana.0, 4);
    }

    #[test]
    fn has_component_and_remove_entity_clear_location() {
        let mut storage = table_storage();
        let tick = ChangeTick::from_raw(3);
        storage.insert_table(entity(3), 0, Health(2), tick);
        assert!(storage.has_component(entity(3), 0));
        assert!(!storage.has_component(entity(9), 0));
        storage.remove_entity(entity(3));
        assert!(!storage.has_component(entity(3), 0));
    }

    #[test]
    fn place_entity_reuses_row_when_signature_unchanged() {
        let mut storage = table_storage();
        let tick = ChangeTick::from_raw(4);
        storage.insert_table(entity(4), 0, Health(1), tick);
        let before = storage.get_table::<Health>(entity(4), 0).copied();
        storage.insert_table(entity(4), 0, Health(2), tick);
        assert_eq!(
            storage.get_table::<Health>(entity(4), 0).copied(),
            before.map(|_| Health(2))
        );
    }

    #[test]
    fn table_tick_and_mut_access_reject_missing_component_index() {
        let mut storage = table_storage();
        let tick = ChangeTick::from_raw(5);
        storage.insert_table(entity(6), 0, Health(4), tick);
        assert!(storage
            .get_table_mut::<Health>(entity(6), 1, tick)
            .is_none());
        assert!(storage.table_added_tick(entity(6), 1).is_none());
        assert!(storage.table_changed_tick(entity(6), 1).is_none());
    }

    #[test]
    fn insert_second_table_component_reuses_existing_row_when_present() {
        let mut storage = table_storage();
        let tick = ChangeTick::from_raw(7);
        storage.insert_table(entity(8), 0, Health(1), tick);
        storage.insert_table(entity(9), 0, Health(2), tick);
        storage.insert_table(entity(8), 1, Mana(3), tick);
        storage.insert_table(entity(9), 1, Mana(4), tick);
        assert_eq!(
            storage.get_table::<Mana>(entity(8), 1).copied(),
            Some(Mana(3))
        );
        assert_eq!(
            storage.get_table::<Mana>(entity(9), 1).copied(),
            Some(Mana(4))
        );
    }

    #[test]
    fn migration_column_write_uses_replace_when_len_exceeds_row() {
        let mut storage = table_storage();
        let tick = ChangeTick::from_raw(9);
        storage.insert_table(entity(40), 0, Health(1), tick);
        storage.insert_table(entity(40), 1, Mana(1), tick);
        let archetype = storage.location(entity(40)).expect("located").archetype as usize;
        storage.write_migration_column_for_test(archetype, 1, 0, Mana(7), tick);
        assert_eq!(
            storage.get_table::<Mana>(entity(40), 1).map(|m| m.0),
            Some(7)
        );
    }

    #[test]
    fn insert_table_migration_replaces_when_target_column_is_longer_than_row() {
        let mut storage = table_storage();
        let tick = ChangeTick::from_raw(8);
        storage.insert_table(entity(20), 0, Health(1), tick);
        storage.insert_table(entity(20), 1, Mana(3), tick);
        storage.insert_table(entity(21), 0, Health(2), tick);
        let archetype = storage.location(entity(20)).expect("located").archetype as usize;
        storage.write_migration_column_for_test(archetype, 1, 1, Mana(99), tick);
        storage.insert_table(entity(21), 1, Mana(5), tick);
        assert_eq!(
            storage.get_table::<Mana>(entity(21), 1).map(|m| m.0),
            Some(5)
        );
    }

    #[test]
    fn place_entity_returns_existing_row_for_current_archetype() {
        let mut storage = table_storage();
        let tick = ChangeTick::from_raw(6);
        storage.insert_table(entity(7), 0, Health(1), tick);
        let location = storage.location(entity(7)).expect("location");
        let signature = storage.signature_for(entity(7));
        let row = storage.place_entity(entity(7), &signature, tick);
        assert_eq!(row, location.row);
    }
}
