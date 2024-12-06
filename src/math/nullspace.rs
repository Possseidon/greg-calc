use std::{iter::repeat, mem::replace};

use bitvec::vec::BitVec;
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};

pub fn nullspace(mut matrix: impl AsMut<[Rational]>, columns: usize) -> (BitVec, Vec<Rational>) {
    let matrix = matrix.as_mut();
    assert!(matrix.len() % columns == 0);
    let rows = matrix.len() / columns;

    let mut unknowns: BitVec = BitVec::with_capacity(rows);
    let mut nullspace = Vec::new();

    let mut pivot_row_index = 0;
    for pivot_column in 0..columns {
        let pivot_index = pivot_row_index + pivot_column;
        let width = columns - pivot_column;

        let Some(first_non_zero_index) = (pivot_index..matrix.len())
            .step_by(columns)
            .find(|index| matrix[*index] != 0)
        else {
            let mut parameters = matrix.iter().skip(pivot_column).step_by(columns).cloned();
            nullspace.extend(
                unknowns
                    .iter()
                    .map(|is_unknown| {
                        if *is_unknown {
                            Rational::ZERO
                        } else {
                            -parameters
                                .next()
                                .expect("parameters should not be depleted")
                        }
                    })
                    .chain([Rational::ONE])
                    .chain(repeat(Rational::ZERO))
                    .take(columns),
            );
            unknowns.push(true);
            continue;
        };
        unknowns.push(false);

        if first_non_zero_index != pivot_index {
            let (before, non_zero_row) = matrix.split_at_mut(first_non_zero_index);
            before[pivot_index..pivot_index + width].swap_with_slice(&mut non_zero_row[..width]);
        }

        for row_index in (pivot_column..matrix.len()).step_by(columns) {
            if row_index == pivot_index {
                let factor = replace(&mut matrix[pivot_index], Rational::ONE);
                for value in &mut matrix[pivot_index + 1..pivot_index + width] {
                    *value /= factor.clone();
                }
            } else {
                let factor = matrix[row_index].clone() / &matrix[pivot_index];
                matrix[row_index] = Rational::ZERO;
                for column in 1..width {
                    matrix[row_index + column] -= factor.clone() * &matrix[pivot_index + column];
                }
            }
        }

        pivot_row_index += columns;
    }

    (unknowns, nullspace)
}
