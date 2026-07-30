mod common;

use tockloader_lib::{connection::Connection, CommandEraseApps, CommandList};

#[tokio::test]
pub async fn test_erase_apps_microbit() {
    let mut conn = common::connect_microbit().await;

    conn.erase_apps().await.expect("Failed to erase apps.");

    let apps_after = conn.list().await.expect("Failed to list apps after erase.");
    assert!(
        apps_after.is_empty(),
        "Expected no apps after erase_apps, but found {}.",
        apps_after.len()
    );

    conn.close().await.expect("Failed to close connection.");
}
