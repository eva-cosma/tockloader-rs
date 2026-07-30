mod common;

use tockloader_lib::{connection::Connection, CommandInfo, CommandList};

#[tokio::test]
pub async fn test_info_microbit() {
    let mut conn = common::connect_microbit().await;

    let apps_via_list = conn.list().await.expect("Failed to list apps.");
    let info = conn.info().await.expect("Failed to read board info.");

    assert_eq!(info.system.arch.as_deref(), Some("cortex-m4"));
    assert!(info.system.appaddr.is_some());
    assert!(info.system.bootloader_version.is_some());

    assert_eq!(
        info.apps.len(),
        apps_via_list.len(),
        "info() and list() disagree on the number of installed apps."
    );

    let mut info_names: Vec<_> = info
        .apps
        .iter()
        .map(|a| a.tbf_header.get_package_name())
        .collect();
    let mut list_names: Vec<_> = apps_via_list
        .iter()
        .map(|a| a.tbf_header.get_package_name())
        .collect();
    info_names.sort();
    list_names.sort();

    assert_eq!(info_names, list_names);

    conn.close().await.expect("Failed to close connection.");
}
