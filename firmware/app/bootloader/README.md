# ESP32-C3 bootloader

`esp32c3-bootloader.bin` is esp-lan-ota's rollback-enabled ESP-IDF
second-stage bootloader. `scripts/flash.sh` writes it on every USB flash and
refuses to run without it, if it lacks the ESP image magic, or if it would
reach the partition table at `0x8000`.

| Field | Value |
| --- | --- |
| Source | [esp-lan-ota](https://github.com/micthiesen/esp-lan-ota) `bootloaders/esp32c3-bootloader.bin` at rev `9b6df6712f2727ecdc7cce8045e39b347085340c` |
| Build | That repo's `bootloaders` CI workflow, ESP-IDF `release/v5.5` (`v5.5.5-832-g2553c5ad432`), espflash's bundled defaults plus `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y` |
| Size | 20,768 bytes |
| SHA-256 | `b75548cd37080f967922e6a022ab5c341b63bc33f653b357b86ab207895af6c0` (matches that rev's `SHA256SUMS`) |

Rollback support is what makes OTA safe: an updated image that never confirms
itself is replaced by the previous slot on its next reset. espflash's bundled
bootloader ignores image states, so `esp-ota status` would report
`rollback unsupported`. To replace it, copy the file from a newer esp-lan-ota
rev and update this table.
