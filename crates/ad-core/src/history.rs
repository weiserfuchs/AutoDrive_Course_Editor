//! Linear undo/redo history with saved-state tracking (AD-4).

use std::collections::VecDeque;

use crate::command::Transaction;
use crate::network::{Network, NetworkError};

/// Identity of a history entry. Never reused within one `History`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId(u64);

/// Default maximum number of entries kept.
pub const DEFAULT_CAP: usize = 500;

/// A point in the history: below the oldest entry, or just after an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Position {
    Base,
    After(EntryId),
}

#[derive(Debug)]
struct Entry {
    id: EntryId,
    tx: Transaction,
}

#[derive(Debug)]
pub struct History {
    entries: VecDeque<Entry>,
    /// Number of entries currently applied; `entries[..cursor]` are applied.
    cursor: usize,
    cap: usize,
    next_id: u64,
    /// `None` once the saved position is lost (evicted or discarded).
    saved: Option<Position>,
}

impl Default for History {
    fn default() -> Self {
        Self::new(DEFAULT_CAP)
    }
}

impl History {
    /// A cap of 0 is treated as 1.
    pub fn new(cap: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            cursor: 0,
            cap: cap.max(1),
            next_id: 0,
            saved: Some(Position::Base),
        }
    }

    fn current(&self) -> Position {
        match self.cursor.checked_sub(1).and_then(|i| self.entries.get(i)) {
            Some(e) => Position::After(e.id),
            None => Position::Base,
        }
    }

    /// Applies the transaction as one new entry and discards the redo tail. On error the
    /// network and the history are unchanged.
    pub fn apply(
        &mut self,
        net: &mut Network,
        mut tx: Transaction,
    ) -> Result<EntryId, NetworkError> {
        tx.apply(net)?;
        if let Some(Position::After(saved)) = self.saved
            && self.entries.iter().skip(self.cursor).any(|e| e.id == saved)
        {
            self.saved = None;
        }
        self.entries.truncate(self.cursor);
        let id = EntryId(self.next_id);
        self.next_id += 1;
        self.entries.push_back(Entry { id, tx });
        self.cursor += 1;
        if self.entries.len() > self.cap
            && let Some(evicted) = self.entries.pop_front()
        {
            self.cursor -= 1;
            if matches!(self.saved, Some(Position::Base))
                || self.saved == Some(Position::After(evicted.id))
            {
                self.saved = None;
            }
        }
        Ok(id)
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.entries.len()
    }

    /// Returns false when there is nothing to undo, or the revert failed.
    pub fn undo(&mut self, net: &mut Network) -> bool {
        let Some(index) = self.cursor.checked_sub(1) else {
            return false;
        };
        let Some(entry) = self.entries.get_mut(index) else {
            return false;
        };
        if entry.tx.revert(net).is_err() {
            return false;
        }
        self.cursor = index;
        true
    }

    /// Returns false when there is nothing to redo, or the apply failed.
    pub fn redo(&mut self, net: &mut Network) -> bool {
        let Some(entry) = self.entries.get_mut(self.cursor) else {
            return false;
        };
        if entry.tx.apply(net).is_err() {
            return false;
        }
        self.cursor += 1;
        true
    }

    pub fn mark_saved(&mut self) {
        self.saved = Some(self.current());
    }

    pub fn is_dirty(&self) -> bool {
        self.saved != Some(self.current())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::command::tests::{linked, snap};
    use crate::command::{AddNode, Command, Connect, DeleteNodes, Disconnect};
    use crate::network::{ConnectionShape, NodeFlag, NodeId};

    fn add(x: f64) -> Transaction {
        Transaction::new(vec![Command::AddNode(AddNode::new(
            x,
            0.0,
            0.0,
            NodeFlag::REGULAR,
        ))])
    }

    #[test]
    fn nothing_to_undo_or_redo() {
        let (mut n, _) = linked();
        let before = snap(&n);
        let mut h = History::default();
        assert!(!h.can_undo() && !h.can_redo());
        assert!(!h.undo(&mut n) && !h.redo(&mut n));
        h.apply(&mut n, add(1.0)).expect("apply");
        assert!(!h.redo(&mut n));
        assert!(h.undo(&mut n));
        assert!(!h.undo(&mut n));
        assert_eq!(snap(&n), before);
    }

    #[test]
    fn undo_redo_alternate_between_two_states() {
        let (mut n, [a, b, c, _]) = linked();
        let mut h = History::default();
        let before = snap(&n);
        h.apply(
            &mut n,
            Transaction::new(vec![
                Command::Disconnect(Disconnect::new(b, c)),
                Command::Connect(Connect::new(a, c, ConnectionShape::Dual)),
                Command::DeleteNodes(DeleteNodes::new(vec![b])),
            ]),
        )
        .expect("apply");
        let after = snap(&n);
        for _ in 0..4 {
            assert!(h.undo(&mut n));
            assert_eq!(snap(&n), before);
            assert!(h.redo(&mut n));
            assert_eq!(snap(&n), after);
        }
    }

    #[test]
    fn failed_transaction_pushes_no_entry() {
        let (mut n, [a, ..]) = linked();
        let mut ghost_net = Network::new();
        let ghost: NodeId = (0..60)
            .map(|_| ghost_net.add_node(0.0, 0.0, 0.0, NodeFlag(0)))
            .last()
            .expect("id");
        let before = snap(&n);
        let mut h = History::default();
        let tx = Transaction::new(vec![
            Command::AddNode(AddNode::new(1.0, 1.0, 1.0, NodeFlag(0))),
            Command::Connect(Connect::new(a, ghost, ConnectionShape::OneWay)),
        ]);
        assert_eq!(h.apply(&mut n, tx), Err(NetworkError::UnknownNode(ghost)));
        assert_eq!(snap(&n), before);
        assert!(!h.can_undo() && !h.is_dirty());
    }

    #[test]
    fn new_transaction_discards_redo_tail() {
        let mut n = Network::new();
        let mut h = History::default();
        h.apply(&mut n, add(1.0)).expect("a");
        h.apply(&mut n, add(2.0)).expect("b");
        assert!(h.undo(&mut n));
        let c = h.apply(&mut n, add(3.0)).expect("c");
        assert!(!h.can_redo());
        assert!(!h.redo(&mut n));
        assert_eq!(n.len(), 2);
        assert_eq!(h.entries.len(), 2);
        assert!(h.entries.iter().all(|e| e.id != EntryId(1)));
        assert_eq!(c, EntryId(2));
    }

    #[test]
    fn dirty_tracking() {
        let mut n = Network::new();
        let mut h = History::default();
        assert!(!h.is_dirty());
        h.apply(&mut n, add(1.0)).expect("a");
        assert!(h.is_dirty());
        h.mark_saved();
        assert!(!h.is_dirty());
        assert!(h.undo(&mut n));
        assert!(h.is_dirty());
        assert!(h.redo(&mut n));
        assert!(!h.is_dirty());
    }

    #[test]
    fn saved_entry_evicted_stays_dirty() {
        let mut n = Network::new();
        let mut h = History::new(2);
        h.apply(&mut n, add(1.0)).expect("a");
        h.mark_saved();
        for x in [2.0, 3.0, 4.0] {
            h.apply(&mut n, add(x)).expect("apply");
        }
        assert_eq!(h.entries.len(), 2);
        assert!(h.is_dirty());
        while h.undo(&mut n) {
            assert!(h.is_dirty());
        }
        while h.redo(&mut n) {
            assert!(h.is_dirty());
        }
        h.mark_saved();
        assert!(!h.is_dirty());
    }

    #[test]
    fn saved_at_base_lost_when_oldest_is_evicted() {
        let mut n = Network::new();
        let mut h = History::new(1);
        h.apply(&mut n, add(1.0)).expect("a");
        assert!(h.undo(&mut n));
        assert!(!h.is_dirty());
        h.apply(&mut n, add(2.0)).expect("b");
        h.apply(&mut n, add(3.0)).expect("c");
        assert!(h.undo(&mut n));
        assert!(h.is_dirty());
    }

    #[test]
    fn saved_entry_discarded_with_redo_tail() {
        let mut n = Network::new();
        let mut h = History::default();
        h.apply(&mut n, add(1.0)).expect("a");
        h.apply(&mut n, add(2.0)).expect("b");
        h.mark_saved();
        assert!(h.undo(&mut n));
        h.apply(&mut n, add(3.0)).expect("c");
        assert!(h.is_dirty());
        assert!(h.undo(&mut n));
        assert!(h.is_dirty());
        assert!(h.undo(&mut n));
        assert!(h.is_dirty());
    }

    #[test]
    fn entry_ids_are_never_reused() {
        let mut n = Network::new();
        let mut h = History::new(2);
        let mut seen = Vec::new();
        for x in 0..6 {
            seen.push(h.apply(&mut n, add(f64::from(x))).expect("apply"));
            if x % 2 == 1 {
                assert!(h.undo(&mut n));
            }
        }
        let mut sorted = seen.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), seen.len());
    }
}
