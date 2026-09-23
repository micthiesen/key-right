//! Bounded newline-delimited local USB protocol. No network debug endpoint.
use crate::runtime::{Hardware, Runtime};
use core::fmt::Write as _;
use embedded_io_async::Read as _;
use esp_hal::usb::usb_serial_jtag::UsbSerialJtagRx;
use esp_hal::Async;
use heapless::String;
use key_right_core::Preset;
use rs_matter_embassy::matter::persist::KvBlobStoreAccess;

pub async fn run<K: KvBlobStoreAccess, H: Hardware>(
    mut rx: UsbSerialJtagRx<'static, Async>,
    runtime: &Runtime<K, H>,
    pairing: &str,
    open_commissioning: impl Fn() -> Result<(), rs_matter_embassy::matter::error::Error>,
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
            let _=write!(out,"KR OK mode=hardware firmware={} identity=KR-{} uptime_ms={} reset={:?} intended_on={} intended_preset={} acknowledged={:?} fault={:?} output_failures={} storage_failures={} recoveries={} wifi_connected={} rssi_dbm={:?} local_ip_ready={} ipv4_ready={} wifi_attempts={} wifi_timeouts={} ip_timeouts={} wifi_restarts={} usb_dropped={} physical_output=unmeasured",
                env!("CARGO_PKG_VERSION"),esp_hal::efuse::base_mac_address(),embassy_time::Instant::now().as_millis(),esp_hal::system::reset_reason(),
                s.intended.on,if s.intended.preset==Preset::One {1}else{2},s.applied,s.fault,s.output_failures,s.storage_failures,s.recoveries,
                connected,rssi,local_ready,ipv4_ready,attempts,timeouts,ip_timeouts,restarts,crate::output::dropped());
            return;
        }
        ("off", [None, None, None]) => runtime.off(),
        ("on", [Some(p @ ("1" | "2")), None, None]) => {
            runtime.set_endpoint(if p == "1" { Preset::One } else { Preset::Two }, true)
        }
        ("verify", [None, None, None]) => runtime.verify(),
        ("registers" | "outputs", [None, None, None]) => {
            match runtime.registers() {
                Ok(registers) => {
                    let _ = out.push_str("KR OK backend=pca9635 address=0x15 registers=");
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
        ("on1", [None, None, None]) => runtime.set_endpoint(Preset::One, true),
        ("on2", [None, None, None]) => runtime.set_endpoint(Preset::Two, true),
        ("commissioning", [Some("code"), None, None]) => {
            if let Err(e) = open_commissioning() {
                let _ = write!(out, "KR ERR commissioning_window_{:?}", e.code());
            } else {
                let _ = write!(out, "KR OK pairing_code={pairing}");
            }
            return;
        }
        _ => {
            let _=out.push_str("KR ERR commands=status|off|on_1_or_2|verify|registers|commissioning_code|reboot|test_watchdog");
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
