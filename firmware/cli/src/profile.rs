use std::{fmt::Write, fs};

use key_right_core::pca9635::{Profile, PROFILE_LEN};

fn decode_hex(value: &str) -> Result<Profile, String> {
    let value = value.trim();
    if value.len() != PROFILE_LEN * 2 || !value.is_ascii() {
        return Err("profile must contain exactly 160 hexadecimal digits".to_owned());
    }
    let mut bytes = [0; PROFILE_LEN];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16)
            .map_err(|_| "profile contains non-hexadecimal text".to_owned())?;
    }
    Profile::decode(&bytes).map_err(|error| format!("invalid profile: {error:?}"))
}

pub fn run(args: &[String]) -> Result<(), String> {
    match args {
        [command] if command == "stock" => {
            let mut hex = String::with_capacity(PROFILE_LEN * 2);
            for byte in Profile::stock_candidate().encode() {
                write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
            }
            println!("{hex}");
            Ok(())
        }
        [command, input] if command == "inspect" => {
            let text = if input.len() == PROFILE_LEN * 2 && input.is_ascii() {
                input.clone()
            } else {
                fs::read_to_string(input).map_err(|error| format!("{input}: {error}"))?
            };
            let profile = decode_hex(&text)?;
            println!(
                "address=0x{:02x} mode2=0x{:02x} nominal_brightness_percent=3",
                profile.address(),
                profile.mode2()
            );
            println!(
                "preset_mired={:?} attestations={} commissioned={}",
                profile.temperatures_mired(),
                profile.attestations(),
                profile.require_commissioned().is_ok()
            );
            for (name, frame) in ["off", "preset1", "preset2"]
                .into_iter()
                .zip(profile.frames())
            {
                println!(
                    "{name}: PWM={:?} GRPPWM={} GRPFREQ={} LEDOUT={:02x?}",
                    &frame.0[..16],
                    frame.0[16],
                    frame.0[17],
                    &frame.0[18..]
                );
            }
            println!("Physical wiring, output and safety are not established by this inspection.");
            Ok(())
        }
        _ => Err("Usage: key-right profile stock | profile inspect <HEX_OR_FILE>".to_owned()),
    }
}
