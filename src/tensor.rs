//! Tensor type, tensor operations, and the shared TensorStore.
//! (PROVIDED — you should not need to modify this file.)

use std::sync::RwLock;

use crate::rng::SplitMix64;

/// A simple 2D tensor (row-major layout).
#[derive(Debug, Clone)]
pub struct Tensor {
    pub data: Vec<f32>,
    pub rows: usize,
    pub cols: usize,
}

impl Tensor {
    /// Create a tensor filled with zeros.
    pub fn zeros(rows: usize, cols: usize) -> Self {
        Self {
            data: vec![0.0; rows * cols],
            rows,
            cols,
        }
    }

    /// Create a tensor filled with deterministic pseudo-random values
    /// in `[-1, 1)` derived from `seed`.
    ///
    /// Using a seed (rather than a global RNG) means every benchmark run
    /// sees identical inputs, so your timings are comparable run-to-run
    /// and your correctness checks are reproducible.
    pub fn seeded(rows: usize, cols: usize, seed: u64) -> Self {
        let mut rng = SplitMix64::new(seed);
        let data: Vec<f32> = (0..rows * cols).map(|_| rng.next_f32_signed()).collect();
        Self { data, rows, cols }
    }

    /// Element at (r, c).
    #[inline]
    pub fn get(&self, r: usize, c: usize) -> f32 {
        self.data[r * self.cols + c]
    }

    /// Mutable reference to element at (r, c).
    #[inline]
    pub fn get_mut(&mut self, r: usize, c: usize) -> &mut f32 {
        &mut self.data[r * self.cols + c]
    }
}

// ═══════════════════════════════════════════════════
// Tensor Operations (all real computation, no sleep)
// ═══════════════════════════════════════════════════

/// C = A @ B  (naive triple-loop matmul).
pub fn matmul(a: &Tensor, b: &Tensor) -> Tensor {
    assert_eq!(
        a.cols, b.rows,
        "matmul dimension mismatch: {}x{} @ {}x{}",
        a.rows, a.cols, b.rows, b.cols
    );
    let mut c = Tensor::zeros(a.rows, b.cols);
    for i in 0..a.rows {
        for k in 0..a.cols {
            let a_ik = a.get(i, k);
            for j in 0..b.cols {
                *c.get_mut(i, j) += a_ik * b.get(k, j);
            }
        }
    }
    c
}

/// C = A + B  (element-wise).
pub fn elem_add(a: &Tensor, b: &Tensor) -> Tensor {
    assert_eq!(
        (a.rows, a.cols),
        (b.rows, b.cols),
        "elem_add shape mismatch"
    );
    let data: Vec<f32> = a.data.iter().zip(&b.data).map(|(x, y)| x + y).collect();
    Tensor {
        data,
        rows: a.rows,
        cols: a.cols,
    }
}

/// C = max(0, A)  (element-wise ReLU).
pub fn relu(a: &Tensor) -> Tensor {
    let data: Vec<f32> = a.data.iter().map(|&x| x.max(0.0)).collect();
    Tensor {
        data,
        rows: a.rows,
        cols: a.cols,
    }
}

/// Row-wise softmax.
pub fn softmax(a: &Tensor) -> Tensor {
    let mut out = Tensor::zeros(a.rows, a.cols);
    for r in 0..a.rows {
        let row_start = r * a.cols;
        let row = &a.data[row_start..row_start + a.cols];
        let max_val = row.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = row.iter().map(|&x| (x - max_val).exp()).collect();
        let sum: f32 = exps.iter().sum();
        for c in 0..a.cols {
            *out.get_mut(r, c) = exps[c] / sum;
        }
    }
    out
}

/// Out-of-place transpose.
pub fn transpose(a: &Tensor) -> Tensor {
    let mut out = Tensor::zeros(a.cols, a.rows);
    for r in 0..a.rows {
        for c in 0..a.cols {
            *out.get_mut(c, r) = a.get(r, c);
        }
    }
    out
}

/// Concatenate two tensors along a given axis.
///   axis=0: stack rows (same cols required)
///   axis=1: stack columns (same rows required)
pub fn concat(a: &Tensor, b: &Tensor, axis: usize) -> Tensor {
    match axis {
        0 => {
            assert_eq!(a.cols, b.cols, "concat axis=0: col mismatch");
            let mut data = a.data.clone();
            data.extend_from_slice(&b.data);
            Tensor {
                data,
                rows: a.rows + b.rows,
                cols: a.cols,
            }
        }
        1 => {
            assert_eq!(a.rows, b.rows, "concat axis=1: row mismatch");
            let mut data = Vec::with_capacity(a.rows * (a.cols + b.cols));
            for r in 0..a.rows {
                data.extend_from_slice(&a.data[r * a.cols..(r + 1) * a.cols]);
                data.extend_from_slice(&b.data[r * b.cols..(r + 1) * b.cols]);
            }
            Tensor {
                data,
                rows: a.rows,
                cols: a.cols + b.cols,
            }
        }
        _ => panic!("concat: unsupported axis {}", axis),
    }
}

/// Layer normalization (per-row: subtract mean, divide by std + eps).
pub fn layer_norm(a: &Tensor) -> Tensor {
    let eps = 1e-5;
    let mut out = Tensor::zeros(a.rows, a.cols);
    for r in 0..a.rows {
        let row_start = r * a.cols;
        let row = &a.data[row_start..row_start + a.cols];
        let mean: f32 = row.iter().sum::<f32>() / a.cols as f32;
        let var: f32 = row.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / a.cols as f32;
        let std = (var + eps).sqrt();
        for c in 0..a.cols {
            *out.get_mut(r, c) = (a.get(r, c) - mean) / std;
        }
    }
    out
}

// ═══════════════════════════════════════════════════
// TensorStore: shared storage for DAG node outputs
// ═══════════════════════════════════════════════════

/// A thread-safe store mapping node IDs to their output tensors.
///
/// - Writers: only the worker executing a node writes its output (exclusive).
/// - Readers: workers executing downstream nodes read inputs (shared).
/// - The DAG guarantees no read-write conflicts in correct operation.
pub struct TensorStore {
    store: Vec<RwLock<Option<Tensor>>>,
}

impl TensorStore {
    /// Create a store with `n` slots, all initially empty.
    pub fn new(n: usize) -> Self {
        let store = (0..n).map(|_| RwLock::new(None)).collect();
        Self { store }
    }

    /// Read the output tensor of a given node.
    ///
    /// # Panics
    /// Panics if the tensor has not been written yet (indicates a
    /// dependency tracking bug in the scheduler).
    pub fn read(&self, node_id: usize) -> Tensor {
        let guard = self.store[node_id].read().unwrap();
        guard
            .as_ref()
            .unwrap_or_else(|| panic!("TensorStore: node {} output not yet available", node_id))
            .clone()
    }

    /// Write the output tensor for a given node.
    pub fn write(&self, node_id: usize, tensor: Tensor) {
        let mut guard = self.store[node_id].write().unwrap();
        *guard = Some(tensor);
    }

    /// Check if a node's output has been written.
    pub fn is_available(&self, node_id: usize) -> bool {
        self.store[node_id].read().unwrap().is_some()
    }
}
