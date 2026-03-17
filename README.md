# Gallery.rs

A modern, high-performance, real-time photo gallery server written in **Rust** using the **Axum** framework. This project provides a clean, responsive web interface to browse, upload, and manage images stored on your local filesystem, with live UI updates powered by WebSockets.

## ✨ Features

* **⚡ High Performance:** Built on the `tokio`/`axum` stack for asynchronous, non-blocking I/O.
* **🔄 Real-Time Sync:** Uses `notify` to watch the filesystem and WebSockets to instantly refresh the UI across all connected clients when images are added, modified, or removed.
* **🛡️ Secure Path Resolution:** Prevents directory traversal attacks by canonicalizing paths and enforcing boundaries.
* **📱 Modern UI:** A responsive, dark-themed frontend built with **Tailwind CSS** featuring a smooth lightbox preview.
* **📤 Easy Management:** Support for bulk image uploads via file selection and quick-delete functionality.
* **🔍 Detailed Metadata:** Instantly view file size and last modified timestamps for any image in the gallery.

## 🚀 Quick Start

### Prerequisites
* [Rust](https://www.rust-lang.org/tools/install) (2024 edition or later)

### Installation
1.  Clone the repository:
    ```bash
    git clone [https://github.com/your-username/gallery-rs.git](https://github.com/your-username/gallery-rs.git)
    cd gallery-rs
    ```
2.  Build and run the project:
    ```bash
    cargo run --release
    ```

### Usage
Run the server by specifying your photo storage directory:
```bash
cargo run --release -- --storage-dir ./my-photos --port 3020
