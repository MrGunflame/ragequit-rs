//! Gracefully shut down a process
#[cfg(unix)]
mod unix;

use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::task::{Context, Poll};

use pin_project::{pin_project, pinned_drop};
use tokio::sync::futures::Notified;
use tokio::sync::Notify;

pub static SHUTDOWN: Shutdown = Shutdown::new();

pub fn init() {
    #[cfg(unix)]
    unix::init();
}

pub(crate) fn terminate() {
    SHUTDOWN.in_progress.store(true, Ordering::Release);
    SHUTDOWN.notify_shutdown.notify_waiters();

    if SHUTDOWN.counter.load(Ordering::Acquire) == 0 {
        SHUTDOWN.notify_done.notify_waiters();
    }
}

/// A RAII structure waiting for a shutdown signal.
#[pin_project(PinnedDrop)]
pub struct ShutdownListener {
    #[pin]
    notified: Notified<'static>,
}

impl ShutdownListener {
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

    pub fn quit(&'static self) {
        terminate();
    }

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
