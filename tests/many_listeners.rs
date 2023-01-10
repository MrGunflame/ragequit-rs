use std::time::{Duration, Instant};

use ragequit::SHUTDOWN;
use tokio::time::{sleep, sleep_until};

#[tokio::test]
async fn test_many_listeners() {
    for i in 0..64 {
        let listener = SHUTDOWN.listen();
        tokio::task::spawn(async move {
            tokio::pin!(listener);
            (&mut listener).await;

            // Drop delay on half of the listeners.
            if i >= 32 {
                sleep(Duration::from_secs(2)).await;
            }
        });
    }

    let now = Instant::now();
    tokio::task::spawn(async move {
        sleep_until((now + Duration::from_secs(3)).into()).await;
        SHUTDOWN.quit();
    });

    SHUTDOWN.wait().await;
    assert!(now.elapsed() >= Duration::from_secs(5));
}
