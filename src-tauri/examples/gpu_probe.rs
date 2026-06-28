fn main() {
    #[cfg(target_os = "macos")]
    {
        use cardyn_lib::gpu::{macos::MacGpu, GpuProvider};
        let mut gpu = MacGpu;
        println!("{:?}", gpu.sample());
    }
    #[cfg(not(target_os = "macos"))]
    println!("MacGpu only on macOS");
}
