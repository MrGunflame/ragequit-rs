# ragequit

[![Crates.io](https://img.shields.io/crates/v/ragequit)](https://crates.io/crates/ragequit)
[![Docs.rs](https://img.shields.io/docsrs/ragequit/latest)](https://docs.rs/ragequit)


Gracefully shut down a process

`ragequit` provides a set of utilities to shut down a process. It is primarily targeted at
server processes, but may have other applications aswell.


# Usage

The global [`SHUTDOWN`] instance is used to signal shutdown events and handle them gracefully
by creating [`ShutdownListener`]s.

```rust
use ragequit::{init, SHUTDOWN};

#[tokio::main]
async fn main() {
    // Install default system signal handlers.
    init();

    let listener = SHUTDOWN.listen();
    tokio::spawn(async move {
        // Wait for the shutdown signal.
        tokio::pin!(listener);
        (&mut listener).await;

        // Drop the listener, allowing the main process to exit.
        println!("Goodbye");
        drop(listener);
    });

    // Wait for a shutdown signal and for all listeners to be dropped.
    SHUTDOWN.wait().await;
}
```

Call [`init`] once during the start of the process to install the default system signal
handlers. Alternatively you can install system signal handlers yourself.

## Example for *nix systems

```rust
use core::ffi::c_int;

use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};
use ragequit::SHUTDOWN;

let action = SigAction::new(SigHandler::Handler(quit), SaFlags::empty(), SigSet::empty());

unsafe {
    let _ = sigaction(Signal::SIGINT, &action);
    let _ = sigaction(Signal::SIGTERM, &action);
}

extern "C" fn quit(_: c_int) {
    SHUTDOWN.quit();
}
```

# Tokio dependency

`ragequit` depends on [`tokio`] only for synchronization primitives. It does not depend on the
tokio runtime. `ragequit` works in any asynchronous runtime.

# License

Licensed under either [MIT License](https://github.com/MrGunflame/ragequit-rs/blob/master/LICENSE-MIT) or [Apache License, Version 2.0](https://github.com/MrGunflame/ragequit-rs/blob/master/LICENSE-APACHE) at your option.
