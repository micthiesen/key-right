use std::{convert::Infallible, env, process::ExitCode};

use key_right_core::{Command, Controller, LightOutput, LightState, Preset};

mod profile;

const USAGE: &str = "Usage: key-right simulate [on|off|preset-1|preset-2|brightness=N]...\n\
Runs one in-memory session, starting off. N is 0..100 and is ignored.\n\
This simulator performs no device or network I/O.\n\
       key-right profile stock\n\
Print an uncommissioned stock 3300 K / 5000 K profile as 160 hex digits.\n\
       key-right profile inspect <HEX_OR_FILE>\n\
Validate and describe a profile without accessing hardware.\n\
Use scripts/device.py for the native USB console.";

#[derive(Default)]
struct SimulatedOutput {
    writes: usize,
}

impl LightOutput for SimulatedOutput {
    type Error = Infallible;

    fn apply(&mut self, _state: LightState) -> Result<(), Self::Error> {
        self.writes += 1;
        Ok(())
    }
}

fn parse_command(value: &str) -> Result<Command, String> {
    match value {
        "on" => Ok(Command::SetPower(true)),
        "off" => Ok(Command::SetPower(false)),
        "preset-1" => Ok(Command::SelectPreset(Preset::One)),
        "preset-2" => Ok(Command::SelectPreset(Preset::Two)),
        _ => {
            if let Some(number) = value.strip_prefix("brightness=") {
                let brightness = number
                    .parse::<u8>()
                    .ok()
                    .filter(|value| *value <= 100)
                    .ok_or_else(|| "brightness must be an integer from 0 to 100".to_owned())?;
                Ok(Command::SetBrightness(brightness))
            } else {
                Err(format!("unknown command: {value}"))
            }
        }
    }
}

fn report(controller: &Controller, output: &SimulatedOutput) {
    let intended = controller.intended();
    let applied = controller
        .applied()
        .expect("simulation acknowledges every output application");
    println!(
        "mode=simulation intended_on={} intended_preset={:?} applied_on={} applied_preset={:?} nominal_brightness_percent={} output_writes={}",
        intended.on,
        intended.preset,
        applied.on,
        applied.preset,
        applied.brightness_percent(),
        output.writes,
    );
}

fn run(args: &[String]) -> Result<(), String> {
    match args {
        [] => {
            println!("{USAGE}");
            Ok(())
        }
        [flag] if flag == "--help" || flag == "-h" => {
            println!("{USAGE}");
            Ok(())
        }
        [flag] if flag == "--version" => {
            println!("key-right {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        [mode, values @ ..] if mode == "simulate" => {
            let commands: Vec<Command> = values
                .iter()
                .map(|value| parse_command(value))
                .collect::<Result<_, _>>()?;
            let mut controller = Controller::new(None);
            let mut output = SimulatedOutput::default();
            controller.reconcile(&mut output).unwrap();
            report(&controller, &output);
            for command in commands {
                controller.command(command);
                controller.reconcile(&mut output).unwrap();
                report(&controller, &output);
            }
            Ok(())
        }
        [mode, values @ ..] if mode == "profile" => profile::run(values),
        _ => Err(USAGE.to_owned()),
    }
}

fn main() -> ExitCode {
    match run(&env::args().skip(1).collect::<Vec<_>>()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}
