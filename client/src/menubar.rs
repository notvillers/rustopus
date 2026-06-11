//! "Minimize to menu bar / tray" support.
//!
//! On macOS, hiding puts an octopus status-bar icon in the menu bar with a
//! Show/Quit menu, hides the window, and removes the Dock icon (activation
//! policy Accessory) until shown again. On Windows the icon goes to the
//! system tray (right-click for the menu, left-click to show the window)
//! and the window disappears from the taskbar while hidden. Elsewhere it
//! falls back to a plain window minimize.

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod imp {
    use std::sync::mpsc;

    use tray_icon::{
        TrayIcon, TrayIconBuilder,
        menu::{Menu, MenuEvent, MenuId, MenuItem},
    };
    #[cfg(target_os = "windows")]
    use tray_icon::TrayIconEvent;

    enum Event {
        Menu(MenuEvent),
        // Tray-icon clicks only matter on Windows; on macOS clicking the
        // status item opens the menu instead.
        #[cfg(target_os = "windows")]
        Tray(TrayIconEvent),
    }

    pub struct MenuBar {
        tray: Option<TrayIcon>,
        rx: mpsc::Receiver<Event>,
        // muda's set_event_handler is backed by a OnceCell and only takes
        // effect on the first call, so the channel must live for the whole
        // app; `tx` is taken when the handlers are installed.
        tx: Option<mpsc::Sender<Event>>,
        show_id: Option<MenuId>,
        quit_id: Option<MenuId>,
        quitting: bool,
    }

    impl MenuBar {
        pub fn new() -> Self {
            let (tx, rx) = mpsc::channel();
            Self {
                tray: None,
                rx,
                tx: Some(tx),
                show_id: None,
                quit_id: None,
                quitting: false,
            }
        }

        /// While menu-bar mode is on, the window's close button hides to the
        /// menu bar / tray instead of quitting; only the tray's Quit exits.
        pub fn intercept_close(&mut self, ctx: &egui::Context) {
            if self.quitting {
                return;
            }
            if ctx.input(|i| i.viewport().close_requested()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.hide(ctx);
            }
        }

        /// Hide the window into the menu bar / system tray.
        pub fn hide(&mut self, ctx: &egui::Context) {
            if self.tray.is_some() {
                return;
            }

            let menu = Menu::new();
            let show_item = MenuItem::new("Show Rustopus Client", true, None);
            let quit_item = MenuItem::new("Quit Rustopus Client", true, None);
            if menu.append_items(&[&show_item, &quit_item]).is_err() {
                return;
            }

            let Some(icon) = octopus_icon() else { return };
            let tray = match TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_icon(icon)
                .with_tooltip("Rustopus Client")
                .build()
            {
                Ok(tray) => tray,
                Err(_) => return,
            };

            // Menu/tray events arrive outside the frame loop; forward them
            // into the channel and wake egui so `poll` runs even while
            // hidden. Installed once for the app's lifetime (see `tx`).
            if let Some(tx) = self.tx.take() {
                #[cfg(target_os = "windows")]
                {
                    let tray_tx = tx.clone();
                    let tray_ctx = ctx.clone();
                    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
                        let _ = tray_tx.send(Event::Tray(event));
                        tray_ctx.request_repaint();
                    }));
                }

                let menu_ctx = ctx.clone();
                MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                    let _ = tx.send(Event::Menu(event));
                    menu_ctx.request_repaint();
                }));
            }

            self.show_id = Some(show_item.id().clone());
            self.quit_id = Some(quit_item.id().clone());
            self.tray = Some(tray);

            #[cfg(target_os = "macos")]
            set_dock_visible(false);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        /// Handle menu-bar / tray events; call once per frame.
        pub fn poll(&mut self, ctx: &egui::Context) {
            let mut show = false;
            let mut quit = false;
            while let Ok(event) = self.rx.try_recv() {
                match event {
                    Event::Menu(event) => {
                        if Some(event.id()) == self.show_id.as_ref() {
                            show = true;
                        } else if Some(event.id()) == self.quit_id.as_ref() {
                            quit = true;
                        }
                    }
                    // A left-click on the tray icon shows the window
                    // (the menu is on right-click).
                    #[cfg(target_os = "windows")]
                    Event::Tray(TrayIconEvent::Click {
                        button: tray_icon::MouseButton::Left,
                        button_state: tray_icon::MouseButtonState::Up,
                        ..
                    }) => show = true,
                    #[cfg(target_os = "windows")]
                    Event::Tray(_) => {}
                }
            }

            if quit {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if show {
                self.show(ctx);
            }
        }

        fn show(&mut self, ctx: &egui::Context) {
            self.tray = None; // dropping the TrayIcon removes it from the menu bar / tray

            #[cfg(target_os = "macos")]
            set_dock_visible(true);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    #[cfg(target_os = "macos")]
    fn set_dock_visible(visible: bool) {
        use objc2::AnyThread;
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSImage};
        use objc2_foundation::NSData;

        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            app.setActivationPolicy(if visible {
                NSApplicationActivationPolicy::Regular
            } else {
                NSApplicationActivationPolicy::Accessory
            });

            // Returning from Accessory to Regular leaves a generic "exec"
            // Dock icon; restore the octopus explicitly.
            if visible {
                let data = NSData::with_bytes(include_bytes!("assets/images/octopus.png"));
                if let Some(img) = NSImage::initWithData(NSImage::alloc(), &data) {
                    // SAFETY: called on the main thread with a valid NSImage.
                    unsafe { app.setApplicationIconImage(Some(&img)) };
                }
            }
        }
    }

    fn octopus_icon() -> Option<tray_icon::Icon> {
        let bytes = include_bytes!("assets/images/octopus.png");
        let img = image::load_from_memory(bytes).ok()?.into_rgba8();
        let (w, h) = img.dimensions();
        tray_icon::Icon::from_rgba(img.into_raw(), w, h).ok()
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod imp {
    pub struct MenuBar;

    impl MenuBar {
        pub fn new() -> Self {
            Self
        }

        pub fn hide(&mut self, ctx: &egui::Context) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }

        pub fn poll(&mut self, _ctx: &egui::Context) {}

        // No tray on this platform, so there is no alternative quit path —
        // the close button must keep closing.
        pub fn intercept_close(&mut self, _ctx: &egui::Context) {}
    }
}

pub use imp::MenuBar;
