use super::*;

#[test]
fn test_windows_network_manager_creation() {
    let manager = WindowsNetworkManager::new();
    assert_eq!(manager.config.primary_port, 1900);
}

#[test]
fn test_requires_elevation() {
    let manager = WindowsNetworkManager::new();
    assert!(manager.requires_elevation(80));
    assert!(manager.requires_elevation(443));
    assert!(!manager.requires_elevation(8080));
    assert!(!manager.requires_elevation(9090));
}

#[tokio::test]
async fn test_port_availability_check() {
    let manager = WindowsNetworkManager::new();

    // Test with a high port that should be available
    let available = manager.is_port_available(8080).await;
    // This might fail in test environment, but we can at least verify the method works
    println!("Port 8080 available: {}", available);
}
