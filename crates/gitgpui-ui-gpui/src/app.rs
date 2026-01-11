use crate::view::GitGpuiView;
use gitgpui_core::services::GitBackend;
use gitgpui_state::store::AppStore;
use gpui::{
    App, AppContext, Application, Bounds, TitlebarOptions, WindowBounds, WindowDecorations,
    WindowOptions, point, px, size,
};
use std::sync::Arc;

pub fn run(backend: Arc<dyn GitBackend>) {
    let initial_path = std::env::args_os().nth(1).map(std::path::PathBuf::from);

    Application::new().run(move |cx: &mut App| {
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1100.0), px(720.0)), cx);
        let backend = Arc::clone(&backend);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("GitGpui".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(9.0), px(9.0))),
                }),
                window_decorations: Some(WindowDecorations::Client),
                is_movable: true,
                is_resizable: true,
                ..Default::default()
            },
            move |window, cx| {
                let (store, events) = AppStore::new(Arc::clone(&backend));
                cx.new(|cx| GitGpuiView::new(store, events, initial_path.clone(), window, cx))
            },
        )
        .unwrap();

        cx.activate(true);
    });
}
