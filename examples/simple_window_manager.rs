// A very simple reparenting window manager.
// This WM does NOT follow ICCCM!

extern crate x11rb;

use std::collections::HashSet;
use std::process::exit;

use x11rb::connection::Connection;
use x11rb::errors::{ReplyError, ReplyOrIdError};
use x11rb::protocol::xproto::*;
use x11rb::protocol::{Error, Event};
use x11rb::{COPY_DEPTH_FROM_PARENT, CURRENT_TIME};

const TITLEBAR_HEIGHT: u16 = 20;

/// The state of a single window that we manage
#[derive(Debug)]
struct WindowState {
    window: Window,
    frame_window: Window,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
}

impl WindowState {
    fn new(window: Window, frame_window: Window, geom: &GetGeometryReply) -> WindowState {
        WindowState {
            window,
            frame_window,
            x: geom.x,
            y: geom.y,
            width: geom.width,
            height: geom.height,
        }
    }

    fn close_x_position(&self) -> i16 {
        std::cmp::max(0, self.width - TITLEBAR_HEIGHT) as _
    }
}

/// The state of the full WM
#[derive(Debug)]
struct WMState<'a, C: Connection> {
    conn: &'a C,
    screen_num: usize,
    black_gc: Gcontext,
    windows: Vec<WindowState>,
    pending_expose: HashSet<Window>,
    wm_protocols: Atom,
    wm_delete_window: Atom,
}

impl<'a, C: Connection> WMState<'a, C> {
    fn new(conn: &'a C, screen_num: usize) -> Result<WMState<'a, C>, ReplyOrIdError<C::Buf>> {
        let screen = &conn.setup().roots[screen_num];
        let black_gc = conn.generate_id()?;
        let font = conn.generate_id()?;
        conn.open_font(font, b"9x15")?;

        let gc_aux = CreateGCAux::new()
            .graphics_exposures(0)
            .background(screen.white_pixel)
            .foreground(screen.black_pixel)
            .font(font);
        conn.create_gc(black_gc, screen.root, &gc_aux)?;
        conn.close_font(font)?;

        let wm_protocols = conn.intern_atom(false, b"WM_PROTOCOLS")?;
        let wm_delete_window = conn.intern_atom(false, b"WM_DELETE_WINDOW")?;

        Ok(WMState {
            conn,
            screen_num,
            black_gc,
            windows: Vec::default(),
            pending_expose: HashSet::default(),
            wm_protocols: wm_protocols.reply()?.atom,
            wm_delete_window: wm_delete_window.reply()?.atom,
        })
    }

    /// Scan for already existing windows and manage them
    fn scan_windows(&mut self) -> Result<(), ReplyOrIdError<C::Buf>> {
        // Get the already existing top-level windows.
        let screen = &self.conn.setup().roots[self.screen_num];
        let tree_reply = self.conn.query_tree(screen.root)?.reply()?;

        // For each window, request its attributes and geometry *now*
        let mut cookies = Vec::with_capacity(tree_reply.children.len());
        for win in tree_reply.children {
            let attr = self.conn.get_window_attributes(win)?;
            let geom = self.conn.get_geometry(win)?;
            cookies.push((win, attr, geom));
        }
        // Get the replies and manage windows
        for (win, attr, geom) in cookies {
            let (attr, geom) = (attr.reply(), geom.reply());
            if attr.is_err() || geom.is_err() {
                // Just skip this window
                continue;
            }
            let (attr, geom) = (attr.unwrap(), geom.unwrap());
            if !attr.override_redirect && attr.map_state != MapState::Unmapped {
                self.manage_window(win, &geom)?;
            }
        }

        Ok(())
    }

    /// Add a new window that should be managed by the WM
    fn manage_window(
        &mut self,
        win: Window,
        geom: &GetGeometryReply,
    ) -> Result<(), ReplyOrIdError<C::Buf>> {
        println!("Managing window {:?}", win);
        let screen = &self.conn.setup().roots[self.screen_num];
        assert!(self.find_window_by_id(win).is_none());

        let frame_win = self.conn.generate_id()?;
        let win_aux = CreateWindowAux::new()
            .event_mask(
                EventMask::Exposure | EventMask::SubstructureNotify | EventMask::ButtonRelease,
            )
            .background_pixel(screen.white_pixel);
        self.conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            frame_win,
            screen.root,
            geom.x,
            geom.y,
            geom.width,
            geom.height + TITLEBAR_HEIGHT,
            1,
            WindowClass::InputOutput,
            0,
            &win_aux,
        )?;

        self.conn
            .reparent_window(win, frame_win, 0, TITLEBAR_HEIGHT as _)?;
        self.conn.map_window(win)?;
        self.conn.map_window(frame_win)?;

        self.windows.push(WindowState::new(win, frame_win, geom));
        Ok(())
    }

