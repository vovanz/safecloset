use {
    super::*,
    crate::{
        cli::Args,
        core::*,
        error::SafeClosetError,
    },
    crossterm::{self, event::KeyEvent},
};

/// TUI Application state, containing a drawer state.
///
/// Needs a closet
pub struct AppState {
    pub open_closet: OpenCloset,
    pub drawer_state: DrawerState,
    // the help state, if help is currently displayed
    pub help: Option<HelpState>,
    pub message: Option<Message>,
    pub hide_values: bool,
    // number of drawers created during this session
    pub created_drawers: usize,
}

impl AppState {

    pub fn new(open_closet: OpenCloset, args: &Args) -> Self {
        let mut state = Self {
            open_closet,
            drawer_state: DrawerState::NoneOpen,
            help: None,
            message: None,
            hide_values: args.hide,
            created_drawers: 0,
        };
        if args.open && !state.open_closet.just_created() {
            state.drawer_state = DrawerState::DrawerOpening(PasswordInputState::new(true));
        }
        state
    }

    fn set_error<S: Into<String>>(&mut self, error: S) {
        let text = error.into();
        warn!("error: {:?}", &text);
        self.message = Some(Message{ text, error: true });
    }
    #[allow(dead_code)]
    fn set_info<S: Into<String>>(&mut self, info: S) {
        let text = info.into();
        debug!("info: {:?}", &text);
        self.message = Some(Message{ text, error: false });
    }


    /// If there's an open drawer input (entry name or value), close it, keeping
    /// the input content if required.
    ///
    /// Return true if there was such input
    fn close_drawer_input(&mut self, discard: bool) -> bool {
        if let DrawerState::DrawerEdit(des) = &mut self.drawer_state {
            des.close_input(discard)
        } else {
            false
        }
    }

    /// Save the content of the edited cell if any, then save the whole closet
    fn save(&mut self, reopen_if_open: bool) -> Result<(), SafeClosetError> {
        time!(self.close_drawer_input(false));
        let drawer_state = std::mem::take(&mut self.drawer_state);
        if let DrawerState::DrawerEdit(mut des) = drawer_state {
            if reopen_if_open {
                self.drawer_state = DrawerState::DrawerEdit(
                    time!(des.save_and_reopen(&mut self.open_closet)?)
                );
            } else {
                des.drawer.content.remove_empty_entries();
                time!(self.open_closet.push_back(des.drawer)?);
                time!(self.open_closet.close_and_save())?;
            }
        }
        Ok(())
    }

    /// Handle an event asking for copying from SafeCloset
    pub fn copy(&mut self) {
        #[cfg(not(feature = "clipboard"))]
        {
            self.set_error("Clipboard feature not enabled at compilation");
        }
        #[cfg(feature = "clipboard")]
        {
            if let DrawerState::DrawerEdit(des) = &self.drawer_state {
                if let Some(cell) = des.current_cell() {
                    if let Err(e) = terminal_clipboard::set_string(cell) {
                        self.set_error(e.to_string());
                    } else {
                        self.set_info("cell copied in the clipboard, be cautious");
                    }
                } else {
                    self.set_error("you can only copy from a selected name or value");
                }
            } else {
                self.set_error("you can only copy from an open drawer");
            }
        }
    }

    /// Handle an event asking for pasting into SafeCloset
    pub fn paste(&mut self) {
        #[cfg(not(feature = "clipboard"))]
        {
            self.set_error("Clipboard feature not enabled at compilation");
        }
        #[cfg(feature = "clipboard")]
        {
            use DrawerFocus::*;
            match terminal_clipboard::get_string() {
                Ok(mut pasted) if !pasted.is_empty() => {
                    if !self.drawer_state.is_on_entry_value() {
                        pasted.truncate(pasted.lines().next().unwrap().len());
                    }
                    if let Some(input) = self.drawer_state.input() {
                        input.insert_str(pasted);
                    } else if let DrawerState::DrawerEdit(des) = &mut self.drawer_state {
                        if let NameSelected { line } = &mut des.focus {
                            let line = *line;
                            if des.edit_entry_name_by_line(line, EditionPos::Start) {
                                if let Some(input) = self.drawer_state.input() {
                                    input.set_str(pasted);
                                    input.move_to_end();
                                    self.set_info("Hit *esc* to cancel pasting");
                                } else {
                                    warn!("unexpected lack of input");
                                }
                            }
                        } else if let ValueSelected { line } = &mut des.focus {
                            let line = *line;
                            if des.edit_entry_value_by_line(line, EditionPos::Start) {
                                if let Some(input) = self.drawer_state.input() {
                                    input.set_str(pasted);
                                    input.move_to_end();
                                    self.set_info("Hit *esc* to cancel pasting");
                                } else {
                                    warn!("unexpected lack of input");
                                }
                            }
                        }
                    }
                }
                _ => {
                    self.set_error("nothing to paste");
                }
            }
        }
    }

