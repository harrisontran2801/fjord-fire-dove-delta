mod config;
mod doctor;
mod exec;
mod inspect;
mod pipeline;
mod report;
mod security;
mod serve;
mod util;

use config::load_config;
use doctor::{doctor_json, doctor_text, run_doctor};
use inspect::{inspect_path, inspect_to_json};
use pipeline::{load_run, optimize};
use report::report_text;
use serde_json::json;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn usage() -> String {
    r#"quench-agent — native local optimization pipeline (Linux x86_64)

Commands:
  quench-agent doctor
  quench-agent inspect <file>
  quench-agent optimize --config quench.yaml
  quench-agent report <run-id>
  quench-agent serve [--bind 127.0.0.1:4783] [--socket /tmp/quench-agent.sock]

The agent never uploads binaries. It listens only on loopback / a unix socket.
"#
    .to_string()
}

fn json_flag(args: &[String]) -> bool {
    args.iter().any(|a| a == "--json")
}

fn arg_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            return args.get(i + 1).map(|s| s.as_str());
        }
        if let Some(rest) = args[i].strip_prefix(&format!("{name}=")) {
            return Some(rest);
        }
        i += 1;
    }
    None
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{}", usage());
        return ExitCode::SUCCESS;
    }
    match args[0].as_str() {
        "doctor" => {
            let report = run_doctor();
            if json_flag(&args) {
                println!("{}", doctor_json(&report));
            } else {
                print!("{}", doctor_text(&report));
            }
            if report.ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        "inspect" => {
            let file = args.get(1).cloned().unwrap_or_default();
            if file.is_empty() || file.starts_with('-') {
                eprintln!("usage: quench-agent inspect <file>");
                return ExitCode::from(2);
            }
            let outcome = inspect_path(&PathBuf::from(&file));
            let v = inspect_to_json(&outcome);
            println!(
                "{}",
                serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())
            );
            if v.get("ok").and_then(|x| x.as_bool()) == Some(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        "optimize" => {
            let config = arg_value(&args, "--config").unwrap_or("");
            if config.is_empty() {
                eprintln!("usage: quench-agent optimize --config quench.yaml");
                return ExitCode::from(2);
            }
            match load_config(&PathBuf::from(config)).and_then(|cfg| optimize(&cfg)) {
                Ok(run) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&run.report)
                            .unwrap_or_else(|_| run.report.to_string())
                    );
                    match run.status {
                        pipeline::RunStatus::Complete => ExitCode::SUCCESS,
                        pipeline::RunStatus::VerificationFailed => ExitCode::from(3),
                        _ => ExitCode::from(1),
                    }
                }
                Err(e) => {
                    eprintln!("{}", json!({"ok": false, "error": e}));
                    ExitCode::from(1)
                }
            }
        }
        "report" => {
            let id = args.get(1).cloned().unwrap_or_default();
            if id.is_empty() {
                eprintln!("usage: quench-agent report <run-id>");
                return ExitCode::from(2);
            }
            match load_run(&id) {
                Ok(run) => {
                    if json_flag(&args) {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&run.report)
                                .unwrap_or_else(|_| run.report.to_string())
                        );
                    } else {
                        print!("{}", report_text(&run));
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(1)
                }
            }
        }
        "serve" => {
            let bind = arg_value(&args, "--bind").unwrap_or(serve::DEFAULT_BIND);
            let socket = arg_value(&args, "--socket").unwrap_or(serve::DEFAULT_SOCKET);
            if let Err(e) = serve::serve(bind, socket) {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown command: {other}\n{}", usage());
            ExitCode::from(2)
        }
    }
}
