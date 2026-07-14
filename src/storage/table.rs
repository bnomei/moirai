//! Typed and erased component table columns.
//!
//! Each [`TypedTableColumn`] is a dense value column with parallel added/changed tick arrays.
//! [`ErasedTableColumn`] supports archetype migration without monomorphizing over every component.

use alloc::boxed::Box;
use core::any::{Any, TypeId};

use crate::time::ChangeTick;

/// Column operations used while relocating entities between archetype tables.
pub(crate) trait ErasedTableColumn: Any {
    fn len(&self) -> usize;
    #[allow(dead_code)]
    fn type_id(&self) -> TypeId;
    fn take_row(&mut self, row: usize) -> ErasedTableRow;
    fn move_row_to(&mut self, row: usize, destination: &mut dyn ErasedTableColumn);
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn append_value(&mut self, value: Box<dyn Any>, tick: ChangeTick) -> usize;
    fn replace_value(
        &mut self,
        row: usize,
        value: Box<dyn Any>,
        tick: ChangeTick,
    ) -> Option<Box<dyn Any>>;
    fn get_value(&self, row: usize) -> Option<&dyn Any>;
    fn get_value_mut(&mut self, row: usize, tick: ChangeTick) -> Option<&mut dyn Any>;
    fn added_tick(&self, row: usize) -> Option<ChangeTick>;
    fn changed_tick(&self, row: usize) -> Option<ChangeTick>;
}

/// One boxed component value removed from a table column during migration.
pub(crate) struct ErasedTableRow {
    value: Box<dyn Any>,
}

impl ErasedTableRow {
    pub(crate) fn into_value(self) -> Box<dyn Any> {
        self.value
    }
}

/// Dense component column inside one archetype table.
pub(crate) struct TypedTableColumn<T: 'static> {
    data: alloc::vec::Vec<T>,
    added: alloc::vec::Vec<u64>,
    changed: alloc::vec::Vec<u64>,
}

impl<T: 'static> TypedTableColumn<T> {
    pub fn new() -> Self {
        Self {
            data: alloc::vec::Vec::new(),
            added: alloc::vec::Vec::new(),
            changed: alloc::vec::Vec::new(),
        }
    }

    fn stamp_row(&mut self, row: usize, tick: ChangeTick, is_new: bool) {
        let raw = tick.raw();
        if is_new {
            self.added[row] = raw;
        }
        self.changed[row] = raw;
    }
}

impl<T: 'static> ErasedTableColumn for TypedTableColumn<T> {
    fn len(&self) -> usize {
        self.data.len()
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn take_row(&mut self, row: usize) -> ErasedTableRow {
        let value = Box::new(self.data.swap_remove(row));
        let _ = self.added.swap_remove(row);
        let _ = self.changed.swap_remove(row);
        ErasedTableRow { value }
    }

    fn move_row_to(&mut self, row: usize, destination: &mut dyn ErasedTableColumn) {
        let destination = destination
            .as_any_mut()
            .downcast_mut::<TypedTableColumn<T>>()
            .expect("table column type mismatch");
        destination.data.push(self.data.swap_remove(row));
        destination.added.push(self.added.swap_remove(row));
        destination.changed.push(self.changed.swap_remove(row));
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn append_value(&mut self, value: Box<dyn Any>, tick: ChangeTick) -> usize {
        let value = *value.downcast::<T>().expect("table column type mismatch");
        let row = self.data.len();
        let raw = tick.raw();
        self.data.push(value);
        self.added.push(raw);
        self.changed.push(raw);
        row
    }

    fn replace_value(
        &mut self,
        row: usize,
        value: Box<dyn Any>,
        tick: ChangeTick,
    ) -> Option<Box<dyn Any>> {
        let value = *value.downcast::<T>().expect("table column type mismatch");
        if row >= self.data.len() {
            self.append_value(Box::new(value), tick);
            return None;
        }
        let replaced = core::mem::replace(&mut self.data[row], value);
        let fresh = self.added[row] == 0 && self.changed[row] == 0;
        self.stamp_row(row, tick, fresh);
        Some(Box::new(replaced))
    }

    fn get_value(&self, row: usize) -> Option<&dyn Any> {
        self.data.get(row).map(|value| value as &dyn Any)
    }

    fn get_value_mut(&mut self, row: usize, tick: ChangeTick) -> Option<&mut dyn Any> {
        let value = self.data.get_mut(row)?;
        self.changed[row] = tick.raw();
        Some(value as &mut dyn Any)
    }

    fn added_tick(&self, row: usize) -> Option<ChangeTick> {
        self.added.get(row).copied().map(ChangeTick::from_raw)
    }

    fn changed_tick(&self, row: usize) -> Option<ChangeTick> {
        self.changed.get(row).copied().map(ChangeTick::from_raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Score(i32);

    #[test]
    fn type_id_reports_component_type() {
        let column = TypedTableColumn::<Score>::new();
        assert_eq!(ErasedTableColumn::type_id(&column), TypeId::of::<Score>());
    }

    #[test]
    fn replace_value_appends_when_row_is_out_of_range() {
        let mut column = TypedTableColumn::<Score>::new();
        let tick = ChangeTick::from_raw(3);
        assert!(column.replace_value(4, Box::new(Score(7)), tick).is_none());
        assert_eq!(column.len(), 1);
        assert_eq!(
            column
                .get_value(0)
                .and_then(|value| value.downcast_ref::<Score>()),
            Some(&Score(7))
        );
    }

    #[test]
    fn replace_value_refreshes_added_tick_for_unstamped_rows() {
        let mut column = TypedTableColumn::<Score>::new();
        let tick = ChangeTick::from_raw(8);
        let row = column.append_value(Box::new(Score(1)), ChangeTick::from_raw(1));
        column.added[row] = 0;
        column.changed[row] = 0;
        assert!(column
            .replace_value(row, Box::new(Score(2)), tick)
            .is_some());
        assert_eq!(column.added[row], tick.raw());
    }

    #[test]
    fn stamp_row_marks_added_for_fresh_rows() {
        let mut column = TypedTableColumn::<Score>::new();
        let first = ChangeTick::from_raw(1);
        let row = column.append_value(Box::new(Score(1)), first);
        let second = ChangeTick::from_raw(2);
        let _ = column.replace_value(row, Box::new(Score(2)), second);
        assert_eq!(column.added_tick(row), Some(first));
        assert_eq!(column.changed_tick(row), Some(second));
    }
}
