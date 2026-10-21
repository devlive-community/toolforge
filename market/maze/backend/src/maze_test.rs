use super::*;
use crate::tests::{Context, spec};

#[test]
fn many_seeds_sizes_and_ports_produce_connected_acyclic_graphs() {
    let ctx = Context::default();
    for seed in 0..150 {
        let mut spec = spec(&seed.to_string());
        spec.rows = 2 + seed % 17;
        spec.cols = 2 + (seed * 7) % 19;
        spec.entrance = Port {
            side: SIDES[seed % 4],
            index: 1,
        };
        spec.exit = Port {
            side: SIDES[(seed + 2) % 4],
            index: 2,
        };
        let maze = generate(spec, &ctx).unwrap();
        let n = maze.passages.len();
        let mut roots = (0..n).collect::<Vec<_>>();
        fn root(roots: &[usize], mut at: usize) -> usize {
            while roots[at] != at {
                at = roots[at];
            }
            at
        }
        let mut edges = 0;
        for at in 0..n {
            for side in [Side::Right, Side::Bottom] {
                if let Some(next) = maze.adjacent(at, side)
                    && maze.passages[at] & side.bit() != 0
                {
                    assert_ne!(root(&roots, at), root(&roots, next), "cycle at seed {seed}");
                    let a = root(&roots, at);
                    let b = root(&roots, next);
                    roots[a] = b;
                    edges += 1;
                }
            }
        }
        assert_eq!(edges, n - 1);
        assert!((0..n).all(|id| root(&roots, id) == root(&roots, 0)));
        let route = maze.route(maze.start, &ctx).unwrap();
        assert_eq!(route[0], maze.start);
        assert_eq!(*route.last().unwrap(), maze.exit);
        assert!(route.windows(2).all(|p| maze.legal(p[0], p[1])));
        let outer: usize = (0..n)
            .map(|at| {
                SIDES
                    .iter()
                    .filter(|&&side| {
                        maze.adjacent(at, side).is_none() && maze.passages[at] & side.bit() != 0
                    })
                    .count()
            })
            .sum();
        assert_eq!(outer, 2);
    }
}
#[test]
fn reproducible_codes_preserve_unicode_seeds_and_all_generation_parameters() {
    let mut spec = spec("午后🧭");
    spec.entrance = Port {
        side: Side::Bottom,
        index: 3,
    };
    let restored = Spec::from_code(&spec.code()).unwrap();
    assert_eq!(restored.id(), spec.id());
    assert_eq!(restored.seed, spec.seed);
    let a = generate(spec, &Context::default()).unwrap();
    let b = generate(restored, &Context::default()).unwrap();
    assert_eq!(a.passages, b.passages);
    let mut c = b.spec.clone();
    c.seed.push('x');
    assert_ne!(c.id(), b.spec.id());
}
#[test]
fn maximum_grid_and_opposite_ports_work_without_recursion() {
    let mut spec = spec("max");
    spec.rows = 40;
    spec.cols = 40;
    spec.exit.index = 40;
    let maze = generate(spec, &Context::default()).unwrap();
    assert_eq!(maze.passages.len(), 1600);
    maze.verify(&Context::default()).unwrap();
}
#[test]
fn invalid_sizes_ports_versions_seeds_and_codes_are_rejected() {
    for size in [0, 1, 41, usize::MAX] {
        let mut s = spec("x");
        s.rows = size;
        assert!(generate(s, &Context::default()).is_err());
    }
    let mut s = spec("x");
    s.exit = Port {
        side: Side::Top,
        index: 1,
    };
    assert_eq!(s.validate().err().unwrap().code, "maze.same_port");
    let mut s = spec("x");
    s.entrance.index = 0;
    assert!(s.validate().is_err());
    let mut s = spec("x");
    s.algorithm = "future".into();
    assert!(s.validate().is_err());
    for seed in ["".to_owned(), "x".repeat(81), "a\nb".into()] {
        let s = spec(&seed);
        assert!(s.validate().is_err());
    }
    for code in [
        "M2.2.2.8.1.2.2.61",
        "M1.2.2.8.1.2.2.ff",
        "M1.2.2.8.1.2.2.abc",
        "M1.bad",
    ] {
        assert!(Spec::from_code(code).is_err());
    }
}
#[test]
fn cancellation_stops_generation_and_solution_work() {
    let ctx = Context {
        cancel_on: Some("stop"),
        ..Context::default()
    };
    ctx.stage("stop");
    assert!(generate(spec("x"), &ctx).is_err());
    let maze = generate(spec("x"), &Context::default()).unwrap();
    assert!(maze.route(maze.start, &ctx).is_err());
}

#[test]
fn v1_generation_has_a_fixed_reproduction_fixture() {
    let maze = generate(spec("review"), &Context::default()).unwrap();
    assert_eq!(maze.spec.id(), "M1-0DFBEBC4CABE22DA4F6350E4");
    assert_eq!(
        format!("{:x}", Sha256::digest(&maze.passages)),
        "7ae068e77161e166cbead780104f1834915a85edc64e36ca00a5f6017cd9075d"
    );
}
