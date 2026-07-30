mod common;

use regression_test::RegTest;
use regression_test_macros::regtest;
use tockloader_lib::{connection::Connection, tabs::tab::Tab, CommandInstall, CommandList};

#[regtest]
#[tokio::test]
pub async fn test_install_microbit(mut rt: RegTest) {
    let mut conn = common::connect_microbit().await;

    let tab = Tab::open(common::c_hello_tab_path()).expect("Failed to open c_hello.tab");
    conn.install_app(tab).await.expect("Failed to install app.");

    let apps = conn.list().await.expect("Failed to list apps after install.");

    assert_eq!(apps.len(), 1);
    assert_eq!(
        apps[0].tbf_header.get_package_name(),
        Some(common::C_HELLO_PACKAGE_NAME)
    );

    // Deterministic here: we know exactly what we just installed.
    rt.regtest_dbg(apps);

    conn.close().await.expect("Failed to close connection.");
}
