#![forbid(unsafe_code)]

use galileo_core::state::Run as CoreRun;
use galileo_core::{engine, eval, why, Fact, SourceSpan};
use std::cell::{Ref, RefCell};
use std::collections::BTreeMap;
use std::sync::Arc;

mod image;
pub use image::Program;

pub use galileo_core::state::{Checkpoint, Rejection, RejectionKind, State, Tuple};
pub use galileo_core::{EvalError, Value, Verdict, View};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub revision: u64,
    pub effects: Vec<Tuple>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Rejected(Rejection),
    Evaluation(EvalError),
    Image(String),
    WrongProgram,
}

impl From<EvalError> for Error {
    fn from(error: EvalError) -> Self {
        Self::Evaluation(error)
    }
}

pub trait Event {
    fn tuple(&self, program: &Program) -> Tuple;
}

impl Event for Tuple {
    fn tuple(&self, _program: &Program) -> Tuple {
        self.clone()
    }
}

pub trait Projection: Sized {
    type Arguments;
    fn view(program: &Program) -> &View;
    fn arguments(arguments: Self::Arguments) -> Vec<Value>;
    fn decode(value: &Value) -> Result<Self, EvalError>;
}

struct CachedView {
    view: View,
    arguments: Vec<Value>,
    record: Value,
    reads: BTreeMap<String, Vec<(galileo_core::RelationId, Option<Arc<eval::Tuples>>)>>,
}

pub struct Session<'p> {
    program: &'p Program,
    run: CoreRun,
    views: RefCell<Vec<CachedView>>,
}

impl<'p> Session<'p> {
    pub fn new(program: &'p Program) -> Self {
        Self {
            program,
            run: CoreRun::new(program),
            views: RefCell::new(Vec::new()),
        }
    }

    pub fn restore(program: &'p Program, checkpoint: Checkpoint) -> Result<Self, Error> {
        if checkpoint.program_identity != Some(program.identity.0) {
            return Err(Error::WrongProgram);
        }
        Ok(Self {
            program,
            run: CoreRun::restore(checkpoint),
            views: RefCell::new(Vec::new()),
        })
    }

    pub fn checkpoint(&self) -> Checkpoint {
        let mut checkpoint = self.run.checkpoint();
        checkpoint.program_identity = Some(self.program.identity.0);
        checkpoint
    }

    pub fn revision(&self) -> u64 {
        self.run.revision
    }

    pub fn state(&self) -> &State {
        &self.run.state
    }

    pub fn pending(&self) -> &[Tuple] {
        self.run.pending()
    }

    pub fn run<E: Event>(&mut self, event: E) -> Result<Step, Error> {
        self.run_with_budget(event.tuple(self.program), Default::default())
    }

    pub fn run_with_budget(
        &mut self,
        event: Tuple,
        budget: galileo_core::Budget,
    ) -> Result<Step, Error> {
        self.step(&[event], budget)
    }

    pub fn run_all<E: Event>(&mut self, events: &[E]) -> Result<Step, Error> {
        let events = events
            .iter()
            .map(|event| event.tuple(self.program))
            .collect::<Vec<_>>();
        self.step(&events, Default::default())
    }

    fn step(&mut self, events: &[Tuple], budget: galileo_core::Budget) -> Result<Step, Error> {
        let record = self.run.run_all(self.program, events, budget)?;
        match &record.result {
            Ok(effects) => Ok(Step {
                revision: record.revision,
                effects: effects.clone(),
            }),
            Err(rejection) => Err(Error::Rejected(rejection.clone())),
        }
    }

    pub fn view<V: Projection>(&self, arguments: V::Arguments) -> Result<V, EvalError> {
        let record = self.cached_project(V::view(self.program), &V::arguments(arguments))?;
        V::decode(&record)
    }

    pub fn project(&self, view: &View, arguments: &[Value]) -> Result<Value, EvalError> {
        Ok(self.cached_project(view, arguments)?.clone())
    }

