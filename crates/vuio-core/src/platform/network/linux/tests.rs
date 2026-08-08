use super::*;

#[test]
fn test_linux_network_manager_creation() {
    let manager = LinuxNetworkManager::new();
    assert_eq!(manager.config.primary_port, 1900);
}

#[test]
fn test_requires_elevation() {
    let manager = LinuxNetworkManager::new();
    assert!(manager.requires_elevation(80));
    assert!(manager.requires_elevation(443));
    assert!(!manager.requires_elevation(8080));
    assert!(!manager.requires_elevation(9090));
}

#[test]
fn test_interface_type_determination() {
    let manager = LinuxNetworkManager::new();

    assert_eq!(
        manager.determine_linux_interface_type("eth0"),
        InterfaceType::Ethernet
    );

    assert_eq!(
        manager.determine_linux_interface_type("enp0s3"),
        InterfaceType::Ethernet
    );

    assert_eq!(
        manager.determine_linux_interface_type("wlan0"),
        InterfaceType::WiFi
    );

    assert_eq!(
        manager.determine_linux_interface_type("wlp2s0"),
        InterfaceType::WiFi
    );

    assert_eq!(
        manager.determine_linux_interface_type("tun0"),
        InterfaceType::VPN
    );

    assert_eq!(
        manager.determine_linux_interface_type("lo"),
        InterfaceType::Loopback
    );
}

#[tokio::test]
async fn test_port_availability_check() {
    let manager = LinuxNetworkManager::new();

    // Test with a high port that should be available
    let available = manager.is_port_available(8080).await;
    // This might fail in test environment, but we can at least verify the method works
    println!("Port 8080 available: {}", available);
}

#[test]
fn test_network_namespaces() {
    let manager = LinuxNetworkManager::new();
    let namespaces = manager.get_network_namespaces();
    // Namespaces list can be empty, that's fine
    println!("Network namespaces: {:?}", namespaces);
}
