//! Bounded newline-delimited local USB protocol. No network debug endpoint.
use crate::runtime::{Hardware, Runtime};
use core::fmt::Write as _;
use embedded_io_async::Read as _;
use esp_hal::usb::usb_serial_jtag::UsbSerialJtagRx;
use esp_hal::Async;
use heapless::String;
use key_right_core::{ColorTemperature, Level, LightState, Preset};
use rs_matter_embassy::matter::error::{Error, ErrorCode};
use rs_matter_embassy::matter::persist::KvBlobStoreAccess;

pub async fn run<K: KvBlobStoreAccess, H: Hardware>(
    mut rx: UsbSerialJtagRx<'static, Async>,
    runtime: &Runtime<K, H>,
    pairing: &str,
    pairing_qr: &str,
    open_commissioning: impl Fn() -> Result<(), rs_matter_embassy::matter::error::Error>,
    is_commissioned: impl Fn() -> bool,
) -> ! {
    let mut line = String::<256>::new();
    let mut overflow = false;
    let mut bytes = [0; 64];
    loop {
        let count = match rx.read(&mut bytes).await {
            Ok(n) => n,
            Err(_) => continue,
        };
        for &byte in &bytes[..count] {
            if byte == b'\r' {
                continue;
            }
            if byte == b'\n' {
                let mut reply = String::<1024>::new();
                if overflow {
                    let _ = reply.push_str("KR ERR line_too_long");
                } else if matches!(line.trim(), "test wifi" | "test network") {
                    if !is_commissioned() {
                        let _ = reply.push_str("KR ERR requires_commissioned_device");
                    } else if line.trim() == "test wifi" {
                        match crate::network::request_disconnect() {
                            Ok(()) => {
                                let _ = reply.push_str(
                                    "KR OK wifi_disconnect_requested intent_preserved=true",
                                );
                            }
                            Err(error) => {
                                let _ = write!(reply, "KR ERR {:?}", error.code());
                            }
                        }
                    } else {
                        let _ =
                            reply.push_str("KR OK network_restart_requested intent_preserved=true");
                        crate::output::response_and_flush(reply).await;
                        crate::network::RESTART.signal(());
                        line.clear();
                        overflow = false;
                        continue;
                    }
                } else if line.trim() == "reboot" {
                    let off_verified = runtime.prepare_reboot();
                    let _ = write!(reply, "KR OK rebooting intent_preserved=true off_registers_verified={off_verified}");
                    crate::output::response_and_flush(reply).await;
                    esp_hal::system::software_reset();
                } else if line.trim() == "test watchdog" {
                    match runtime.watchdog_test_ready() {
                        Ok(()) => {
                            let _ = reply.push_str(
                                "KR OK watchdog_test reset_expected_seconds=15 intent_preserved=true",
                            );
                            crate::output::response_and_flush(reply).await;
                            // This console and the watchdog-feeding maintenance future
                            // share one thread-mode task. Deliberately stop polling it;
                            // The PCA retains its last PWM state during this stall.
                            loop {
                                core::hint::spin_loop();
                            }
                        }
                        Err(e) => {
                            let _ = write!(reply, "KR ERR {:?}", e.code());
                        }
                    }
                } else {
                    command(
                        line.as_str(),
                        runtime,
                        pairing,
                        pairing_qr,
                        &open_commissioning,
                        &mut reply,
                    );
                }
                crate::output::response(reply).await;
                line.clear();
                overflow = false;
            } else if !(0x20..=0x7e).contains(&byte) || line.push(byte as char).is_err() {
                overflow = true;
            }
        }
    }
}

