use std::process::Command;

#[test]
fn simulation_keeps_one_session_and_ignores_brightness_writes() {
    let output = Command::new(env!("CARGO_BIN_EXE_key-right"))
        .args(["simulate", "on", "preset-2", "brightness=100", "off", "on"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines.len(), 6);
    assert!(lines[0].contains("applied_on=false"));
    assert!(lines[1].contains("nominal_brightness_percent=3"));
    assert_eq!(lines[2], lines[3]);
    assert!(lines[4].contains("nominal_brightness_percent=0"));
    assert!(lines[5].contains("applied_preset=Two"));
    assert!(lines[5].contains("nominal_brightness_percent=3"));
    assert!(lines[5].contains("output_writes=5"));
}

#[test]
fn invalid_commands_fail_before_starting_a_session() {
    for value in [
        "brightness=101",
        "brightness=-1",
        "brightness=nope",
        "flash",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_key-right"))
            .args(["simulate", "on", value])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn stock_profile_is_inspectable_but_not_commissioned() {
    let output = Command::new(env!("CARGO_BIN_EXE_key-right"))
        .args(["profile", "stock"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let hex = String::from_utf8(output.stdout).unwrap();
    assert_eq!(hex.trim().len(), 160);
    let inspect = Command::new(env!("CARGO_BIN_EXE_key-right"))
        .args(["profile", "inspect", hex.trim()])
        .output()
        .unwrap();
    assert!(inspect.status.success());
    let description = String::from_utf8(inspect.stdout).unwrap();
    assert!(description.contains("address=0x15 mode2=0x14"));
    assert!(description.contains("preset_mired=[303, 200] attestations=0 commissioned=false"));
    let mut damaged = hex.trim().to_owned();
    damaged.replace_range(30..31, "f");
    let inspect = Command::new(env!("CARGO_BIN_EXE_key-right"))
        .args(["profile", "inspect", &damaged])
        .output()
        .unwrap();
    assert_eq!(inspect.status.code(), Some(2));
    assert!(String::from_utf8(inspect.stderr)
        .unwrap()
        .contains("Checksum"));
}
