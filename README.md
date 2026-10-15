# Reynard

> The RipperFox successor[cite: 1].

Reynard is a cross-component utility featuring a Rust backend host and a companion browser extension designed for management, automation, and asset acquisition.

---

## Architecture

* **Backend (`src/`)**: Written in Rust, handling native host operations (`host.rs`), job management (`jobs.rs`), path resolution (`paths.rs`), configuration settings (`settings.rs`), shell integration (`shell.rs`), and external tools (`tools.rs`).
* **Browser Extension (`extension/`)**: Communicates with the backend, featuring dedicated background scripts, options dashboards (with flag configurations), and a clean popup interface.
* **Scripts (`release/`, root)**: Includes automation scripts (`install.bat`, `uninstall.bat`, `setup.bat`) and PowerShell utilities (`tools/fetch.ps1`) for deployment and environment setup.

---

## Installation

1. Clone or download the repository.
2. Run `setup.bat` or use the scripts provided in the `release/` directory to configure the environment and native host manifests.
3. Load the `extension/` directory as an unpacked extension in your browser.

---

## License

Distributed under the terms specified in the repository.