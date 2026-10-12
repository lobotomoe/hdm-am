//! `hdm simulate`: a simulated device on a local port, steered from standard input.

use anyhow::{Context, Result};
use hdm_am::simulator::{Fault, SimulatedOperator, Simulator, SimulatorConfig};
use std::io::BufRead as _;
use std::net::TcpListener;

use crate::{Cli, SimulateArgs};

/// What a line typed at the simulator asks for.
fn parse_fault(line: &str) -> Option<Fault> {
    let mut words = line.split_whitespace();
    match (words.next()?, words.next()) {
        ("lose", None) => Some(Fault::LoseAnswer),
        ("refuse", Some(code)) => code.parse().ok().map(Fault::Refuse),
        _ => None,
    }
}

/// Serve the simulated device until the process is stopped.
pub fn run(cli: &Cli, args: &SimulateArgs) -> Result<()> {
    let mut config = SimulatorConfig::default();
    if let Some(password) = &cli.password {
        config.password.clone_from(password);
    }
    if cli.cashier.is_some() || cli.pin.is_some() {
        let default = config.operators.remove(0);
        config.operators = vec![SimulatedOperator {
            id: cli.cashier.unwrap_or(default.id),
            pin: cli.pin.clone().unwrap_or(default.pin),
            name: default.name,
        }];
    }
    let listener =
        TcpListener::bind(args.listen).with_context(|| format!("listening on {}", args.listen))?;
    let operator = &config.operators[0];
    println!(
        "simulated HDM on {}: registration number {}, operator {}, department {}",
        args.listen, config.crn, operator.id, config.departments[0].id
    );
    println!("type `refuse <code>` or `lose` and Enter to get the next operation wrong");

    let simulator = Simulator::new(config);
    let steered = simulator.clone();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            match parse_fault(&line) {
                Some(fault) => {
                    steered.inject(fault);
                    println!("next operation: {fault:?}");
                }
                None => println!("not understood: `refuse <code>` or `lose`"),
            }
        }
    });
    simulator.serve(&listener).context("accepting a connection")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_two_faults_and_nothing_else() {
        assert_eq!(parse_fault("refuse 145"), Some(Fault::Refuse(145)));
        assert_eq!(parse_fault("  lose "), Some(Fault::LoseAnswer));
        assert_eq!(parse_fault("refuse paper"), None);
        assert_eq!(parse_fault("lose 1"), None);
        assert_eq!(parse_fault(""), None);
    }
}
