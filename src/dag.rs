//! Part B — the DAG definition types, the sample DAG builders (provided),
//! and the runtime readiness tracker (yours to implement).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ═══════════════════════════════════════════════════
// DAG Definition Types
// ═══════════════════════════════════════════════════

/// The kind of tensor operation a DAG node performs.
#[derive(Debug, Clone)]
pub enum OpKind {
    /// Leaf node: stores a pre-initialized tensor (no computation).
    Input,
    /// Matrix multiply: C = deps[0] @ deps[1].
    MatMul,
    /// Element-wise addition: C = deps[0] + deps[1].
    ElemAdd,
    /// Element-wise ReLU: C = max(0, deps[0]).
    ReLU,
    /// Row-wise softmax of deps[0].
    Softmax,
    /// Transpose of deps[0] (used for K^T in attention).
    Transpose,
    /// Concatenate deps[0] and deps[1] along the given axis.
    Concat { axis: usize },
    /// Layer normalization of deps[0].
    LayerNorm,
}

/// A single node in the computation DAG.
#[derive(Debug, Clone)]
pub struct OpNode {
    /// Unique node ID (index into ComputeDAG::nodes).
    pub id: usize,
    /// Human-readable name (e.g., "Q0 = X @ Wq0").
    pub name: String,
    /// The operation this node performs.
    pub kind: OpKind,
    /// IDs of nodes whose outputs are inputs to this node.
    pub deps: Vec<usize>,
    /// Shape of this node's output tensor (rows, cols).
    pub output_shape: (usize, usize),
}

/// A complete computation graph.
#[derive(Debug, Clone)]
pub struct ComputeDAG {
    pub nodes: Vec<OpNode>,
    pub name: String,
}

impl ComputeDAG {
    /// Number of nodes in the graph.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// True if the graph has no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Length of the longest path through the DAG (the *critical path*),
    /// measured in nodes.
    ///
    /// This is the hard lower bound on execution time: no scheduler, with
    /// any number of workers, can finish faster than the critical path.
    /// Use this in your writeup when explaining sub-linear speedup.
    pub fn critical_path_len(&self) -> usize {
        let mut depth = vec![0usize; self.nodes.len()];
        // Node IDs are assigned in topological order by all builders here,
        // so a single forward pass is sufficient.
        for node in &self.nodes {
            let d = node
                .deps
                .iter()
                .map(|&d| depth[d])
                .max()
                .unwrap_or(0);
            depth[node.id] = d + 1;
        }
        depth.into_iter().max().unwrap_or(0)
    }
}

// ═══════════════════════════════════════════════════
// DAG Builders (provided)
// ═══════════════════════════════════════════════════

/// Build a simple diamond DAG:
///
///     [0] A
///    /     \
/// [1] B   [2] C
///    \     /
///     [3] D
///
/// A is Input. B = ReLU(A). C = ReLU(A). D = ElemAdd(B, C).
pub fn build_diamond_dag(rows: usize, cols: usize) -> ComputeDAG {
    ComputeDAG {
        name: format!("Diamond ({}x{})", rows, cols),
        nodes: vec![
            OpNode { id: 0, name: "A (input)".into(), kind: OpKind::Input, deps: vec![], output_shape: (rows, cols) },
            OpNode { id: 1, name: "B = ReLU(A)".into(), kind: OpKind::ReLU, deps: vec![0], output_shape: (rows, cols) },
            OpNode { id: 2, name: "C = ReLU(A)".into(), kind: OpKind::ReLU, deps: vec![0], output_shape: (rows, cols) },
            OpNode { id: 3, name: "D = B + C".into(), kind: OpKind::ElemAdd, deps: vec![1, 2], output_shape: (rows, cols) },
        ],
    }
}

