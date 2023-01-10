use std::time::{Duration, Instant};

use ragequit::SHUTDOWN;
use tokio::time::{sleep, sleep_until};

#[tokio::test]
async fn test_slow_listeners() {
    for _ in 0..8 {
        let listener = SHUTDOWN.listen();
        tokio::task::spawn(async move {
            tokio::pin!(listener);
            (&mut listener).await;

            sleep(Duration::from_secs(5)).await;
        });
    }

    let now = Instant::now();
    tokio::task::spawn(async move {
        sleep_until((now + Duration::from_secs(3)).into()).await;
        SHUTDOWN.quit();
    });

    SHUTDOWN.wait().await;
    dbg!(now.elapsed());
    assert!(now.elapsed() >= Duration::from_secs(8));
}
