use std::{convert::Infallible, env, process::ExitCode};

use key_right_core::{
    ColorTemperature, Command, Controller, Level, LightOutput, LightState, Preset,
};

const USAGE: &str = "Usage: key-right simulate [on|off|level=N|mired=N|preset-1|preset-2]...\n\
Runs one in-memory session, starting off at level57/mired303.\n\
Level is 1..254, mapped to nominal stock brightness 1..10%; mired is 143..344.\n\
This simulator performs no device or network I/O.\n\
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
            if let Some(number) = value.strip_prefix("level=") {
                let level = number
                    .parse::<u8>()
                    .ok()
                    .and_then(Level::new)
                    .ok_or_else(|| "level must be an integer from 1 to 254".to_owned())?;
                Ok(Command::SetLevel(level))
            } else if let Some(number) = value.strip_prefix("mired=") {
                let temperature = number
                    .parse::<u16>()
                    .ok()
                    .and_then(ColorTemperature::new)
                    .ok_or_else(|| "mired must be an integer from 143 to 344".to_owned())?;
                Ok(Command::SetTemperature(temperature))
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
    let (brightness_numerator, brightness_denominator) = applied.level.stock_brightness_ratio();
    let frame = key_right_core::pca9635::stock_frame(applied);
    println!(
        "mode=simulation intended_on={} intended_level={} intended_mired={} applied_on={} applied_level={} applied_mired={} selected_stock_brightness_percent={}/{} raw_warm={} raw_cool={} output_writes={}",
        intended.on,
        intended.level.get(),
        intended.temperature.get(),
        applied.on,
        applied.level.get(),
        applied.temperature.get(),
        brightness_numerator,
        brightness_denominator,
        frame[0],
        frame[4],
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
