use crate::scripting::guiwin::GuiWin;
use crate::TermWindow;
use ::window::*;
use anyhow::Context;
use config::NotificationHandling;
use mux::window::WindowId as MuxWindowId;
use mux::{Mux, MuxNotification};
use promise::{Future, Promise};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;
use wezterm_term::{Alert, ClipboardSelection};
use wezterm_toast_notification::*;

#[cfg_attr(not(windows), allow(dead_code))]
pub struct GuiFrontEnd {
    connection: Rc<Connection>,
    spawned_mux_window: RefCell<HashSet<MuxWindowId>>,
    known_windows: RefCell<BTreeMap<Window, MuxWindowId>>,
    single_session_workspace: String,
}

impl Drop for GuiFrontEnd {
    fn drop(&mut self) {
        ::window::shutdown();
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
impl GuiFrontEnd {
    pub fn try_new() -> anyhow::Result<Rc<GuiFrontEnd>> {
        let connection = Connection::init()?;
        let mux = Mux::get();
        let client_id = mux.active_identity().expect("to have set my own id");
        let single_session_workspace = mux.active_workspace();

        let front_end = Rc::new(GuiFrontEnd {
            connection,
            spawned_mux_window: RefCell::new(HashSet::new()),
            known_windows: RefCell::new(BTreeMap::new()),
            single_session_workspace,
        });

        mux.subscribe(move |n| {
            match n {
                MuxNotification::WindowCreated(_) | MuxNotification::WindowRemoved(_) => {
                    promise::spawn::spawn_into_main_thread(async move {
                        let fe = crate::frontend::front_end();
                        fe.reconcile_workspace();
                    })
                    .detach();
                }
                MuxNotification::PaneFocused(pane_id) => {
                    promise::spawn::spawn_into_main_thread(async move {
                        let mux = Mux::get();
                        if let Err(err) = mux.focus_pane_and_containing_tab(pane_id) {
                            log::error!("Error reconciling PaneFocused notification: {err:#}");
                        }
                    })
                    .detach();
                }
                MuxNotification::TabTitleChanged { .. } => {}
                MuxNotification::WindowTitleChanged { .. } => {}
                MuxNotification::TabResized(_) => {}
                MuxNotification::TabAddedToWindow { .. } => {}
                MuxNotification::PaneRemoved(_) => {}
                MuxNotification::WindowInvalidated(_) => {}
                MuxNotification::PaneOutput(_) => {}
                MuxNotification::PaneAdded(_) => {}
                MuxNotification::Alert {
                    pane_id,
                    alert:
                        Alert::ToastNotification {
                            title,
                            body,
                            focus: _,
                        },
                } => {
                    let mux = Mux::get();

                    if let Some((_domain, window_id, tab_id)) = mux.resolve_pane_id(pane_id) {
                        let config = config::configuration();

                        if let Some((_fdomain, f_window, f_tab, f_pane)) =
                            mux.resolve_focused_pane(&client_id)
                        {
                            let show = match config.notification_handling {
                                NotificationHandling::NeverShow => false,
                                NotificationHandling::AlwaysShow => true,
                                NotificationHandling::SuppressFromFocusedPane => f_pane != pane_id,
                                NotificationHandling::SuppressFromFocusedTab => f_tab != tab_id,
                                NotificationHandling::SuppressFromFocusedWindow => {
                                    f_window != window_id
                                }
                            };

                            if show {
                                let message = if title.is_none() { "" } else { &body };
                                let title = title.as_ref().unwrap_or(&body);
                                // FIXME: if notification.focus is true, we should do
                                // something here to arrange to focus pane_id when the
                                // notification is clicked
                                persistent_toast_notification(title, message);
                            }
                        }
                    }
                }
                MuxNotification::Alert {
                    pane_id: _,
                    alert: Alert::Bell | Alert::Progress(_),
                } => {
                    // Handled via TermWindowNotif; NOP it here.
                }
                MuxNotification::Alert {
                    pane_id: _,
                    alert:
                        Alert::OutputSinceFocusLost
                        | Alert::PaletteChanged
                        | Alert::CurrentWorkingDirectoryChanged
                        | Alert::WindowTitleChanged(_)
                        | Alert::TabTitleChanged(_)
                        | Alert::IconTitleChanged(_)
                        | Alert::SetUserVar { .. },
                } => {}
                MuxNotification::Empty => {
                    // There is no listener or launcher that can revive this
                    // process after its only session exits. Ignore the legacy
                    // keep-GUI-alive preference, but protect in-flight startup.
                    promise::spawn::spawn_into_main_thread(async move {
                        if mux::activity::Activity::count() == 0 && Mux::get().is_empty() {
                            log::trace!("Single WSL session ended, terminate gui");
                            Connection::get().unwrap().terminate_message_loop();
                        }
                    })
                    .detach();
                }
                MuxNotification::AssignClipboard {
                    pane_id,
                    selection,
                    clipboard,
                } => {
                    promise::spawn::spawn_into_main_thread(async move {
                        let fe = crate::frontend::front_end();
                        log::trace!(
                            "set clipboard in pane {} {:?} {:?}",
                            pane_id,
                            selection,
                            clipboard
                        );
                        if let Some(window) = fe.known_windows.borrow().keys().next() {
                            window.set_clipboard(
                                match selection {
                                    ClipboardSelection::Clipboard => Clipboard::Clipboard,
                                    ClipboardSelection::PrimarySelection => {
                                        Clipboard::PrimarySelection
                                    }
                                },
                                clipboard.unwrap_or_else(String::new),
                            );
                        } else {
                            log::error!("Cannot assign clipboard as there are no windows");
                        };
                    })
                    .detach();
                }
            }
            true
        });
        Ok(front_end)
    }

    pub fn run_forever(&self) -> anyhow::Result<()> {
        self.connection
            .run_message_loop()
            .context("running message loop")
    }

    pub fn gui_windows(&self) -> Vec<GuiWin> {
        let windows = self.known_windows.borrow();
        let mut windows: Vec<GuiWin> = windows
            .iter()
            .map(|(window, &mux_window_id)| GuiWin {
                mux_window_id,
                window: window.clone(),
            })
            .collect();
        windows.sort_by(|a, b| a.window.cmp(&b.window));
        windows
    }

    pub fn reconcile_workspace(&self) -> Future<()> {
        let mut promise = Promise::new();
        let mux = Mux::get();
        let workspace = self.single_session_workspace.clone();
        log::debug!("workspace is {}, fixup windows", workspace);

        let mut mux_windows = mux.iter_windows_in_workspace(&workspace);
        if mux_windows.len() > 1 {
            log::warn!(
                "single-session GUI found {} mux windows; displaying only the first",
                mux_windows.len()
            );
            mux_windows.truncate(1);
        }

        // Keep the session's existing native window; close stale windows even
        // when the session list is empty. Never repurpose a window for a session.
        let known_windows = std::mem::take(&mut *self.known_windows.borrow_mut());
        let mut windows = BTreeMap::new();

        for (window, window_id) in known_windows.into_iter() {
            if let Some(idx) = mux_windows.iter().position(|&id| id == window_id) {
                // it already points to the desired mux window
                windows.insert(window, window_id);
                mux_windows.remove(idx);
            } else {
                window.close();
                self.spawned_mux_window.borrow_mut().remove(&window_id);
            }
        }

        let mut mux_windows = mux_windows.into_iter();

        log::trace!("reconcile: windows -> {:?}", windows);
        *self.known_windows.borrow_mut() = windows;

        let future = promise.get_future().unwrap();

        // then spawn any new windows that are needed
        promise::spawn::spawn(async move {
            while let Some(mux_window_id) = mux_windows.next() {
                if front_end().has_mux_window(mux_window_id)
                    || front_end()
                        .spawned_mux_window
                        .borrow()
                        .contains(&mux_window_id)
                {
                    continue;
                }
                front_end()
                    .spawned_mux_window
                    .borrow_mut()
                    .insert(mux_window_id);
                log::trace!("Creating TermWindow for mux_window_id={}", mux_window_id);
                if let Err(err) = TermWindow::new_window(mux_window_id).await {
                    log::error!("Failed to create window: {:#}", err);
                    let mux = Mux::get();
                    mux.kill_window(mux_window_id);
                    front_end()
                        .spawned_mux_window
                        .borrow_mut()
                        .remove(&mux_window_id);
                }
            }
            promise.ok(());
        })
        .detach();
        future
    }

    fn has_mux_window(&self, mux_window_id: MuxWindowId) -> bool {
        for &mux_id in self.known_windows.borrow().values() {
            if mux_id == mux_window_id {
                return true;
            }
        }
        false
    }

    pub fn record_known_window(&self, window: Window, mux_window_id: MuxWindowId) {
        self.known_windows
            .borrow_mut()
            .insert(window, mux_window_id);
        self.reconcile_workspace();
    }

    pub fn forget_known_window(&self, window: &Window) {
        self.known_windows.borrow_mut().remove(window);
        self.reconcile_workspace();
    }

    pub fn gui_window_for_mux_window(&self, mux_window_id: MuxWindowId) -> Option<GuiWin> {
        let windows = self.known_windows.borrow();
        for (window, v) in windows.iter() {
            if *v == mux_window_id {
                return Some(GuiWin {
                    mux_window_id,
                    window: window.clone(),
                });
            }
        }
        None
    }
}

thread_local! {
    static FRONT_END: RefCell<Option<Rc<GuiFrontEnd>>> = RefCell::new(None);
}

pub fn try_front_end() -> Option<Rc<GuiFrontEnd>> {
    FRONT_END.with(|f| f.borrow().as_ref().map(Rc::clone))
}

pub fn front_end() -> Rc<GuiFrontEnd> {
    FRONT_END
        .with(|f| f.borrow().as_ref().map(Rc::clone))
        .expect("to be called on gui thread")
}

pub fn shutdown() {
    FRONT_END.with(|f| drop(f.borrow_mut().take()));
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn try_new() -> anyhow::Result<Rc<GuiFrontEnd>> {
    let front_end = GuiFrontEnd::try_new()?;
    FRONT_END.with(|f| *f.borrow_mut() = Some(Rc::clone(&front_end)));
    Ok(front_end)
}
