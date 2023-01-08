use futures::FutureExt;
use ragequit::SHUTDOWN;

#[test]
fn test_no_listeners() {
    SHUTDOWN.quit();
    SHUTDOWN.wait().now_or_never().unwrap();
}
