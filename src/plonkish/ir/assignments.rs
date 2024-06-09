use std::{
    collections::HashMap,
    fmt,
    hash::Hash,
    ops::{Deref, DerefMut},
};

use crate::{field::Field, wit_gen::AutoTraceGenerator};

use halo2_proofs::plonk::{Advice, Column as Halo2Column};

use crate::{
    plonkish::compiler::{cell_manager::Placement, step_selector::StepSelector},
    sbpir::{query::Queriable, ForwardSignal, SharedSignal, StepTypeUUID},
    util::UUID,
    wit_gen::{StepInstance, TraceGenerator, TraceWitness},
};

use super::{Column, PolyExpr};

#[derive(Debug, Clone)]
pub struct Assignments<F>(pub HashMap<Column, Vec<F>>);

impl<F: fmt::Debug> fmt::Display for Assignments<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // get the decimal width based on the step_instances size, add extra one leading zero
        let decimal_width = self.0.len().checked_ilog10().unwrap_or(0) + 2;
        // offset(col_uuid): value0, value1, value2,...
        for (i, (col, vals)) in self.0.iter().enumerate() {
            let vals = vals.iter().fold(String::new(), |mut acc, val| {
                acc.push_str(&format!("{:?}, ", val));
                acc
            });
            writeln!(
                f,
                "{:0>width$}({}): {}",
                i,
                col.id,
                vals,
                width = decimal_width as usize,
            )?;
        }
        Ok(())
    }
}

impl<F> Default for Assignments<F> {
    fn default() -> Self {
        Self(HashMap::default())
    }
}

impl<F> Deref for Assignments<F> {
    type Target = HashMap<Column, Vec<F>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<F> DerefMut for Assignments<F> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct AssignmentGenerator<F, TraceArgs> {
    columns: Vec<Column>,
    placement: Placement,
    selector: StepSelector<F>,
    trace_gen: TraceGenerator<F, TraceArgs>,
    auto_trace_gen: AutoTraceGenerator<F>,

    num_rows: usize,

    ir_id: UUID,
}

impl<F: Clone, TraceArgs> Clone for AssignmentGenerator<F, TraceArgs> {
    fn clone(&self) -> Self {
        Self {
            columns: self.columns.clone(),
            placement: self.placement.clone(),
            selector: self.selector.clone(),
            trace_gen: self.trace_gen.clone(),
            auto_trace_gen: self.auto_trace_gen.clone(),
            num_rows: self.num_rows,
            ir_id: self.ir_id,
        }
    }
}

impl<F: Clone, TraceArgs> Default for AssignmentGenerator<F, TraceArgs> {
    fn default() -> Self {
        Self {
            columns: Default::default(),
            placement: Default::default(),
            selector: Default::default(),
            trace_gen: Default::default(),
            auto_trace_gen: Default::default(),
            num_rows: Default::default(),
            ir_id: Default::default(),
        }
    }
}

impl<F: Field + Hash, TraceArgs> AssignmentGenerator<F, TraceArgs> {
    pub fn new(
        columns: Vec<Column>,
        placement: Placement,
        selector: StepSelector<F>,
        trace_gen: TraceGenerator<F, TraceArgs>,
        auto_trace_gen: AutoTraceGenerator<F>,
        num_rows: usize,
        ir_id: UUID,
    ) -> Self {
        Self {
            columns,
            placement,
            selector,
            trace_gen,
            auto_trace_gen,
            num_rows,
            ir_id,
        }
    }

    pub fn empty(ir_id: UUID) -> Self {
        Self {
            ir_id,
            ..Default::default()
        }
    }

    pub fn generate_trace_witness(&self, args: TraceArgs) -> TraceWitness<F> {
        self.trace_gen.generate(args)
    }

    pub fn generate(&self, args: TraceArgs) -> Assignments<F> {
        let witness = self.generate_trace_witness(args);

        self.generate_with_witness(witness)
    }

