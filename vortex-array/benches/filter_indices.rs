use criterion::{black_box, criterion_group, criterion_main, Criterion};
use itertools::Itertools;
use rand::distributions::Uniform;
use rand::{thread_rng, Rng};
use vortex::IntoArray;
use vortex_dtype::field_paths::FieldPath;
use vortex_error::VortexError;
use vortex_expr::expressions::{lit, Conjunction, Disjunction};
use vortex_expr::field_paths::FieldPathOperations;

fn filter_indices(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_indices");

    let mut rng = thread_rng();
    let range = Uniform::new(0i64, 100_000_000);
    let arr = (0..10_000_000)
        .map(|_| rng.sample(range))
        .collect_vec()
        .into_array();

    let predicate = Disjunction {
        conjunctions: vec![Conjunction {
            predicates: vec![FieldPath::builder().build().lt(lit(50_000_000i64))],
        }],
    };

    group.bench_function("vortex", |b| {
        b.iter(|| {
            let indices =
                vortex::compute::filter_indices::filter_indices(&arr, &predicate).unwrap();
            black_box(indices);
            Ok::<(), VortexError>(())
        });
    });
}

criterion_group!(benches, filter_indices);
criterion_main!(benches);
