use std::future::Future;
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(unix)]
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[cfg(unix)]
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
extern "C" fn handle_interrupt(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::Release);
}

pub(crate) struct InterruptGuard {
    #[cfg(unix)]
    previous: libc::sighandler_t,
}

impl InterruptGuard {
    pub(crate) fn install() -> Self {
        #[cfg(unix)]
        {
            INTERRUPTED.store(false, Ordering::Release);
            let previous = unsafe {
                libc::signal(
                    libc::SIGINT,
                    handle_interrupt as *const () as libc::sighandler_t,
                )
            };
            Self { previous }
        }
        #[cfg(not(unix))]
        Self {}
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        if self.previous != libc::SIG_ERR {
            unsafe {
                libc::signal(libc::SIGINT, self.previous);
            }
        }
    }
}

pub(crate) async fn wait_for_interrupt() {
    #[cfg(unix)]
    loop {
        if INTERRUPTED.swap(false, Ordering::AcqRel) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    #[cfg(not(unix))]
    std::future::pending::<()>().await;
}

pub(crate) fn is_interrupted() -> bool {
    #[cfg(unix)]
    {
        INTERRUPTED.load(Ordering::Acquire)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

pub(crate) async fn cancellable<T>(
    future: impl Future<Output = T>,
    cancel: CancellationToken,
) -> T {
    tokio::pin!(future);
    tokio::select! {
        result = &mut future => result,
        _ = wait_for_interrupt() => {
            cancel.cancel();
            // Await the runner so process-group cleanup and cancelled evidence finish.
            future.await
        }
    }
}
