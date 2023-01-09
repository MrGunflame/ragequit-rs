//! Gracefully shut down a process
//!
//! `ragequit` provides a set of utilities to gracefully shut down a process. It is primarily
//! targeted at server processes, but may have other applications as well.
//!
//! # Usage
//!
//! The global [`SHUTDOWN`] instance is used to signal shutdown events and handle them gracefully
//! by creating [`ShutdownListener`]s.
//!
//! ```no_run
//! use ragequit::{init, SHUTDOWN};
//!
//! #[tokio::main]
//! async fn main() {
//!     // Install default system signal handlers.
//!     init();
//!
//!     let listener = SHUTDOWN.listen();
//!     tokio::spawn(async move {
//!         // Wait for the shutdown signal.
//!         tokio::pin!(listener);
//!         (&mut listener).await;
//!
//!         // Drop the listener, allowing the main process to exit.
//!         println!("Goodbye");
//!         drop(listener);
//!     });
//!
//!     // Wait for a shutdown signal and for all listeners to be dropped.
//!     SHUTDOWN.wait().await;
//! }
//! ```
//!
//! Call [`init`] once during the start of the process to install the default system signal
//! handlers. Alternatively you can install system signal handlers yourself.
//!
//! ## Example for *nix systems
//!
//! ```no_run
//! use core::ffi::c_int;
//!
//! use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};
//! use ragequit::SHUTDOWN;
//!
//! let action = SigAction::new(SigHandler::Handler(quit), SaFlags::empty(), SigSet::empty());
//!
//! unsafe {
//!     let _ = sigaction(Signal::SIGINT, &action);
//!     let _ = sigaction(Signal::SIGTERM, &action);
//! }
//!
//! extern "C" fn quit(_: c_int) {
//!     SHUTDOWN.quit();
//! }
//! ```
//!
//! # Tokio dependency
//!
//! `ragequit` depends on [`tokio`] only for synchronization primitives. It does not depend on the
//! tokio runtime. `ragequit` works in any asynchronous runtime.

#[cfg(target_family = "unix")]
mod unix;

#[cfg(target_family = "windows")]
mod windows;

use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::task::{Context, Poll};

use pin_project::{pin_project, pinned_drop};
use tokio::sync::futures::Notified;
use tokio::sync::Notify;

/// The global [`Shutdown`] instance.
pub static SHUTDOWN: Shutdown = Shutdown::new();

/// Initializes the global [`SHUTDOWN`] instance by installing system signal handlers.
pub fn init() {
    #[cfg(target_family = "unix")]
    unix::init();

    #[cfg(target_family = "windows")]
    windows::init();
}

#[inline]
pub(crate) fn terminate() {
    SHUTDOWN.quit();
}

/// A future and RAII structure waiting for a shutdown signal.
///
/// `ShutdownListener` completes once a shutdown signal has been received. All future calls to
/// `poll` will complete immediately.
///
/// `ShutdownListener` also doubles as a RAII strucuture. While this instance is kept alive, the
/// process will not exit.
#[pin_project(PinnedDrop)]
pub struct ShutdownListener {
    #[pin]
    notified: Notified<'static>,
}

impl ShutdownListener {
    /// Returns `true` if a shutdown signal has been received yet.
    ///
    /// Once this function returns `true`, all future calls will also return `true` and calls to
    /// [`poll`] will resolve immediately.
    ///
    /// [`poll`]: Future::poll
    #[inline]
    pub fn is_in_progress(&self) -> bool {
        SHUTDOWN.in_progress.load(Ordering::Acquire)
    }
}

impl Future for ShutdownListener {
    type Output = ();

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.is_in_progress() {
            return Poll::Ready(());
        }

        self.project().notified.poll(cx)
    }
}

#[pinned_drop]
impl PinnedDrop for ShutdownListener {
    #[inline]
    fn drop(self: Pin<&mut Self>) {
        SHUTDOWN.dec();
    }
}

/// A group of [`ShutdownListener`]s waiting for a shutdown signal.
#[derive(Debug)]
pub struct Shutdown {
    in_progress: AtomicBool,
    counter: AtomicUsize,
    notify_shutdown: Notify,
    notify_done: Notify,
}

impl Shutdown {
    #[inline]
    const fn new() -> Self {
        Self {
            in_progress: AtomicBool::new(false),
            counter: AtomicUsize::new(0),
            notify_shutdown: Notify::const_new(),
            notify_done: Notify::const_new(),
        }
    }

    #[inline]
    fn inc(&self) {
        self.counter.fetch_add(1, Ordering::Acquire);
    }

    #[inline]
    fn dec(&self) {
        let prev = self.counter.fetch_sub(1, Ordering::AcqRel);

        if self.in_progress.load(Ordering::Acquire) && prev == 1 {
            self.notify_done.notify_waiters();
        }
    }

    /// Manually shut down this `Shutdown` instance.
    ///
    /// This has no effect if called multiple times.
    #[inline]
    pub fn quit(&'static self) {
        self.in_progress.store(true, Ordering::Release);
        self.notify_shutdown.notify_waiters();

        if self.counter.load(Ordering::Acquire) == 0 {
            self.notify_done.notify_waiters();
        }
    }

    /// Creates a new [`ShutdownListener`] on this `Shutdown` instance.
    #[inline]
    pub fn listen(&'static self) -> ShutdownListener {
        self.inc();

        ShutdownListener {
            notified: self.notify_shutdown.notified(),
        }
    }

    /// Returns a future that completes once a shutdown signal has been received and all
    /// [`ShutdownListener`]s have been dropped.
    #[inline]
    pub fn wait(&self) -> Wait<'_> {
        Wait {
            inner: self,
            notified: self.notify_done.notified(),
        }
    }
}

/// A future that completes once a shutdown signal has been received and all [`ShutdownListener`]s
/// have been dropped.
#[pin_project]
pub struct Wait<'a> {
    inner: &'a Shutdown,
    #[pin]
    notified: Notified<'a>,
}

impl<'a> Future for Wait<'a> {
    type Output = ();

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.inner.in_progress.load(Ordering::Acquire)
            && self.inner.counter.load(Ordering::Acquire) == 0
        {
            return Poll::Ready(());
        }

        self.project().notified.poll(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::SHUTDOWN;

    #[test]
    fn test_shutdown_counter() {
        let listener1 = SHUTDOWN.listen();
        let listener2 = SHUTDOWN.listen();
        assert_eq!(SHUTDOWN.counter.load(Ordering::Acquire), 2);

        drop(listener2);
        assert_eq!(SHUTDOWN.counter.load(Ordering::Acquire), 1);

        drop(listener1);
        assert_eq!(SHUTDOWN.counter.load(Ordering::Acquire), 0);
    }
}
