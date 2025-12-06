use context_trace::{
    tests::{
        env::Env2,
        test_case::TestEnv,
    },
    *,
};
use pretty_assertions::assert_eq;
use std::collections::HashSet;

/// Test case for query [d, e, f, g, h] finding ancestor in graph with cdefghi patterns
pub struct CdefghiTraceCase {
    pub env: Env2,
    pub query_tokens: Vec<Token>,
    pub expected_root: Token,
    pub expected_end_bound: usize,
    pub expected_cache: TraceCache,
}

impl Default for CdefghiTraceCase {
    fn default() -> Self {
        let env = Env2::initialize();

        let Env2 {
            d,
            e,
            f,
            g,
            h,
            cd,
            hi,
            cdefg,
            efghi,
            cdefghi,
            c_d_id,
            h_i_id,
            cd_efg_id,
            efg_hi_id,
            cdefghi_ids,
            ..
        } = &env;

        // Dereference for use in macro
        let (d, e, f, g, h) = (*d, *e, *f, *g, *h);
        let (cd, hi, cdefg, _efghi, cdefghi) =
            (*cd, *hi, *cdefg, *efghi, *cdefghi);
        let (c_d_id, h_i_id, cd_efg_id, _efg_hi_id) =
            (*c_d_id, *h_i_id, *cd_efg_id, *efg_hi_id);

        let expected_cache = build_trace_cache!(
            d => (
                BU {},
                TD {}
            ),
            cd => (
                BU {
                    1 => d -> (c_d_id, 1)
                },
                TD {}
            ),
            hi => (
                BU {},
                TD {
                   4 => h -> (h_i_id, 0)
                }
            ),
            cdefg => (
                BU {
                    1 => cd -> (cd_efg_id, 0)
                },
                TD {}
            ),
            h => (
                BU {},
                TD {
                    4
                }
            ),
            cdefghi => (
                BU {
                    4 => cdefg -> (cdefghi_ids[0], 0)
                },
                TD {
                    4 => hi -> (cdefghi_ids[0], 1)
                }
            ),
        );

        Self {
            query_tokens: vec![d, e, f, g, h],
            expected_root: cdefghi,
            expected_end_bound: 5,
            expected_cache,
            env,
        }
    }
}
impl CdefghiTraceCase {
    pub fn verify_trace_cache(
        &self,
        actual_cache: &TraceCache,
    ) {
        // Verify number of entries
        if actual_cache.entries.len() != self.expected_cache.entries.len() {
            let actual_keys: HashSet<_> = actual_cache.entries.keys().collect();
            let expected_keys: HashSet<_> =
                self.expected_cache.entries.keys().collect();
            let extra_in_actual: Vec<_> =
                actual_keys.difference(&expected_keys).collect();
            let missing_from_actual: Vec<_> =
                expected_keys.difference(&actual_keys).collect();

            let mut msg = format!(
                "Trace cache entry count mismatch. Actual: {}, Expected: {}",
                actual_cache.entries.len(),
                self.expected_cache.entries.len()
            );

            if !extra_in_actual.is_empty() {
                msg.push_str(&format!(
                    "\nExtra entries in actual: {:?}",
                    extra_in_actual
                        .iter()
                        .map(|&&idx| {
                            actual_cache.entries.get(idx).map(|e| &e.index)
                        })
                        .collect::<Vec<_>>()
                ));
            }

            if !missing_from_actual.is_empty() {
                msg.push_str(&format!(
                    "\nMissing from actual: {:#?}",
                    missing_from_actual
                        .iter()
                        .map(|&&idx| { self.expected_cache.entries.get(idx) })
                        .collect::<Vec<_>>()
                ));
            }

            panic!("{}", msg);
        }

        // Check each entry
        for (idx, expected_entry) in self.expected_cache.entries.iter() {
            let actual_entry =
                actual_cache.entries.get(idx).unwrap_or_else(|| {
                    panic!("Missing entry for {:?}", expected_entry.index)
                });
            assert_eq!(
                actual_entry, expected_entry,
                "Trace entry mismatch for {:?}",
                expected_entry.index
            );
        }
    }
}

#[cfg(test)]
use crate::Find;

#[test]
fn test_cdefghi_trace_cache() {
    let test_case = CdefghiTraceCase::default();
    let _tracing = context_trace::init_test_tracing!(&test_case.env.graph);

    let res = test_case
        .env
        .graph
        .find_ancestor(test_case.query_tokens.clone())
        .unwrap();
    assert!(res.query_exhausted());

    assert_eq!(res.root_token(), test_case.expected_root);
    assert_eq!(
        res.checkpoint_position(),
        test_case.expected_end_bound.into()
    );

    test_case.verify_trace_cache(&res.cache);
}