/// Build a wide-fan DAG:
///
///        [0] Root
///       / | | | \
///     [1][2]...[N]  (all ReLU of root)
///       \ | | | /
///      [N+1] Sink   (sum via sequential adds)
///
/// Tests maximum parallelism at the fan-out level.
pub fn build_wide_fan_dag(fan_width: usize, rows: usize, cols: usize) -> ComputeDAG {
    let mut nodes = Vec::new();

    // Node 0: Input root
    nodes.push(OpNode {
        id: 0,
        name: "Root (input)".into(),
        kind: OpKind::Input,
        deps: vec![],
        output_shape: (rows, cols),
    });

    // Nodes 1..=fan_width: independent ReLU operations
    for i in 1..=fan_width {
        nodes.push(OpNode {
            id: i,
            name: format!("Fan_{} = ReLU(Root)", i),
            kind: OpKind::ReLU,
            deps: vec![0],
            output_shape: (rows, cols),
        });
    }

    // Sink: sequentially add all fan outputs.
    // We chain adds: tmp1 = fan[1]+fan[2], tmp2 = tmp1+fan[3], ...
    if fan_width >= 2 {
        let first_tmp_id = fan_width + 1;
        nodes.push(OpNode {
            id: first_tmp_id,
            name: "Sink_add_1".into(),
            kind: OpKind::ElemAdd,
            deps: vec![1, 2],
            output_shape: (rows, cols),
        });
        for i in 3..=fan_width {
            let node_id = first_tmp_id + i - 2;
            nodes.push(OpNode {
                id: node_id,
                name: format!("Sink_add_{}", i - 1),
                kind: OpKind::ElemAdd,
                deps: vec![node_id - 1, i],
                output_shape: (rows, cols),
            });
        }
    }

    ComputeDAG {
        name: format!("Wide-Fan (width={}, {}x{})", fan_width, rows, cols),
        nodes,
    }
}

/// Build a purely sequential chain DAG:
///
///     [0] Input -> [1] -> [2] -> ... -> [depth]
///
/// Every node depends on the previous one, so there is **no** parallelism
/// available at all. Its critical path equals its node count.
///
/// This is the control case for your benchmark: whatever slowdown you
/// measure here versus 1 worker is pure scheduler overhead — steal probes,
/// backoff sleeps, and synchronisation — with zero parallel work to hide it.
/// Use it to separate "Amdahl's Law" from "my scheduler is expensive".
pub fn build_chain_dag(depth: usize, rows: usize, cols: usize) -> ComputeDAG {
    assert!(depth >= 1, "chain depth must be at least 1");
    let mut nodes = Vec::with_capacity(depth + 1);

    nodes.push(OpNode {
        id: 0,
        name: "Chain input".into(),
        kind: OpKind::Input,
        deps: vec![],
        output_shape: (rows, cols),
    });

    for i in 1..=depth {
        // Alternate op kinds so the chain isn't trivially optimisable.
        let kind = if i % 2 == 0 {
            OpKind::LayerNorm
        } else {
            OpKind::ReLU
        };
        nodes.push(OpNode {
            id: i,
            name: format!("Chain_{}", i),
            kind,
            deps: vec![i - 1],
            output_shape: (rows, cols),
        });
    }

    ComputeDAG {
        name: format!("Chain (depth={}, {}x{})", depth, rows, cols),
        nodes,
    }
}