    /// Handle a click event
    pub fn on_click(&mut self, x: u16, y: u16)-> Result<(), SafeClosetError> {

        // TODO handle click in search input location

        if let Some(input) = self.drawer_state.input() {
            if input.apply_click_event(x, y) {
                return Ok(());
            } else if let DrawerState::DrawerEdit(des) = &mut self.drawer_state {
                // unfocusing the input, validating it
                des.focus = DrawerFocus::NoneSelected;
            }
        }

        if let DrawerState::DrawerEdit(des) = &mut self.drawer_state {
            if let Some(clicked_line) = des.clicked_line(y) {
                use DrawerFocus::*;
                let in_name = des.layout().is_in_name_column(x);
                des.focus = if in_name {
                    NameSelected { line: clicked_line }
                } else {
                    ValueSelected { line: clicked_line }
                };
            }
        }

        Ok(())
    }

    /// Handle a mouse wheel event
    pub fn on_mouse_wheel(&mut self, amount: i32) {
        if let DrawerState::DrawerEdit(des) = &mut self.drawer_state {
            des.move_line(
                if amount < 0 { Direction::Up } else { Direction::Down }
            );
        }
    }

    /// push back the open drawer, if any, and set the drawer_state to NoneOpen
    fn push_back_drawer(&mut self) -> Result<(), SafeClosetError> {
        self.close_drawer_input(true);
        // if there's an edited drawer, we push it back to the closet
        let drawer_state = std::mem::take(&mut self.drawer_state);
        if let DrawerState::DrawerEdit(DrawerEditState { drawer, .. }) = drawer_state {
            self.open_closet.push_back(drawer)?;
        }
        Ok(())
    }


