use crate::Value;
use std::collections::{BTreeMap, BTreeSet};

pub type Tuple = Vec<Value>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark(usize);

#[derive(Clone, Debug)]
enum Undo {
    Count(Tuple, usize),
    Index(Vec<usize>),
}

#[derive(Clone, Debug, Default)]
pub struct Store {
    counts: BTreeMap<Tuple, usize>,
    tuples: BTreeSet<Tuple>,
    indexes: BTreeMap<Vec<usize>, BTreeMap<Tuple, BTreeSet<Tuple>>>,
    undo: Vec<Undo>,
    unlogged: bool,
}

impl Store {
    pub fn unlogged() -> Store {
        Store { unlogged: true, ..Store::default() }
    }

    pub fn log(&mut self) {
        self.unlogged = false;
    }

    pub fn count(&self, tuple: &[Value]) -> usize {
        self.counts.get(tuple).copied().unwrap_or(0)
    }

    pub fn counts(&self) -> &BTreeMap<Tuple, usize> {
        &self.counts
    }

    pub fn tuples(&self) -> &BTreeSet<Tuple> {
        &self.tuples
    }

    pub fn presence(&self) -> impl Iterator<Item = &Tuple> {
        self.counts.keys()
    }

    pub fn insert(&mut self, tuple: Tuple) -> bool {
        let old = self.count(&tuple);
        if !self.unlogged {
            self.undo.push(Undo::Count(tuple.clone(), old));
        }
        self.raise(tuple, old)
    }

    pub fn add(&mut self, tuple: Tuple) -> bool {
        let old = self.count(&tuple);
        self.raise(tuple, old)
    }

    fn raise(&mut self, tuple: Tuple, old: usize) -> bool {
        let new = old.checked_add(1).expect("tuple derivation count overflow");
        self.set_count(tuple, new);
        old == 0
    }

    pub fn remove(&mut self, tuple: &[Value]) -> bool {
        let old = self.count(tuple);
        if old == 0 {
            return false;
        }
        if !self.unlogged {
            self.undo.push(Undo::Count(tuple.to_vec(), old));
        }
        self.set_count(tuple.to_vec(), old - 1);
        old == 1
    }

    pub fn mark(&self) -> Mark {
        Mark(self.undo.len())
    }

    pub fn undo(&mut self, mark: Mark) {
        assert!(mark.0 <= self.undo.len(), "mark is beyond the undo log");
        while self.undo.len() > mark.0 {
            match self.undo.pop().unwrap() {
                Undo::Count(tuple, count) => self.set_count(tuple, count),
                Undo::Index(columns) => {
                    self.indexes.remove(&columns);
                }
            }
        }
    }

    pub fn changed_since(&self, mark: Mark) -> BTreeMap<&Tuple, bool> {
        let mut was = BTreeMap::new();
        for entry in &self.undo[mark.0..] {
            if let Undo::Count(tuple, count) = entry {
                was.entry(tuple).or_insert(*count > 0);
            }
        }
        was
    }

    pub fn forget(&mut self) {
        self.undo.clear();
    }

    pub fn matching(&mut self, columns: &[usize], key: &[Value]) -> impl Iterator<Item = &Tuple> {
        assert_eq!(columns.len(), key.len());
        self.index(columns);
        self.rows(columns, key).into_iter().flat_map(BTreeSet::iter)
    }

    pub fn rows(&self, columns: &[usize], key: &[Value]) -> Option<&BTreeSet<Tuple>> {
        self.indexes[columns].get(key)
    }

    pub fn index(&mut self, columns: &[usize]) {
        if !self.indexes.contains_key(columns) {
            let mut index: BTreeMap<Tuple, BTreeSet<Tuple>> = BTreeMap::new();
            for tuple in self.counts.keys() {
                index
                    .entry(project(tuple, columns))
                    .or_default()
                    .insert(tuple.clone());
            }
            self.undo.push(Undo::Index(columns.to_vec()));
            self.indexes.insert(columns.to_vec(), index);
        }
    }

    pub fn matching_len(&mut self, columns: &[usize], key: &[Value]) -> usize {
        let _ = self.matching(columns, key);
        self.indexes[columns].get(key).map_or(0, BTreeSet::len)
    }

    pub fn matching_before(
        &mut self,
        columns: &[usize],
        key: &[Value],
        inserted: &BTreeSet<Tuple>,
        deleted: &BTreeSet<Tuple>,
    ) -> BTreeSet<Tuple> {
        let mut rows: BTreeSet<Tuple> = self
            .matching(columns, key)
            .filter(|tuple| !inserted.contains(*tuple))
            .cloned()
            .collect();
        rows.extend(
            deleted
                .iter()
                .filter(|tuple| {
                    columns
                        .iter()
                        .zip(key)
                        .all(|(&column, value)| &tuple[column] == value)
                })
                .cloned(),
        );
        rows
    }

    fn set_count(&mut self, tuple: Tuple, count: usize) {
        let present = self.counts.contains_key(&tuple);
        if present != (count > 0) {
            for (columns, index) in &mut self.indexes {
                let key = project(&tuple, columns);
                if count > 0 {
                    index.entry(key).or_default().insert(tuple.clone());
                } else if let Some(rows) = index.get_mut(&key) {
                    rows.remove(&tuple);
                    if rows.is_empty() {
                        index.remove(&key);
                    }
                }
            }
        }
        if count == 0 {
            self.tuples.remove(&tuple);
            self.counts.remove(&tuple);
        } else {
            self.tuples.insert(tuple.clone());
            self.counts.insert(tuple, count);
        }
    }
}

fn project(tuple: &[Value], columns: &[usize]) -> Tuple {
    columns
        .iter()
        .map(|&column| tuple[column].clone())
        .collect()
}