fn command<K: KvBlobStoreAccess, H: Hardware>(
    line: &str,
    runtime: &Runtime<K, H>,
    pairing: &str,
    pairing_qr: &str,
    open_commissioning: &impl Fn() -> Result<(), rs_matter_embassy::matter::error::Error>,
    out: &mut String<1024>,
) {
    let mut words = line.split_ascii_whitespace();
    let command = words.next().unwrap_or("");
    let args: [Option<&str>; 3] = [words.next(), words.next(), words.next()];
    if words.next().is_some() {
        let _ = out.push_str("KR ERR arguments");
        return;
    }
    let result = match (command, args) {
        ("status", [None, None, None]) => {
            let s = runtime.snapshot();
            let (attempts, timeouts, restarts, connected) = crate::network::metrics();
            let (rssi, local_ready, ipv4_ready, ip_timeouts) = crate::network::local_metrics();
            let _=write!(out,"KR OK mode={} firmware={} chip=esp32c3 identity=KR-{} uptime_ms={} reset={:?} intended_on={} intended_level={} intended_mired={} stock_range_percent=1..10 acknowledged={:?} fault={:?} output_failures={} storage_failures={} recoveries={} wifi_connected={} rssi_dbm={:?} local_ip_ready={} ipv4_ready={} wifi_attempts={} wifi_timeouts={} ip_timeouts={} wifi_restarts={} usb_dropped={} physical_output=unmeasured",
                H::MODE,env!("CARGO_PKG_VERSION"),esp_hal::efuse::base_mac_address(),embassy_time::Instant::now().as_millis(),esp_hal::system::reset_reason(),
                s.intended.on,s.intended.level.get(),s.intended.temperature.get(),s.applied,s.fault,s.output_failures,s.storage_failures,s.recoveries,
                connected,rssi,local_ready,ipv4_ready,attempts,timeouts,ip_timeouts,restarts,crate::output::dropped());
            let _ = write!(out, " acknowledged_stock_percent_numerator={:?} stock_percent_denominator=253 level_remaining_ms={} temperature_remaining_ms={}",
                s.applied_frame.map(|frame| frame.brightness.get()), runtime.level_remaining_ms(), runtime.temperature_remaining_ms());
            return;
        }
        ("off", [None, None, None]) => runtime.off(),
        ("on", [None, None, None]) => runtime.set_power(true),
        ("on", [Some(p @ ("1" | "2")), None, None]) => runtime.request(LightState {
            on: true,
            temperature: if p == "1" {
                Preset::One.temperature()
            } else {
                Preset::Two.temperature()
            },
            ..LightState::default()
        }),
        ("level", [Some(value), None, None]) => value
            .parse::<u8>()
            .ok()
            .and_then(Level::new)
            .ok_or_else(|| Error::from(ErrorCode::ConstraintError))
            .and_then(|level| runtime.set_level(level, false)),
        ("temperature", [Some(value), None, None]) => value
            .parse::<u16>()
            .ok()
            .and_then(ColorTemperature::new)
            .ok_or_else(|| Error::from(ErrorCode::ConstraintError))
            .and_then(|temperature| runtime.set_temperature(temperature)),
        ("verify", [None, None, None]) => runtime.verify(),
        ("registers" | "outputs", [None, None, None]) => {
            match runtime.registers() {
                Ok(registers) => {
                    let _ = write!(
                        out,
                        "KR OK mode={} backend=pca9635 address=0x15 registers=",
                        H::MODE
                    );
                    for byte in registers {
                        let _ = write!(out, "{byte:02x}");
                    }
                    let _ = out.push_str(" physical_output=unmeasured");
                }
                Err(e) => {
                    let _ = write!(out, "KR ERR {:?}", e.code());
                }
            }
            return;
        }
        ("on1", [None, None, None]) => runtime.request(LightState {
            on: true,
            temperature: Preset::One.temperature(),
            ..LightState::default()
        }),
        ("on2", [None, None, None]) => runtime.request(LightState {
            on: true,
            temperature: Preset::Two.temperature(),
            ..LightState::default()
        }),
        ("commissioning", [Some(format @ ("code" | "qr")), None, None]) => {
            if let Err(e) = open_commissioning() {
                let _ = write!(out, "KR ERR commissioning_window_{:?}", e.code());
            } else if format == "qr" {
                let _ = write!(out, "KR OK qr_payload={pairing_qr}");
            } else {
                let _ = write!(out, "KR OK pairing_code={pairing}");
            }
            return;
        }
        _ => {
            let _=out.push_str("KR ERR commands=status|off|on|level_1..254|temperature_143..344|on_1_or_2|verify|registers|commissioning_code|commissioning_qr|reboot|test_watchdog|test_wifi|test_network");
            return;
        }
    };
    match result {
        Ok(()) => {
            let _ = out.push_str("KR OK");
        }
        Err(e) => {
            let _ = write!(out, "KR ERR {:?}", e.code());
        }
    }
}
