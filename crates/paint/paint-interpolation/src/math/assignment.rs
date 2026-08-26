pub(super) fn minimum_cost_pairs(costs: &[Vec<f32>], unmatched: f32) -> Vec<(usize, usize)> {
    if costs.is_empty() || costs[0].is_empty() {
        return Vec::new();
    }
    let rows = costs.len();
    let columns = costs[0].len();
    let size = rows + columns;
    let mut matrix = vec![vec![0.0; size]; size];
    for (row, values) in costs.iter().enumerate() {
        matrix[row][..columns].copy_from_slice(values);
        matrix[row][columns..].fill(unmatched);
    }
    for values in matrix.iter_mut().skip(rows) {
        values[..columns].fill(unmatched);
    }

    let mut row_potential = vec![0.0f32; size + 1];
    let mut column_potential = vec![0.0f32; size + 1];
    let mut matching = vec![0usize; size + 1];
    let mut predecessor = vec![0usize; size + 1];
    for row in 1..=size {
        matching[0] = row;
        let mut column = 0;
        let mut minimum = vec![f32::INFINITY; size + 1];
        let mut used = vec![false; size + 1];
        loop {
            used[column] = true;
            let current_row = matching[column];
            let mut delta = f32::INFINITY;
            let mut next_column = 0;
            for candidate in 1..=size {
                if used[candidate] {
                    continue;
                }
                let cost = matrix[current_row - 1][candidate - 1]
                    - row_potential[current_row]
                    - column_potential[candidate];
                if cost < minimum[candidate] {
                    minimum[candidate] = cost;
                    predecessor[candidate] = column;
                }
                if minimum[candidate] < delta {
                    delta = minimum[candidate];
                    next_column = candidate;
                }
            }
            for candidate in 0..=size {
                if used[candidate] {
                    row_potential[matching[candidate]] += delta;
                    column_potential[candidate] -= delta;
                } else {
                    minimum[candidate] -= delta;
                }
            }
            column = next_column;
            if matching[column] == 0 {
                break;
            }
        }
        loop {
            let previous = predecessor[column];
            matching[column] = matching[previous];
            column = previous;
            if column == 0 {
                break;
            }
        }
    }

    (1..=size)
        .filter_map(|column| {
            let row = matching[column].checked_sub(1)?;
            let column = column - 1;
            (row < rows && column < columns).then_some((row, column))
        })
        .collect()
}