/// Build a multi-head transformer self-attention layer DAG.
///
/// `num_heads` controls how much head-level parallelism the graph exposes.
/// The graph always contains **11 × (num_heads + 1)** nodes:
///
/// | heads | nodes |
/// |-------|-------|
/// | 2     | 33    |
/// | 4     | 55    |
/// | 8     | 99    |
///
/// Structure (see the assignment spec for the full diagram):
///   - one input X and `3 × num_heads` weight matrices
///   - `3 × num_heads` independent QKV projections  (wide parallelism)
///   - per head: K transpose, Q@K^T, softmax, A@V   (head-level parallelism)
///   - a concat chain joining the heads              (fan-in)
///   - output projection, residual, LayerNorm, FFN   (sequential tail)
///
/// # Panics
/// Panics if `embed_dim` is not divisible by `num_heads`.
pub fn build_attention_dag(seq_len: usize, embed_dim: usize, num_heads: usize) -> ComputeDAG {
    assert!(num_heads >= 1, "num_heads must be at least 1");
    assert!(
        embed_dim % num_heads == 0,
        "embed_dim ({}) must be divisible by num_heads ({})",
        embed_dim,
        num_heads
    );

    let head_dim = embed_dim / num_heads;
    let mut nodes = Vec::new();
    let mut id = 0;

    macro_rules! node {
        ($name:expr, $kind:expr, $deps:expr, $shape:expr) => {{
            let nid = id;
            nodes.push(OpNode {
                id: nid,
                name: $name.to_string(),
                kind: $kind,
                deps: $deps,
                output_shape: $shape,
            });
            id += 1;
            nid
        }};
    }

    // Input activation.
    let x = node!("X (input)", OpKind::Input, vec![], (seq_len, embed_dim));

    // Per-head weight matrices (all independent inputs).
    let mut wq = Vec::with_capacity(num_heads);
    let mut wk = Vec::with_capacity(num_heads);
    let mut wv = Vec::with_capacity(num_heads);
    for h in 0..num_heads {
        wq.push(node!(format!("Wq{}", h), OpKind::Input, vec![], (embed_dim, head_dim)));
        wk.push(node!(format!("Wk{}", h), OpKind::Input, vec![], (embed_dim, head_dim)));
        wv.push(node!(format!("Wv{}", h), OpKind::Input, vec![], (embed_dim, head_dim)));
    }

    // QKV projections: 3 × num_heads independent matmuls.
    let mut q = Vec::with_capacity(num_heads);
    let mut k = Vec::with_capacity(num_heads);
    let mut v = Vec::with_capacity(num_heads);
    for h in 0..num_heads {
        q.push(node!(format!("Q{} = X @ Wq{}", h, h), OpKind::MatMul, vec![x, wq[h]], (seq_len, head_dim)));
        k.push(node!(format!("K{} = X @ Wk{}", h, h), OpKind::MatMul, vec![x, wk[h]], (seq_len, head_dim)));
        v.push(node!(format!("V{} = X @ Wv{}", h, h), OpKind::MatMul, vec![x, wv[h]], (seq_len, head_dim)));
    }

    // Per-head attention: K^T, scores, softmax, weighted values.
    let mut o = Vec::with_capacity(num_heads);
    for h in 0..num_heads {
        let kt = node!(format!("K{}T = K{}.T", h, h), OpKind::Transpose, vec![k[h]], (head_dim, seq_len));
        let s = node!(format!("S{} = Q{} @ K{}T", h, h, h), OpKind::MatMul, vec![q[h], kt], (seq_len, seq_len));
        let a = node!(format!("A{} = softmax(S{})", h, h), OpKind::Softmax, vec![s], (seq_len, seq_len));
        o.push(node!(format!("O{} = A{} @ V{}", h, h, h), OpKind::MatMul, vec![a, v[h]], (seq_len, head_dim)));
    }

    // Concat the heads back together (a left-leaning chain of concats).
    let mut cat = o[0];
    let mut cat_cols = head_dim;
    for h in 1..num_heads {
        cat_cols += head_dim;
        cat = node!(
            format!("Concat(..O{})", h),
            OpKind::Concat { axis: 1 },
            vec![cat, o[h]],
            (seq_len, cat_cols)
        );
    }

    // Output projection.
    let wo = node!("Wo", OpKind::Input, vec![], (embed_dim, embed_dim));
    let proj = node!("Proj = Concat @ Wo", OpKind::MatMul, vec![cat, wo], (seq_len, embed_dim));

    // Residual add + LayerNorm.
    let res1 = node!("ResAdd = X + Proj", OpKind::ElemAdd, vec![x, proj], (seq_len, embed_dim));
    let ln1 = node!("LN1 = LayerNorm(ResAdd)", OpKind::LayerNorm, vec![res1], (seq_len, embed_dim));

    // Feed-forward network (the sequential tail).
    let w1 = node!("W1 (FF)", OpKind::Input, vec![], (embed_dim, embed_dim * 4));
    let ff1 = node!("FF1 = LN1 @ W1", OpKind::MatMul, vec![ln1, w1], (seq_len, embed_dim * 4));
    let act = node!("ReLU(FF1)", OpKind::ReLU, vec![ff1], (seq_len, embed_dim * 4));

    let w2 = node!("W2 (FF)", OpKind::Input, vec![], (embed_dim * 4, embed_dim));
    let ff2 = node!("FF2 = ReLU @ W2", OpKind::MatMul, vec![act, w2], (seq_len, embed_dim));

    // Second residual + LayerNorm.
    let res2 = node!("ResAdd2 = LN1 + FF2", OpKind::ElemAdd, vec![ln1, ff2], (seq_len, embed_dim));
    let _out = node!("Output = LayerNorm(ResAdd2)", OpKind::LayerNorm, vec![res2], (seq_len, embed_dim));
    let _ = id; // the macro's final increment is intentionally unused

    ComputeDAG {
        name: format!(
            "Attention Layer ({}-head, {}x{})",
            num_heads, seq_len, embed_dim
        ),
        nodes,
    }
}

// ═══════════════════════════════════════════════════
// ReadyTracker — runtime dependency tracking
// ═══════════════════════════════════════════════════

