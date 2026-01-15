//! Build script for terminal_core
//!
//! This script runs during compilation to perform platform-specific setup.

fn main() {
    // Rebuild if build.rs changes
    println!("cargo:rerun-if-changed=build.rs");
    
    // Platform-specific configuration
    #[cfg(target_os = "android")]
    {
        // Android-specific build steps (if any)
        println!("cargo:rustc-cfg=android");
    }
    
    #[cfg(target_os = "ios")]
    {
        // iOS-specific build steps (if any)
        println!("cargo:rustc-cfg=ios");
    }
}
