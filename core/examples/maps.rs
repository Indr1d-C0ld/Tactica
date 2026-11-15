//! Stampa alcune mappe generate: `cargo run -p tactica-core --example maps [N] [seed]`.
use tactica_core::map::{Map, MapParams};
use tactica_core::rng::Rng;

fn main() {
    let args: Vec<u64> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let n = args.first().copied().unwrap_or(4);
    let seed = args.get(1).copied().unwrap_or(1);
    let mut sizes = Vec::new();
    for i in 0..n {
        let mut rng = Rng::new(seed + i);
        let params = MapParams::random(&mut rng);
        let map = Map::generate(&params, &mut rng);
        let hs: Vec<i32> = map
            .positions()
            .map(|p| map.tile(p).unwrap().height)
            .collect();
        sizes.push(map.width * map.height);
        if i < 4 {
            println!(
                "{} {}x{} altezze {}..{}",
                map.biome.name(),
                map.width,
                map.height,
                hs.iter().min().unwrap(),
                hs.iter().max().unwrap()
            );
            println!("{}", map.render(&|_| None));
        }
    }
    println!(
        "superficie media: {} caselle",
        sizes.iter().sum::<i32>() / sizes.len() as i32
    );
}
