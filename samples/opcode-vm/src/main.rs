use opcode_vm::execute;
use std::env;
use std::time::Instant;

fn parse_flag(args: &[String], name: &str, default: u32) -> u32 {
    args.windows(2)
        .find(|w| w[0] == name)
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let steps = parse_flag(&args, "--steps", 80_000_000);
    let seed = u64::from(parse_flag(&args, "--seed", 42));
    let bench = args.iter().any(|a| a == "--bench");
    let self_test = args.iter().any(|a| a == "--self-test");

    if self_test {
        let a = execute(8_192, 7);
        let b = execute(8_192, 7);
        assert_eq!(a, b);
        assert_ne!(a, execute(8_192, 8));
        println!("self-test ok checksum={a}");
        return;
    }

    let started = Instant::now();
    let checksum = execute(steps, seed);
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    println!("checksum={checksum} steps={steps} elapsed_ms={elapsed_ms:.6}");
    if bench {
        println!("QUENCH_BENCH {{\"elapsed_ms\":{elapsed_ms:.6},\"checksum\":{checksum}}}");
    }
}
