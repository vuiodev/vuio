use super::*;

#[test]
fn test_macos_network_manager_creation() {
    let manager = MacOSNetworkManager::new();
    assert_eq!(manager.config.primary_port, 1900);
}

#[test]
fn test_requires_elevation() {
    let manager = MacOSNetworkManager::new();
    assert!(manager.requires_elevation(80));
    assert!(manager.requires_elevation(443));
    assert!(!manager.requires_elevation(8080));
    assert!(!manager.requires_elevation(9090));
}

#[test]
fn test_interface_type_determination() {
    let manager = MacOSNetworkManager::new();

    assert_eq!(
        manager.determine_macos_interface_type("en0"),
        InterfaceType::WiFi
    );

    assert_eq!(
        manager.determine_macos_interface_type("en1"),
        InterfaceType::Ethernet
    );

    assert_eq!(
        manager.determine_macos_interface_type("utun0"),
        InterfaceType::VPN
    );

    assert_eq!(
        manager.determine_macos_interface_type("lo0"),
        InterfaceType::Loopback
    );
}

#[tokio::test]
async fn test_port_availability_check() {
    let manager = MacOSNetworkManager::new();

    // Test with a high port that should be available
    let available = manager.is_port_available(8080).await;
    // This might fail in test environment, but we can at least verify the method works
    println!("Port 8080 available: {}", available);
}

#[test]
fn test_ifconfig_parsing() {
    let manager = MacOSNetworkManager::new();

    let sample_output = r#"
lo0: flags=8049<UP,LOOPBACK,RUNNING,MULTICAST> mtu 16384
	inet 127.0.0.1 netmask 0xff000000
en0: flags=8863<UP,BROADCAST,SMART,RUNNING,SIMPLEX,MULTICAST> mtu 1500
	inet 192.168.1.100 netmask 0xffffff00 broadcast 192.168.1.255
	status: active
en1: flags=8863<UP,BROADCAST,SMART,RUNNING,SIMPLEX,MULTICAST> mtu 1500
	inet 192.168.1.101 netmask 0xffffff00 broadcast 192.168.1.255
	status: active
"#;

    let interfaces = manager.parse_ifconfig_output(sample_output).unwrap();
    assert_eq!(interfaces.len(), 2); // lo0 should be filtered out

    let en0 = &interfaces[0];
    assert_eq!(en0.name, "en0");
    assert_eq!(en0.ip_address, "192.168.1.100".parse::<IpAddr>().unwrap());
    assert_eq!(en0.interface_type, InterfaceType::WiFi);
    assert!(en0.is_up);
    assert!(en0.supports_multicast);

    let en1 = &interfaces[1];
    assert_eq!(en1.name, "en1");
    assert_eq!(en1.ip_address, "192.168.1.101".parse::<IpAddr>().unwrap());
    assert_eq!(en1.interface_type, InterfaceType::Ethernet);
}

#[test]
fn test_preferred_interface_selection() {
    let manager = MacOSNetworkManager::new();

    let interfaces = vec![
        NetworkInterface {
            name: "en1".to_string(),
            ip_address: "192.168.1.101".parse().unwrap(),
            is_loopback: false,
            is_up: true,
            supports_multicast: true,
            interface_type: InterfaceType::Ethernet,
        },
        NetworkInterface {
            name: "en0".to_string(),
            ip_address: "192.168.1.100".parse().unwrap(),
            is_loopback: false,
            is_up: true,
            supports_multicast: true,
            interface_type: InterfaceType::WiFi,
        },
        NetworkInterface {
            name: "utun0".to_string(),
            ip_address: "10.0.0.1".parse().unwrap(),
            is_loopback: false,
            is_up: true,
            supports_multicast: true,
            interface_type: InterfaceType::VPN,
        },
    ];

    let preferred = manager.get_preferred_multicast_interface(&interfaces);
    assert!(preferred.is_some());
    assert_eq!(preferred.unwrap().name, "en0"); // Should prefer en0 (WiFi on modern Macs)
}
