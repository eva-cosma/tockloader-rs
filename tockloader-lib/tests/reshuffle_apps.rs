use tockloader_lib::{board_settings::BoardSettings, command_impl::reshuffle_apps::*};

#[test]
fn install_new_c_app() {
    let settings: &BoardSettings = &BoardSettings {
        arch: Some("cortex-m4".to_string()),
        start_address: 0x00040000,
        page_size: 512,
        ram_start_address: 0x20000000,
    };

    let c_app_1: TockApp = TockApp::Flexible(FlexibleApp {
        installed: true,
        idx: None,
        size: 0x2000,
    });

    let c_app_2: TockApp = TockApp::Flexible(FlexibleApp {
        installed: true,
        idx: None,
        size: 0x2000,
    });

    let c_app_3: TockApp = TockApp::Flexible(FlexibleApp {
        installed: false,
        idx: None,
        size: 0x2000,
    });

    let apps: Vec<TockApp> = vec![c_app_1, c_app_2, c_app_3];

    let reshuffled_apps = reshuffle_apps(settings, apps);

    let correct_config: Option<Vec<Index>> = Some(vec![
        Index {
            installed: true,
            idx: Some(0x0),
            ram_address: None,
            address: 0x40000,
            size: 0x2000,
        },
        Index {
            installed: true,
            idx: Some(0x1),
            ram_address: None,
            address: 0x42000,
            size: 0x2000,
        },
        Index {
            installed: false,
            idx: Some(0x2),
            ram_address: None,
            address: 0x44000,
            size: 0x2000,
        },
    ]);
    assert_eq!(reshuffled_apps, correct_config);
}

#[test]
fn install_new_rust_app() {
    let settings: &BoardSettings = &BoardSettings {
        arch: Some("cortex-m4".to_string()),
        start_address: 0x00040000,
        page_size: 512,
        ram_start_address: 0x20000000,
    };

    let rust_app: TockApp = TockApp::Fixed(FixedApp {
        installed: false,
        idx: None,
        compatible_addresses: vec![
            Some((0x40000, 0x20008000)),
            Some((0x42000, 0x2000a000)),
            Some((0x48000, 0x20010000)),
            Some((0x80000, 0x20006000)),
            Some((0x88000, 0x2000e000)),
        ],
        size: 0x2000,
    });

    let apps: Vec<TockApp> = vec![rust_app];

    let reshuffled_apps = reshuffle_apps(settings, apps);

    let correct_config: Option<Vec<Index>> = Some(vec![Index {
        installed: false,
        idx: Some(0x0),
        ram_address: Some(0x20008000),
        address: 0x40000,
        size: 0x2000,
    }]);
    assert_eq!(reshuffled_apps, correct_config);
}

#[test]
fn install_more_rust_apps() {
    let settings: &BoardSettings = &BoardSettings {
        arch: Some("cortex-m4".to_string()),
        start_address: 0x00040000,
        page_size: 512,
        ram_start_address: 0x20000000,
    };

    let rust_app_1: TockApp = TockApp::Fixed(FixedApp {
        installed: true,
        idx: None,
        compatible_addresses: vec![Some((0x40000, 0x20008000))],
        size: 0x2000,
    });

    let rust_app_2: TockApp = TockApp::Fixed(FixedApp {
        installed: true,
        idx: None,
        compatible_addresses: vec![Some((0x42000, 0x2000a000))],
        size: 0x2000,
    });

    let rust_app_3: TockApp = TockApp::Fixed(FixedApp {
        installed: false,
        idx: None,
        compatible_addresses: vec![
            Some((0x40000, 0x20008000)),
            Some((0x42000, 0x2000a000)),
            Some((0x48000, 0x20010000)),
            Some((0x80000, 0x20006000)),
            Some((0x88000, 0x2000e000)),
        ],
        size: 0x2000,
    });

    let apps: Vec<TockApp> = vec![rust_app_1, rust_app_2, rust_app_3];

    let reshuffled_apps = reshuffle_apps(settings, apps);

    let correct_config: Option<Vec<Index>> = Some(vec![
        Index {
            installed: true,
            idx: Some(0x0),
            ram_address: Some(0x20008000),
            address: 0x40000,
            size: 0x2000,
        },
        Index {
            installed: true,
            idx: Some(0x1),
            ram_address: Some(0x2000a000),
            address: 0x42000,
            size: 0x2000,
        },
        // padding
        Index {
            installed: false,
            idx: None,
            ram_address: None,
            address: 0x44000,
            size: 0x4000,
        },
        Index {
            installed: false,
            idx: Some(0x2),
            ram_address: Some(0x20010000),
            address: 0x48000,
            size: 0x2000,
        },
    ]);
    assert_eq!(reshuffled_apps, correct_config);
}

#[test]
fn insert_c_app_between_rust_apps() {
    let settings: &BoardSettings = &BoardSettings {
        arch: Some("cortex-m4".to_string()),
        start_address: 0x00040000,
        page_size: 512,
        ram_start_address: 0x20000000,
    };

    let rust_app_1: TockApp = TockApp::Fixed(FixedApp {
        installed: true,
        idx: None,
        compatible_addresses: vec![Some((0x40000, 0x20008000))],
        size: 0x2000,
    });

    let rust_app_2: TockApp = TockApp::Fixed(FixedApp {
        installed: true,
        idx: None,
        compatible_addresses: vec![Some((0x42000, 0x2000a000))],
        size: 0x2000,
    });

    let rust_app_3: TockApp = TockApp::Fixed(FixedApp {
        installed: true,
        idx: None,
        compatible_addresses: vec![Some((0x48000, 0x20010000))],
        size: 0x2000,
    });

    let c_app_1: TockApp = TockApp::Flexible(FlexibleApp {
        installed: false,
        idx: None,
        size: 0x2000,
    });

    let apps: Vec<TockApp> = vec![rust_app_1, rust_app_2, rust_app_3, c_app_1];

    let reshuffled_apps = reshuffle_apps(settings, apps);

    let correct_config: Option<Vec<Index>> = Some(vec![
        Index {
            installed: true,
            idx: Some(0x0),
            ram_address: Some(0x20008000),
            address: 0x40000,
            size: 0x2000,
        },
        Index {
            installed: true,
            idx: Some(0x1),
            ram_address: Some(0x2000a000),
            address: 0x42000,
            size: 0x2000,
        },
        Index {
            installed: false,
            idx: Some(0x3),
            ram_address: None,
            address: 0x44000,
            size: 0x2000,
        },
        // padding
        Index {
            installed: false,
            idx: None,
            ram_address: None,
            address: 0x46000,
            size: 0x2000,
        },
        Index {
            installed: true,
            idx: Some(0x2),
            ram_address: Some(0x20010000),
            address: 0x48000,
            size: 0x2000,
        },
    ]);
    assert_eq!(reshuffled_apps, correct_config);
}