    /// Handle a key event
    pub fn on_key(&mut self, key: KeyEvent) -> Result<CmdResult, SafeClosetError> {
        use {
            DrawerFocus::*,
            DrawerState::*,
        };
        self.message = None;

        // We start with the few actions that can be done the same with or
        // without the help screen displayed

        if key == CONTROL_N { // new drawer
            self.push_back_drawer()?;
            self.drawer_state = DrawerCreation(PasswordInputState::new(false));
            return Ok(CmdResult::Stay);
        }

        if key == CONTROL_O { // open drawer
            self.help = None;
            self.push_back_drawer()?;
            self.drawer_state = DrawerOpening(PasswordInputState::new(true));
            return Ok(CmdResult::Stay);
        }

        if key == CONTROL_Q {
            debug!("user requests quit");
            return Ok(CmdResult::Quit);
        }

        if key == CONTROL_S {
            debug!("user requests save, keep state");
            self.save(true)?;
            return Ok(CmdResult::Stay);
        }

        // if key == CONTROL_X {
        //     debug!("user requests save and quit");
        //     self.save(false)?;
        //     return Ok(CmdResult::Quit);
        // }

        if key == CONTROL_U { // up the drawer chain, close the current one
            if self.help.is_some() {
                // close the help
                self.help = None;
            } else {
                // close the drawer
                self.save(true)?;
                self.push_back_drawer()?;
                let _ = self.open_closet.close_deepest_drawer();
                self.drawer_state = match self.open_closet.take_deepest_open_drawer() {
                    Some(open_drawer) => DrawerState::edit(open_drawer),
                    None => DrawerState::NoneOpen,
                };
            }
            return Ok(CmdResult::Stay);
        }

        if key == CONTROL_C {
            self.copy();
            return Ok(CmdResult::Stay);
        }

        if key == CONTROL_V {
            self.paste();
            return Ok(CmdResult::Stay);
        }

        if key == ESC {
            if self.help.is_some() {
                self.help = None;
            } else if matches!(self.drawer_state, DrawerCreation(_) | DrawerOpening(_)) {
                self.drawer_state = NoneOpen;
            } else if let DrawerEdit(des) = &mut self.drawer_state {
                if !des.close_input(true) {
                    des.focus = NoneSelected;
                }
            }
            return Ok(CmdResult::Stay);
        }

        // If the help is shown, it captures other events
        if let Some(help_state) = &mut self.help {
            help_state.apply_key_event(key);
            return Ok(CmdResult::Stay);
        }

        if key == CONTROL_UP { // moving the selected line up
            if let DrawerEdit(des) = &mut self.drawer_state {
                let entries = &mut des.drawer.content.entries;
                let len = entries.len();
                match &mut des.focus {
                    NameSelected { line } => {
                        let new_line = (*line + len - 1) % len;
                        entries.swap(*line, new_line);
                        des.focus = NameSelected { line: new_line };
                    }
                    ValueSelected { line } => {
                        let new_line = (*line + len - 1) % len;
                        entries.swap(*line, new_line);
                        des.focus = ValueSelected { line: new_line };
                    }
                    ValueEdit { input, .. }  => {
                        input.move_current_line_up();
                    }
                    _ => {}
                }
                des.update_search();
            }
            return Ok(CmdResult::Stay);
        }
        if key == CONTROL_DOWN { // moving the selected line down
            if let DrawerEdit(des) = &mut self.drawer_state {
                let entries = &mut des.drawer.content.entries;
                let len = entries.len();
                match &mut des.focus {
                    NameSelected { line } => {
                        let new_line = (*line + 1) % len;
                        entries.swap(*line, new_line);
                        des.focus = NameSelected { line: new_line };
                    }
                    ValueSelected { line } => {
                        let new_line = (*line + 1) % len;
                        entries.swap(*line, new_line);
                        des.focus = ValueSelected { line: new_line };
                    }
                    ValueEdit { input, .. }  => {
                        input.move_current_line_down();
                    }
                    _ => {}
                }
                des.update_search();
            }
            return Ok(CmdResult::Stay);
        }

        if let DrawerEdit(des) = &mut self.drawer_state {
            // -- pending removal
            if let PendingRemoval { line } = &des.focus {
                let line = *line;
                if let Some(idx) = des.listed_entry_idx(line) {
                    // we either confirm (delete) or cancel removal
                    if as_letter(key) == Some('y') {
                        info!("user requests entry removal");
                        des.drawer.content.entries.remove(idx);
                        des.focus = if line > 0 {
                            NameSelected { line: line - 1 }
                        } else {
                            NoneSelected
                        };
                        des.update_search();
                    } else {
                        info!("user cancels entry removal");
                        des.focus = NameSelected { line };
                    }
                }
                return Ok(CmdResult::Stay);
            }
        }

        // -- toggle visibility of password or values

        if key == CONTROL_H {
            if let DrawerCreation(pis) | DrawerOpening(pis) = &mut self.drawer_state {
                pis.input.password_mode ^= true;
                return Ok(CmdResult::Stay);
            }
            if let DrawerEdit(des) = &mut self.drawer_state {
                des.drawer.content.settings.hide_values ^= true;
                return Ok(CmdResult::Stay);
            }
        }

        if key == ENTER {
            self.close_drawer_input(false); // if there's an entry input
            if let DrawerCreation(PasswordInputState { input }) = &mut self.drawer_state {
                let pwd = input.get_content();
                let open_drawer = time!(self.open_closet.create_take_drawer(&pwd));
                match open_drawer {
                    Ok(open_drawer) => {
                        self.drawer_state = DrawerState::edit(open_drawer);
                        self.created_drawers += 1;
                    }
                    Err(e) => {
                        self.set_error(e.to_string());
                    }
                }
            } else if let DrawerOpening(PasswordInputState { input }) = &mut self.drawer_state {
                let pwd = input.get_content();
                let open_drawer = self.open_closet.open_take_drawer(&pwd);
                match open_drawer {
                    Some(mut open_drawer) => {
                        if self.hide_values {
                            open_drawer.content.settings.hide_values = true;
                        }
                        self.drawer_state = DrawerState::edit(open_drawer);
                    }
                    None => {
                        self.set_error("This passphrase opens no drawer");
                    }
                }
            }
            return Ok(CmdResult::Stay);
        }

        if key == TAB {
            if let DrawerEdit(des) = &mut self.drawer_state {
                if matches!(des.focus, NoneSelected) {
                    // we remove any search
                    des.search.clear();
                    let idx = des.drawer.content.empty_entry();
                    des.edit_entry_name_by_line(idx, EditionPos::Start); // as there's no filtering, idx==line
                } else if let NameSelected { line } = &des.focus {
                    let line = *line;
                    des.edit_entry_value_by_line(line, EditionPos::Start);
                } else if let NameEdit { line, .. } = &des.focus {
                    let line = *line;
                    des.close_input(false);
                    des.edit_entry_value_by_line(line, EditionPos::Start);
                } else if let ValueSelected { line } | ValueEdit { line, .. } = &des.focus {
                    let line = *line;
                    des.close_input(false);
                    if des.listed_entries_count() == line + 1 {
                        // last listed entry
                        if des.drawer.content.entries[line].is_empty() {
                            // if the current entry is empty, we don't create a new one
                            // but go back to the current (empty) entry name
                            des.edit_entry_name_by_line(line, EditionPos::Start);
                        } else {
                            // we create a new entry and start edit it
                            // but we must ensure there's no search which could filter it
                            des.search.clear();
                            des.drawer.content.entries.push(Entry::default());
                            des.edit_entry_name_by_line(
                                des.drawer.content.entries.len() - 1,
                                EditionPos::Start,
                            );
                        }
                    } else {
                        des.edit_entry_name_by_line(
                            line + 1,
                            EditionPos::Start,
                        );
                    }
                }
                des.update_search();
                return Ok(CmdResult::Stay);
            }
        }

        // --- input

        if let Some(input) = self.drawer_state.input() {
            if input.apply_key_event(key) {
                if let DrawerEdit(des) = &mut self.drawer_state {
                    if des.focus.is_search() {
                        des.search.update(&des.drawer);
                    }
                }
                return Ok(CmdResult::Stay);
            }
        }

        // --- help

        if key == F1 || key == QUESTION || key == SHIFT_QUESTION {
            // notes:
            //  - F1 is rarely available in terminals
            //  - shift-? is here because on Windows on some keyboard I receive it for the ?
            self.help = Some(HelpState::default());
            return Ok(CmdResult::Stay);
        }

        if let DrawerEdit(des) = &mut self.drawer_state {
            if key == HOME {
                des.apply_scroll_command(ScrollCommand::Top);
                return Ok(CmdResult::Stay);
            }
            if key == END {
                des.apply_scroll_command(ScrollCommand::Bottom);
                return Ok(CmdResult::Stay);
            }
            if key == PAGE_UP {
                des.apply_scroll_command(ScrollCommand::Pages(-1));
                return Ok(CmdResult::Stay);
            }
            if key == PAGE_DOWN {
                des.apply_scroll_command(ScrollCommand::Pages(1));
                return Ok(CmdResult::Stay);
            }
        }

        if key == INSERT || as_letter(key) == Some('i') {
            if let DrawerEdit(des) = &mut self.drawer_state {
                if let NameSelected { line } = &des.focus {
                    let line = *line;
                    des.edit_entry_name_by_line(line, EditionPos::Start);
                }
                if let ValueSelected { line } = &des.focus {
                    let line = *line;
                    des.edit_entry_value_by_line(line, EditionPos::Start);
                }
            }
            return Ok(CmdResult::Stay);
        }

        if as_letter(key) == Some('a') {
            if let DrawerEdit(des) = &mut self.drawer_state {
                if let NameSelected { line } = &des.focus {
                    let line = *line;
                    des.edit_entry_name_by_line(line, EditionPos::End);
                }
                if let ValueSelected { line } = &des.focus {
                    let line = *line;
                    des.edit_entry_value_by_line(line, EditionPos::End);
                }
            }
            return Ok(CmdResult::Stay);
        }

        if let DrawerEdit(des) = &mut self.drawer_state {
            if key == RIGHT {
                match &des.focus {
                    SearchEdit { previous_line } => {
                        // we're here because apply_event on the input returned false,
                        // which means the right arrow key was ignored because it was
                        // at the end of the input. We'll assume the user wants to
                        // select the value of the selected line
                        if let Some(line) = des.best_search_line() {
                            des.focus = ValueSelected { line };
                        } else if let Some(&line) = previous_line.as_ref() {
                            des.focus = ValueSelected { line };
                        }
                    }
                    NameSelected { line } => {
                        let line = *line;
                        des.focus = ValueSelected { line };
                    }
                    NoneSelected => {
                        des.focus = NameSelected { line: 0 };
                    }
                    _ => {}
                }
                return Ok(CmdResult::Stay);
            }
            if key == LEFT {
                match &des.focus {
                    NameSelected { .. } => {
                        des.focus = SearchEdit { previous_line: des.focus.line() };
                    }
                    ValueSelected { line } => {
                        let line = *line;
                        des.focus = NameSelected { line };
                    }
                    NoneSelected => {
                        des.focus = NameSelected { line: 0 };
                    }
                    _ => {}
                }
                return Ok(CmdResult::Stay);
            }
            if key == UP {
                des.move_line(Direction::Up);
                return Ok(CmdResult::Stay);
            }
            if key == DOWN {
                des.move_line(Direction::Down);
                return Ok(CmdResult::Stay);
            }
        }

        // --- other simple char shortcuts

        if let Some(letter) = as_letter(key) {

            if let DrawerEdit(des) = &mut self.drawer_state {
                // if we're here, there's no input
                match (letter, des.focus.line()) {
                    ('n', _) => {
                        // new entry
                        des.search.clear();
                        let idx = des.drawer.content.empty_entry();
                        des.edit_entry_name_by_line(idx, EditionPos::Start);
                    }
                    ('d', Some(line)) => {
                        // delete entry (with confirmation)
                        des.focus = PendingRemoval { line };
                    }
                    ('/', _) => {
                        // start searching
                        des.focus = SearchEdit { previous_line: des.focus.line() };
                    }
                    _ => {}
                }
                return Ok(CmdResult::Stay);
            }
        }

        Ok(CmdResult::Stay)
    }
}
