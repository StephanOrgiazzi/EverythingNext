mod app;
mod backend;
mod diagnostics;
mod window;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
