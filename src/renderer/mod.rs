pub mod backend;
#[allow(clippy::module_inception)]
pub mod renderer;
pub mod software;
pub mod state;

pub use backend::{GlDisplayManager, GlInfo, RendererInitError, create_window_and_renderer};
pub use state::{DirtyRowTracker, PaneRenderState, RowRenderCache};
