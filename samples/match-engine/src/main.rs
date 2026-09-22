use std::env;
use std::time::Instant;
use match_engine::{match_orders, synthetic_book};

fn parse_flag(args: &[String], name: &str, default: u32) -> u32 {
    args.windows(2)
        .find(|w| w[0] == name)
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let orders_n = parse_flag(&args, "--orders", 80_000);
    let seed = u64::from(parse_flag(&args, "--seed", 42));
    let bench = args.iter().any(|a| a == "--bench");
    let self_test = args.iter().any(|a| a == "--self-test");

    if self_test {
        let orders = synthetic_book(1_024, 9);
        let (fills, checksum) = match_orders(&orders);
        assert!(!fills.is_empty());
        println!("self-test ok fills={} checksum={}", fills.len(), checksum);
        return;
    }

    let started = Instant::now();
    let orders = synthetic_book(orders_n, seed);
    let (fills, checksum) = match_orders(&orders);
    let elapsed = started.elapsed();
    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
    println!(
        "matched={} checksum={} orders={} elapsed_ms={:.6}",
        fills.len(),
        checksum,
        orders_n,
        elapsed_ms
    );
    if bench {
        println!(
            "QUENCH_BENCH {{\"elapsed_ms\":{:.6},\"fills\":{},\"checksum\":{}}}",
            elapsed_ms,
            fills.len(),
            checksum
        );
    }
}