    /// Draw the titlebar of a window
    fn redraw_titlebar(&self, state: &WindowState) -> Result<(), ReplyError<C::Buf>> {
        let close_x = state.close_x_position();
        self.conn.poly_line(
            CoordMode::Origin,
            state.frame_window,
            self.black_gc,
            &[
                Point { x: close_x, y: 0 },
                Point {
                    x: state.width as _,
                    y: TITLEBAR_HEIGHT as _,
                },
            ],
        )?;
        self.conn.poly_line(
            CoordMode::Origin,
            state.frame_window,
            self.black_gc,
            &[
                Point {
                    x: close_x,
                    y: TITLEBAR_HEIGHT as _,
                },
                Point {
                    x: state.width as _,
                    y: 0,
                },
            ],
        )?;
        let reply = self
            .conn
            .get_property(
                false,
                state.window,
                AtomEnum::WM_NAME,
                AtomEnum::STRING,
                0,
                std::u32::MAX,
            )?
            .reply()?;
        self.conn
            .image_text8(state.frame_window, self.black_gc, 1, 10, &reply.value)?;
        Ok(())
    }

    /// Do all pending work that was queued while handling some events
    fn refresh(&mut self) -> Result<(), ReplyError<C::Buf>> {
        while let Some(&win) = self.pending_expose.iter().next() {
            self.pending_expose.remove(&win);
            if let Some(state) = self.find_window_by_id(win) {
                if let Err(err) = self.redraw_titlebar(state) {
                    eprintln!(
                        "Error while redrawing window {:x?}: {:?}",
                        state.window, err
                    );
                }
            }
        }
        Ok(())
    }

    fn find_window_by_id(&self, win: Window) -> Option<&WindowState> {
        self.windows
            .iter()
            .find(|state| state.window == win || state.frame_window == win)
    }

    fn find_window_by_id_mut(&mut self, win: Window) -> Option<&mut WindowState> {
        self.windows
            .iter_mut()
            .find(|state| state.window == win || state.frame_window == win)
    }

    /// Handle the given event
    fn handle_event(&mut self, event: Event<C::Buf>) -> Result<(), ReplyOrIdError<C::Buf>> {
        println!("Got event {:?}", event);
        match event {
            Event::UnmapNotify(event) => self.handle_unmap_notify(event)?,
            Event::ConfigureRequest(event) => self.handle_configure_request(event)?,
            Event::MapRequest(event) => self.handle_map_request(event)?,
            Event::Expose(event) => self.handle_expose(event)?,
            Event::EnterNotify(event) => self.handle_enter(event)?,
            Event::ButtonRelease(event) => self.handle_button_release(event)?,
            _ => {}
        }
        Ok(())
    }

    fn handle_unmap_notify(&mut self, event: UnmapNotifyEvent) -> Result<(), ReplyError<C::Buf>> {
        let conn = self.conn;
        self.windows.retain(|state| {
            if state.window != event.window {
                return true;
            }
            conn.destroy_window(state.frame_window).unwrap();
            false
        });
        Ok(())
    }