/// Tracks which DAG nodes are ready to execute at runtime.
///
/// Thread-safe: multiple workers call `retire` concurrently.
pub struct ReadyTracker {
    /// Number of unresolved dependencies for each node.
    /// A node is ready when its in-degree reaches 0.
    in_degree: Vec<AtomicUsize>,
    /// Successor list: successors[i] = nodes that depend on node i.
    successors: Vec<Vec<usize>>,
    /// Whether each node has been completed.
    completed: Vec<AtomicBool>,
    /// Total number of nodes.
    num_nodes: usize,
    /// Number of nodes completed so far.
    completed_count: AtomicUsize,
}

impl ReadyTracker {
    /// Build a tracker from a DAG definition.
    ///
    /// Pre-computes in-degrees and the successor adjacency list.
    pub fn new(dag: &ComputeDAG) -> Self {
        let n = dag.nodes.len();
        let _ = n;

        // TODO(Done): Implement ReadyTracker::new
        //
        // Steps:
        // 1. Build `in_degree`: for each node, count its dependencies.
        //    Wrap each count in AtomicUsize::new(count).
        //
        // 2. Build `successors`: start with n empty Vecs. Then for each
        //    node N and each dep D in N.deps, push N.id into successors[D].
        //
        // 3. Build `completed`: n AtomicBool::new(false).
        //
        // 4. completed_count = AtomicUsize::new(0).
        //
        // 5. Return Self { .. }.

        let in_degree = dag.nodes
            .iter()
            .map(|node| AtomicUsize::new(node.deps.len()))
            .collect();

        let mut successors = vec![Vec::new(); n];
        for node in &dag.nodes {
            for dep in &node.deps {
                successors[*dep].push(node.id);
            }
        }

        let completed = (0..n).map(|_| AtomicBool::new(false)).collect();

        Self {
            in_degree,
            successors,
            completed,
            num_nodes: n,
            completed_count: AtomicUsize::new(0),
        }
    }

