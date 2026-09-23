# Third-party source notices

`firmware/app/src/network.rs` and `network_driver.rs` adapt the ESP Wi-Fi
controller and peripheral lifecycle adapters from
[rs-matter-embassy](https://github.com/ivmarkov/rs-matter-embassy), revision
`f31233a6fd4530ff25ad3bcbd9abf8fe854320aa`, by Ivan Markov and contributors.
The package declares `MIT OR Apache-2.0`; these source adaptations use its MIT
option. The upstream [MIT notice](licenses/rs-matter-embassy-MIT.txt) is retained
verbatim, including its original copyright attribution.

Changes add scan/connect deadlines, transport restart signalling, RSSI and local
interface diagnostics. The complete dependency versions remain in Cargo.lock;
this notice concerns source copied into this repository, not every linked crate.

The application setup and original bench transport came from the user's Stillair
repository, revision `af12fec55430b4af7704dd89636bdd102a0c4158`. Adaptation details
are in [development.md](docs/development.md). The proprietary Elgato firmware used
for offline analysis is not distributed in this repository.
