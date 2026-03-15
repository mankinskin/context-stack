#![allow(non_snake_case, unused)]

use std::{
    collections::VecDeque,
    ops::RangeInclusive,
};

use criterion::{
    criterion_group,
    criterion_main,
    Criterion,
};
use derive_more::Deref;
use derive_new::new;

use context_trace::*;

type BuildKey = RangeInclusive<usize>;

pub fn test_grammar() {
    let N: usize = 100; // total length
    let k: usize = 20; // alphabet size
                       //let mut graph = HypergraphRef::<BaseGraphKind>::default();
    println!("N = {}\nk = {}", N, k);
    let num_v = count_max_nodes(N, k);
    println!("num_v = {}", num_v);
    println!("1/2N^2 + 1/2 = {}", N.pow(2) as f32 / 2.0 + 1.5);
    println!(
        "diff = {}",
        (num_v as f32 - (N.pow(2) as f32 / 2.0 + 1.5)).abs()
    );

    println!("Generating saturated grammar (N = {}) ...", N);
    let g = worst_case_grammar(N, k);
    println!("num_v = {}", g.vertex_count());
    println!("num_e = {}", 4 * g.vertex_count());
    let num_bytes = g.vertex_count()
        * (std::mem::size_of::<VertexData>()
            + std::mem::size_of::<VertexIndex>())
        + 4 * g.vertex_count()
            * (std::mem::size_of::<Token>() + std::mem::size_of::<Parent>());
    println!("total MB = {}", num_bytes as u32 / 10_u32.pow(6),);
    println!("mul = {}", num_bytes / N,);
}

#[derive(new, Deref)]
struct BuilderNode {
    index: Token,
    #[deref]
    range: BuildKey,
}

impl BuilderNode {
    pub(crate) fn prefix_rule(&self) -> [BuildKey; 2] {
        [*self.start()..=self.end() - 1, *self.end()..=*self.end()]
    }
    pub(crate) fn postfix_rule(&self) -> [BuildKey; 2] {
        [
            *self.start()..=*self.start(),
            *self.start() + 1..=*self.end(),
        ]
    }
}

struct GraphBuilder {
    range_map: HashMap<BuildKey, VertexIndex>,
    queue: VecDeque<BuilderNode>,
    graph: Hypergraph,
    N: usize,
}

impl GraphBuilder {
    pub(crate) fn new(N: usize) -> Self {
        Self {
            N,
            range_map: Default::default(),
            graph: Default::default(),
            queue: Default::default(),
        }
    }
    pub(crate) fn queue_node(
        &mut self,
        node: BuilderNode,
    ) {
        self.graph.insert_vertex_data(VertexData::new(Token::new(
            node.index.vertex_index(),
            TokenWidth(node.range.clone().count()),
        )));
        self.queue.push_back(node);
    }

    pub(crate) fn add_rules(
        &mut self,
        node: BuilderNode,
    ) {
        for rule in match node.index.width().0 {
            1 => vec![],
            2 => vec![node.prefix_rule()],
            _ => vec![node.prefix_rule(), node.postfix_rule()],
        } {
            let pid = PatternId::default();
            let pattern: Pattern = rule
                .iter()
                .enumerate()
                .map(|(sub_index, key)| {
                    let loc = ChildLocation::new(node.index, pid, sub_index);
                    if let Some(&v) = self.range_map.get(key) {
                        self.graph
                            .with_vertex_mut(v, |node| node.add_parent(loc));
                        Token::new(v, TokenWidth(key.clone().count()))
                    } else {
                        let vid = self.graph.next_vertex_index();
                        self.range_map.insert(key.clone(), vid);
                        let c =
                            Token::new(vid, TokenWidth(key.clone().count()));
                        self.queue_node(BuilderNode::new(c, key.clone()));
                        c
                    }
                })
                .collect();
            self.graph.with_vertex_mut(node.index, |v| {
                v.add_pattern_no_update(pid, pattern)
            });
        }
    }
    pub(crate) fn fill_grammar(&mut self) {
        let vid = self.graph.next_vertex_index();
        self.queue_node(BuilderNode::new(
            Token::new(vid, TokenWidth(self.N)),
            0..=self.N - 1,
        ));
        while let Some(node) = self.queue.pop_front() {
            self.add_rules(node);
        }
    }
    pub(crate) fn saturated_grammar(
        mut self,
        k: usize,
    ) -> Hypergraph {
        self.fill_grammar();
        let mut ctx = RewireCtx::new(k, self);
        ctx.rewire_grammar();
        ctx.builder.graph
    }
}

struct RewireCtx {
    builder: GraphBuilder,
    prefix_counts: HashMap<VertexIndex, usize>,
    k: usize,
}

impl RewireCtx {
    pub(crate) fn new(
        k: usize,
        builder: GraphBuilder,
    ) -> Self {
        Self {
            builder,
            prefix_counts: Default::default(),
            k,
        }
    }
    pub(crate) fn rewire_grammar(&mut self) {
        let first = *self
            .builder
            .range_map
            .get(&(0..=0))
            .expect("Must include range 0..=0 in range_map");
    }
}

fn worst_case_grammar(
    N: usize,
    k: usize,
) -> Hypergraph {
    GraphBuilder::new(N).saturated_grammar(k)
}

fn count_max_nodes(
    N: usize,
    k: usize,
) -> usize {
    let root = ((N + 1) as f32).log(k as f32);
    println!("n0: {}", root);
    let root: f32 = nrfind::find_root(
        &|x| (k as f32).powf(x) - N as f32 + x - 1.0,
        &|x| (k as f32).powf(x) * (k as f32).ln() + 1.0,
        root,
        0.0001,
        50,
    )
    .unwrap();
    let root: u32 = root.floor() as u32;
    println!("root: {}", root);

    (2..=root)
        .map(|n| k.pow(n))
        .chain(((root as usize + 1)..=N).map(|n| N + 1 - n))
        .sum()
}

fn bench_grammar(c: &mut Criterion) {
    c.bench_function("grammar_worst_case", |b| b.iter(test_grammar));
}

criterion_group!(benches, bench_grammar);
criterion_main!(benches);