    /// Return every node ID whose in-degree is currently 0.
    ///
    /// Called once at startup to seed the workers' queues. These are the
    /// graph's Input/weight nodes — its initial *frontier*.
    pub fn initial_frontier(&self) -> Vec<usize> {
        // TODO(Done): Implement initial_frontier
        //
        // Iterate over in_degree; include each node whose
        // in_degree.load(Ordering::Acquire) == 0.

        self.in_degree
            .iter()
            .enumerate()
            .filter_map(|(id, deg)| {
                if deg.load(Ordering::Acquire) == 0 {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Retire a completed node: decrement the in-degree of each successor
    /// and append any successors that **just** became ready to `newly_ready`.
    ///
    /// # Why an out-parameter?
    ///
    /// This is on the hot path — it runs once per task. Returning a fresh
    /// `Vec` would allocate on every single completion. Instead the caller
    /// owns one scratch buffer and reuses it across the whole worker loop.
    /// Your worker should `clear()` the buffer, call `retire`, then drain it.
    ///
    /// `retire` **appends**; it must not clear the buffer itself.
    ///
    /// # Thread Safety
    /// Multiple workers may call this concurrently for different nodes.
    /// `AtomicUsize::fetch_sub` gives correctness without a lock.
    pub fn retire(&self, node_id: usize, newly_ready: &mut Vec<usize>) {
        // TODO(Done): Implement retire
        //
        // Steps:
        // 1. Mark this node completed:
        //      self.completed[node_id].store(true, Ordering::Release);
        //      self.completed_count.fetch_add(1, Ordering::AcqRel);
        //
        // 2. For each successor S in self.successors[node_id]:
        //      let prev = self.in_degree[S].fetch_sub(1, Ordering::AcqRel);
        //      if prev == 1 { newly_ready.push(S); }
        //
        //    KEY INSIGHT: fetch_sub returns the PREVIOUS value. A previous
        //    value of 1 means the counter is now 0 — this successor just
        //    became ready, and exactly one worker observes that transition.
        //    That is what makes it safe to push without a lock.
        //
        // 3. Append to `newly_ready` — do NOT clear it.

        self.completed[node_id].store(true, Ordering::Release);
        self.completed_count.fetch_add(1, Ordering::AcqRel);

        for &succ in &self.successors[node_id] {
            let prev = self.in_degree[succ].fetch_sub(1, Ordering::AcqRel);
            if prev == 1 {
                newly_ready.push(succ);
            }
        }
    }

    /// True once every node in the DAG has been retired.
    pub fn is_drained(&self) -> bool {
        self.completed_count.load(Ordering::Acquire) == self.num_nodes
    }

    /// True if this specific node has been retired.
    pub fn is_complete(&self, node_id: usize) -> bool {
        self.completed[node_id].load(Ordering::Acquire)
    }

    /// Total number of nodes.
    pub fn num_nodes(&self) -> usize {
        self.num_nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diamond_frontier() {
        let dag = build_diamond_dag(4, 4);
        let tracker = ReadyTracker::new(&dag);
        let ready = tracker.initial_frontier();
        assert_eq!(ready, vec![0], "Only the input node should start ready");
    }

    #[test]
    fn test_diamond_execution_order() {
        let dag = build_diamond_dag(4, 4);
        let tracker = ReadyTracker::new(&dag);
        let mut buf = Vec::new();

        // Complete node 0 (A).
        buf.clear();
        tracker.retire(0, &mut buf);
        buf.sort();
        assert_eq!(buf, vec![1, 2], "B and C become ready after A");

        // Complete node 1 (B) — D still needs C.
        buf.clear();
        tracker.retire(1, &mut buf);
        assert!(buf.is_empty(), "D needs both B and C");

        // Complete node 2 (C) — now D is ready.
        buf.clear();
        tracker.retire(2, &mut buf);
        assert_eq!(buf, vec![3], "D becomes ready after both B and C");

        buf.clear();
        tracker.retire(3, &mut buf);
        assert!(tracker.is_drained());
    }

    #[test]
    fn test_retire_appends_without_clearing() {
        // `retire` must append, so a caller can accumulate across calls.
        let dag = build_diamond_dag(4, 4);
        let tracker = ReadyTracker::new(&dag);

        let mut buf = vec![999];
        tracker.retire(0, &mut buf);
        assert_eq!(buf[0], 999, "retire must not clear the caller's buffer");
        assert_eq!(buf.len(), 3, "sentinel + two newly-ready nodes");
    }

    #[test]
    fn test_wide_fan_frontier() {
        let dag = build_wide_fan_dag(8, 4, 4);
        let tracker = ReadyTracker::new(&dag);
        assert_eq!(tracker.initial_frontier(), vec![0]);
    }

    #[test]
    fn test_wide_fan_parallelism() {
        let dag = build_wide_fan_dag(4, 4, 4);
        let tracker = ReadyTracker::new(&dag);

        let mut buf = Vec::new();
        tracker.retire(0, &mut buf);
        buf.sort();
        assert_eq!(buf, vec![1, 2, 3, 4], "all fan nodes become ready at once");
    }

    #[test]
    fn test_chain_has_no_parallelism() {
        let dag = build_chain_dag(6, 4, 4);
        assert_eq!(dag.len(), 7, "depth 6 => 7 nodes including the input");
        assert_eq!(
            dag.critical_path_len(),
            7,
            "a chain's critical path is its whole length"
        );

        let tracker = ReadyTracker::new(&dag);
        assert_eq!(tracker.initial_frontier(), vec![0]);

        let mut buf = Vec::new();
        tracker.retire(0, &mut buf);
        assert_eq!(buf, vec![1], "a chain unlocks exactly one node at a time");
    }

    #[test]
    fn test_attention_dag_node_count() {
        // The graph always has 11 * (heads + 1) nodes.
        for heads in [1usize, 2, 4, 8] {
            let dag = build_attention_dag(8, 16, heads);
            assert_eq!(
                dag.len(),
                11 * (heads + 1),
                "unexpected node count for {} heads",
                heads
            );
        }
    }

    #[test]
    fn test_attention_dag_frontier() {
        // Ready at start: X, plus Wq/Wk/Wv per head, plus Wo, W1, W2.
        let heads = 4;
        let dag = build_attention_dag(8, 16, heads);
        let tracker = ReadyTracker::new(&dag);
        let ready = tracker.initial_frontier();
        assert_eq!(
            ready.len(),
            1 + 3 * heads + 3,
            "should have 1 activation + 3 weights/head + Wo + W1 + W2"
        );
    }

    #[test]
    fn test_attention_critical_path_is_short() {
        // Lots of nodes, but a shallow critical path — that gap is exactly
        // the parallelism a work-stealing scheduler is meant to exploit.
        let dag = build_attention_dag(8, 16, 4);
        assert!(
            dag.critical_path_len() < dag.len() / 2,
            "attention DAG should expose real parallelism"
        );
    }
}