    fn handle_configure_request(
        &mut self,
        event: ConfigureRequestEvent,
    ) -> Result<(), ReplyError<C::Buf>> {
        if let Some(state) = self.find_window_by_id_mut(event.window) {
            let _ = state;
            unimplemented!();
        }
        let mut aux = ConfigureWindowAux::default();
        if event.value_mask & u16::from(ConfigWindow::X) != 0 {
            aux = aux.x(i32::from(event.x));
        }
        if event.value_mask & u16::from(ConfigWindow::Y) != 0 {
            aux = aux.y(i32::from(event.y));
        }
        if event.value_mask & u16::from(ConfigWindow::Width) != 0 {
            aux = aux.width(u32::from(event.width));
        }
        if event.value_mask & u16::from(ConfigWindow::Height) != 0 {
            aux = aux.height(u32::from(event.height));
        }
        println!("Configure: {:?}", aux);
        self.conn.configure_window(event.window, &aux)?;
        Ok(())
    }

    fn handle_map_request(&mut self, event: MapRequestEvent) -> Result<(), ReplyOrIdError<C::Buf>> {
        self.manage_window(
            event.window,
            &self.conn.get_geometry(event.window)?.reply()?,
        )
    }

    fn handle_expose(&mut self, event: ExposeEvent) -> Result<(), ReplyError<C::Buf>> {
        self.pending_expose.insert(event.window);
        Ok(())
    }

    fn handle_enter(&mut self, event: EnterNotifyEvent) -> Result<(), ReplyError<C::Buf>> {
        let window = if let Some(state) = self.find_window_by_id(event.child) {
            state.window
        } else {
            event.event
        };
        self.conn
            .set_input_focus(InputFocus::Parent, window, CURRENT_TIME)?;
        Ok(())
    }

    fn handle_button_release(
        &mut self,
        event: ButtonReleaseEvent,
    ) -> Result<(), ReplyError<C::Buf>> {
        if let Some(state) = self.find_window_by_id(event.event) {
            let data = [self.wm_delete_window, 0, 0, 0, 0];
            let event = ClientMessageEvent {
                response_type: CLIENT_MESSAGE_EVENT,
                format: 32,
                sequence: 0,
                window: state.window,
                type_: self.wm_protocols,
                data: data.into(),
            };
            self.conn
                .send_event(false, state.window, EventMask::NoEvent, &event)?;
        }
        Ok(())
    }
}

fn become_wm<C: Connection>(conn: &C, screen: &Screen) -> Result<(), ReplyError<C::Buf>> {
    // Try to become the window manager. This causes an error if there is already another WM.
    let change = ChangeWindowAttributesAux::default().event_mask(
        EventMask::SubstructureRedirect | EventMask::SubstructureNotify | EventMask::EnterWindow,
    );
    let error = conn
        .change_window_attributes(screen.root, &change)?
        .check()?;
    if let Some(error) = error {
        match error {
            Error::Access(_) => {
                eprintln!("Another WM is already running.");
                exit(1);
            }
            error => Err(ReplyError::X11Error(error)),
        }
    } else {
        Ok(())
    }
}

fn main() {
    let (conn, screen_num) = x11rb::connect(None).unwrap();

    // The following is only needed for start_timeout_thread(), which is used for 'tests'
    let conn1 = std::sync::Arc::new(conn);
    let conn = &*conn1;

    let screen = &conn.setup().roots[screen_num];

    become_wm(conn, screen).unwrap();

    let mut wm_state = WMState::new(conn, screen_num).unwrap();
    wm_state.scan_windows().unwrap();

    util::start_timeout_thread(conn1.clone(), screen.root);

    loop {
        wm_state.refresh().unwrap();
        conn.flush().unwrap();

        let event = conn.wait_for_event().unwrap();
        let mut event_option = Some(event);
        while let Some(event) = event_option {
            if let Event::ClientMessage(_) = event {
                // This is start_timeout_thread() signaling us to close (most likely).
                return;
            }

            wm_state.handle_event(event).unwrap();
            event_option = conn.poll_for_event().unwrap();
        }
    }
}

include!("integration_test_util/util.rs");
