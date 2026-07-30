mod common;

use tockloader_lib::{connection::Connection, CommandList};

#[tokio::test]
pub async fn test_list_microbit() {
    let mut conn = common::connect_microbit().await;

    let apps = conn.list().await.expect("Failed to list apps.");

    // Structural checks only -- valid for whatever happens to be installed.
    for app in &apps {
        assert!(
            app.tbf_header.total_size() > 0,
            "App at address {:#x} reports a zero total_size.",
            app.address
        );
        assert!(
            app.tbf_header.get_package_name().is_some(),
            "App at address {:#x} has no package name.",
            app.address
        );
    }

    dbg!(&apps);

    conn.close().await.expect("Failed to close connection.");
}
