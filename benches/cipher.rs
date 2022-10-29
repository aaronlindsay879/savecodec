use criterion::{black_box, criterion_group, criterion_main, Criterion};

const CIPHER_KEY: &[u8] = b"therealmisalie";

fn for_loop(out: &mut [u8]) {
    for (index, byte) in out.iter_mut().enumerate() {
        *byte ^= CIPHER_KEY[index % CIPHER_KEY.len()];
    }
}

fn functional(out: &mut [u8]) {
    out.iter_mut()
        .zip(CIPHER_KEY.iter().cycle())
        .for_each(|(byte, key)| *byte ^= key);
}

fn bench(c: &mut Criterion) {
    let save = std::fs::read_to_string("save.txt").unwrap();
    let mut data = save.into_bytes();

    let mut group = c.benchmark_group("Cipher");
    group.bench_function("for loop", |b| b.iter(|| for_loop(&mut data)));
    group.bench_function("functional", |b| b.iter(|| functional(&mut data)));

    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
