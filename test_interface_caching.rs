// Simple test to verify interface caching is working
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    println!("Testing network interface caching...");
    
    // Create a Windows network manager
    #[cfg(target_os = "windows")]
    {
        use vuio::platform::network::{NetworkManager, windows::WindowsNetworkManager};
        
        let manager = WindowsNetworkManager::new();
        
        println!("First call to get_local_interfaces (should detect interfaces):");
        let start = Instant::now();
        let interfaces1 = manager.get_local_interfaces().await?;
        let duration1 = start.elapsed();
        println!("  Found {} interfaces in {:?}", interfaces1.len(), duration1);
        
        println!("Second call to get_local_interfaces (should use cache):");
        let start = Instant::now();
        let interfaces2 = manager.get_local_interfaces().await?;
        let duration2 = start.elapsed();
        println!("  Found {} interfaces in {:?}", interfaces2.len(), duration2);
        
        println!("Third call to get_local_interfaces (should use cache):");
        let start = Instant::now();
        let interfaces3 = manager.get_local_interfaces().await?;
        let duration3 = start.elapsed();
        println!("  Found {} interfaces in {:?}", interfaces3.len(), duration3);
        
        // The cached calls should be much faster
        if duration2 < duration1 / 2 && duration3 < duration1 / 2 {
            println!("✅ Caching is working! Subsequent calls are much faster.");
        } else {
            println!("⚠️  Caching may not be working as expected.");
        }
        
        // Verify the results are the same
        if interfaces1.len() == interfaces2.len() && interfaces2.len() == interfaces3.len() {
            println!("✅ Interface count is consistent across calls.");
        } else {
            println!("⚠️  Interface count is inconsistent.");
        }
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        println!("This test is designed for Windows. Skipping on other platforms.");
    }
    
    Ok(())
}