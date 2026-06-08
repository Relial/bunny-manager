use egui::{Event, Id, KeyboardShortcut, ModifierNames, Ui, Widget};

pub struct ShortcutButton<'a> {
    bind: &'a mut KeyboardShortcut,
    id: Id,
    modifier_names: &'a ModifierNames<'a>,
}

impl<'a> ShortcutButton<'a> {
    pub fn new(shortcut: &'a mut KeyboardShortcut, id: impl Into<Id>) -> Self {
        Self {
            bind: shortcut,
            id: id.into(),
            modifier_names: &ModifierNames::NAMES,
        }
    }
}

fn get_expecting(ui: &Ui, id: Id) -> bool {
    ui.ctx()
        .memory_mut(|mem| *mem.data.get_temp_mut_or_default(ui.make_persistent_id(id)))
}

fn set_expecting(ui: &Ui, id: Id, expecting: bool) {
    ui.ctx().memory_mut(|mem| {
        *mem.data.get_temp_mut_or_default(ui.make_persistent_id(id)) = expecting;
    });
}

impl Widget for ShortcutButton<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let text = self.bind.format(self.modifier_names, false);
        let mut expecting = get_expecting(ui, self.id);
        let mut button = egui::Button::new(text);
        if expecting {
            button = button.selected(true);
        }

        let mut response = ui.add(button);

        let prev_expecting = expecting;
        if response.clicked() {
            expecting = !expecting;
        }

        if expecting {
            if response.clicked_elsewhere() {
                expecting = false;
            } else {
                if let Some((key, mods)) = ui.input(|i| {
                    i.events.iter().find_map(|e| match e {
                        Event::Key {
                            key,
                            pressed: true,
                            modifiers,
                            repeat: false,
                            ..
                        } => Some((*key, *modifiers)),
                        _ => None,
                    })
                }) {
                    let shortcut = KeyboardShortcut::new(mods, key);
                    ui.input_mut(|i| i.consume_shortcut(&shortcut));
                    *self.bind = shortcut;
                    response.mark_changed();
                    expecting = false;
                }
            }
        }

        if prev_expecting != expecting {
            set_expecting(ui, self.id, expecting);
        }
        response
    }
}