    pub fn generate_with_witness(&self, witness: TraceWitness<F>) -> Assignments<F> {
        let mut offset: usize = 0;
        let mut assignments: Assignments<F> = Default::default();

        let witness = self.auto_trace_gen.generate(witness);

        for step_instance in witness.step_instances.into_iter() {
            self.assign_step(&mut offset, &mut assignments, &step_instance);
        }

        assignments
    }

    pub fn uuid(&self) -> UUID {
        self.ir_id
    }

    fn assign_step(
        &self,
        offset: &mut usize,
        assignments: &mut Assignments<F>,
        step_instance: &StepInstance<F>,
    ) {
        for (lhs, rhs) in step_instance.assignments.iter() {
            self.assign(offset, assignments, step_instance.step_type_uuid, lhs, rhs);
        }

        let selector_assignment = self
            .selector
            .get_selector_assignment(step_instance.step_type_uuid);

        for (expr, value) in selector_assignment.iter() {
            match expr {
                PolyExpr::Query((column, rot, _)) => {
                    self.set_value(assignments, column.clone(), *offset + *rot as usize, value)
                }
                _ => panic!("wrong type of expresion is selector assignment"),
            }
        }

        *offset += self.placement.step_height(step_instance.step_type_uuid) as usize;
    }

    fn assign(
        &self,
        offset: &mut usize,
        assignments: &mut Assignments<F>,
        step_uuid: StepTypeUUID,
        lhs: &Queriable<F>,
        value: &F,
    ) {
        let (column, rotation) = self.find_placement(step_uuid, lhs);

        let offset = (*offset as i32 + rotation) as usize;

        self.set_value(assignments, column, offset, value);
    }

    fn find_placement(&self, step_uuid: StepTypeUUID, query: &Queriable<F>) -> (Column, i32) {
        match query {
            Queriable::Internal(signal) => self
                .placement
                .find_internal_signal_placement(step_uuid, signal)
                .into(),

            Queriable::Forward(forward, next) => {
                self.get_forward_placement(step_uuid, forward, *next)
            }

            Queriable::Shared(shared, rot) => self.get_shared_placement(shared, *rot),

            Queriable::Halo2AdviceQuery(signal, rotation) => {
                let column = self
                    .find_halo2_advice_native(signal.column)
                    .expect("column not found");

                (column, *rotation)
            }

            _ => panic!("invalid advice assignment on queriable {:?}", query),
        }
    }

    fn set_value(
        &self,
        assignments: &mut Assignments<F>,
        column: Column,
        offset: usize,
        value: &F,
    ) {
        if let Some(column_assignments) = assignments.get_mut(&column) {
            column_assignments[offset] = *value;
        } else {
            let mut column_assignments = vec![F::ZERO; self.num_rows];
            column_assignments[offset] = *value;

            assignments.insert(column, column_assignments);
        }
    }

    fn get_forward_placement(
        &self,
        step_uuid: StepTypeUUID,
        forward: &ForwardSignal,
        next: bool,
    ) -> (Column, i32) {
        let placement = self
            .placement
            .get_forward_placement(forward)
            .expect("forward signal placement not found");

        let super_rotation = placement.rotation
            + if next {
                self.placement.step_height(step_uuid) as i32
            } else {
                0
            };

        (placement.column, super_rotation)
    }

    fn get_shared_placement(&self, shared: &SharedSignal, rotation: i32) -> (Column, i32) {
        let placement = self
            .placement
            .get_shared_placement(shared)
            .expect("shared signal not found");

        let super_rotation =
            placement.rotation + rotation * (self.placement.first_step_height() as i32);

        (placement.column, super_rotation)
    }

    fn find_halo2_advice_native(&self, halo2_advice: Halo2Column<Advice>) -> Option<Column> {
        for column in self.columns.iter() {
            if let Some(advice) = column.halo2_advice {
                if advice.column == halo2_advice {
                    return Some(column.clone());
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_print_assignments() {
        let display = format!(
            "{}",
            Assignments::<i32>(HashMap::from([
                (Column::advice("a", 1), vec![1, 2, 3]),
                (Column::fixed("a"), vec![4, 5, 6]),
            ])),
        );
        println!("{}", display);
    }
}