    fn cached_project(
        &self,
        view: &View,
        arguments: &[Value],
    ) -> Result<Ref<'_, Value>, EvalError> {
        if view.parameters.len() != arguments.len() {
            return Err(EvalError::Type {
                at: view.name.clone(),
                message: format!(
                    "expected {} arguments, got {}",
                    view.parameters.len(),
                    arguments.len()
                ),
            });
        }
        let model = self.run.model(self.program)?;
        let mut views = self.views.borrow_mut();
        let index = views
            .iter()
            .position(|cached| cached.view == *view && cached.arguments == arguments)
            .unwrap_or_else(|| {
                views.push(CachedView {
                    view: view.clone(),
                    arguments: arguments.to_vec(),
                    record: Value::Record(BTreeMap::new()),
                    reads: view
                        .fields
                        .iter()
                        .map(|(name, expression)| {
                            (
                                name.clone(),
                                galileo_core::check::expression_reads(self.program, expression)
                                    .into_iter()
                                    .map(|relation| (relation, None))
                                    .collect(),
                            )
                        })
                        .collect(),
                });
                views.len() - 1
            });
        let cached = &mut views[index];
        let Value::Record(fields) = &cached.record else {
            unreachable!()
        };
        let mut budget = Default::default();
        let mut changed = Vec::new();
        for (name, expression) in &view.fields {
            let reads = cached
                .reads
                .get(name)
                .expect("each field has recorded reads");
            if !fields.contains_key(name)
                || reads.iter().any(|(relation, previous)| {
                    match (previous, model.relations.get(relation)) {
                        (Some(previous), Some(current)) => !Arc::ptr_eq(previous, current),
                        (None, None) => false,
                        _ => true,
                    }
                })
            {
                let value = eval::project_field(
                    self.program,
                    &model,
                    view,
                    arguments,
                    expression,
                    &mut budget,
                )?;
                changed.push((name.clone(), value));
            }
        }
        let Value::Record(fields) = &mut cached.record else {
            unreachable!()
        };
        for (name, value) in changed {
            fields.insert(name.clone(), value);
            for (relation, previous) in cached
                .reads
                .get_mut(&name)
                .expect("each field has recorded reads")
            {
                *previous = model.relations.get(relation).cloned();
            }
        }
        drop(views);
        Ok(Ref::map(self.views.borrow(), |views| &views[index].record))
    }

    pub fn why(&self, sentence: &Tuple) -> Result<Verdict<String, ()>, EvalError> {
        explain(self.program, &self.run, self.revision(), sentence)
    }

    pub fn as_of(&self, revision: u64) -> Result<Snapshot<'_>, EvalError> {
        let state = self
            .run
            .history
            .as_of(revision)
            .ok_or_else(|| EvalError::Type {
                at: "as of".into(),
                message: format!(
                    "revision {revision} is past the last revision {}",
                    self.revision()
                ),
            })?;
        Ok(Snapshot {
            program: self.program,
            run: &self.run,
            state,
            revision,
        })
    }
}

pub struct Snapshot<'a> {
    program: &'a Program,
    run: &'a CoreRun,
    state: State,
    revision: u64,
}

impl Snapshot<'_> {
    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn view<V: Projection>(&self, arguments: V::Arguments) -> Result<V, EvalError> {
        V::decode(&self.project(V::view(self.program), &V::arguments(arguments))?)
    }

    pub fn project(&self, view: &View, arguments: &[Value]) -> Result<Value, EvalError> {
        project(self.program, &self.state, view, arguments)
    }

    pub fn why(&self, sentence: &Tuple) -> Result<Verdict<String, ()>, EvalError> {
        explain(self.program, self.run, self.revision, sentence)
    }
}

fn project(
    program: &Program,
    state: &State,
    view: &View,
    arguments: &[Value],
) -> Result<Value, EvalError> {
    let mut current = (**program).clone();
    current.entities.retain(|entity| {
        !program
            .entity_types
            .iter()
            .any(|ty| ty.state && ty.name == entity.entity_type)
    });
    current.entities.extend(state.live.iter().cloned());
    let facts: Vec<_> = state
        .facts
        .iter()
        .map(|(relation, values)| Fact::new(*relation, values.clone()))
        .collect();
    let model = engine::evaluate(&current, &facts, Default::default())?;
    eval::project(&current, &model, view, arguments, Default::default())
}

fn explain(
    program: &Program,
    run: &CoreRun,
    revision: u64,
    sentence: &Tuple,
) -> Result<Verdict<String, ()>, EvalError> {
    let sources: Vec<_> = run
        .log
        .iter()
        .enumerate()
        .map(|(index, _)| SourceSpan {
            file: "runtime".into(),
            start: 0,
            end: 0,
            line: index + 1,
            column: 1,
        })
        .collect();
    why::why_history(
        program,
        run,
        &sources,
        revision,
        sentence.0,
        &sentence.1,
        Default::default(),
    )
}
