#[allow(clippy::module_inception)]
pub mod renderer;
pub mod software;

#[allow(unused_imports)]
pub use renderer::Renderer;
#[allow(unused_imports)]
pub use software::CpuRenderer;
