use std::process::Command;

#[test]
fn simulation_keeps_one_session_and_retains_level_and_temperature_while_off() {
    let output = Command::new(env!("CARGO_BIN_EXE_key-right"))
        .args(["simulate", "on", "mired=200", "level=254", "off", "on"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines.len(), 6);
    assert!(lines[0].contains("applied_on=false"));
    assert!(lines[1].contains("raw_warm=6 raw_cool=2"));
    assert!(lines[2].contains("raw_warm=3 raw_cool=6"));
    assert!(lines[3].contains("raw_warm=12 raw_cool=22"));
    assert!(lines[4].contains("raw_warm=0 raw_cool=0"));
    assert!(lines[5].contains("applied_level=254 applied_mired=200"));
    assert!(lines[5].contains("selected_stock_brightness_percent=2530/253"));
    assert!(lines[5].contains("raw_warm=12 raw_cool=22"));
    assert!(lines[5].contains("output_writes=6"));
}

#[test]
fn invalid_commands_fail_before_starting_a_session() {
    for value in [
        "level=0",
        "level=255",
        "level=-1",
        "level=nope",
        "mired=142",
        "mired=345",
        "mired=65536",
        "brightness=3",
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
fn repeated_commands_do_not_write_output_and_presets_remain_conveniences() {
    let output = Command::new(env!("CARGO_BIN_EXE_key-right"))
        .args([
            "simulate",
            "preset-1",
            "level=57",
            "on",
            "preset-2",
            "mired=200",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<_> = stdout.lines().collect();
    assert_eq!(lines[0], lines[1]);
    assert_eq!(lines[1], lines[2]);
    assert_eq!(lines[4], lines[5]);
    assert!(lines[5].contains("output_writes=3"));
}
