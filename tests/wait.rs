use std::time::{Duration, Instant};

use ragequit::SHUTDOWN;
use tokio::time::sleep_until;

#[tokio::test]
async fn wait() {
    let listener1 = SHUTDOWN.listen();
    tokio::task::spawn(async move {
        listener1.await;
    });

    let now = Instant::now();
    tokio::task::spawn(async move {
        sleep_until((now + Duration::from_secs(3)).into()).await;
        SHUTDOWN.quit();
    });

    SHUTDOWN.wait().await;
    assert!(now.elapsed() >= Duration::from_secs(3));
}
