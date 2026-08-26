use adw::prelude::AlertDialogExtManual as _;

pub trait I18nWidgetExt {
    fn set_tooltip_i18n(&self, key: &str);
    fn set_tooltip_i18n_opt(&self, key: Option<&str>);
}

pub trait I18nMenuExt {
    fn append_i18n(&self, key: &str, detailed_action: &str);
    fn append_submenu_i18n(&self, key: &str, submenu: &gtk::gio::Menu);
}

pub trait I18nFileFilterExt {
    fn set_name_i18n(&self, key: &str);
}

pub trait I18nAlertDialogExt {
    fn add_responses_i18n(&self, responses: &[(&str, &str)]);
}

pub fn menu_item_i18n(key: &str, detailed_action: &str) -> gtk::gio::MenuItem {
    gtk::gio::MenuItem::new(Some(crate::i18n::text(key).as_ref()), Some(detailed_action))
}

impl<T: gtk::glib::prelude::IsA<gtk::Widget>> I18nWidgetExt for T {
    fn set_tooltip_i18n(&self, key: &str) {
        self.set_tooltip_i18n_opt(Some(key));
    }

    fn set_tooltip_i18n_opt(&self, key: Option<&str>) {
        let text = key.map(crate::i18n::text);
        gtk::prelude::WidgetExt::set_tooltip_text(self, text.as_deref());
    }
}

impl I18nMenuExt for gtk::gio::Menu {
    fn append_i18n(&self, key: &str, detailed_action: &str) {
        self.append(Some(crate::i18n::text(key).as_ref()), Some(detailed_action));
    }

    fn append_submenu_i18n(&self, key: &str, submenu: &gtk::gio::Menu) {
        self.append_submenu(Some(crate::i18n::text(key).as_ref()), submenu);
    }
}

impl I18nFileFilterExt for gtk::FileFilter {
    fn set_name_i18n(&self, key: &str) {
        self.set_name(Some(crate::i18n::text(key).as_ref()));
    }
}

impl I18nAlertDialogExt for adw::AlertDialog {
    fn add_responses_i18n(&self, responses: &[(&str, &str)]) {
        let labels = responses
            .iter()
            .map(|(_, key)| crate::i18n::text(key))
            .collect::<Vec<_>>();
        let responses = responses
            .iter()
            .zip(&labels)
            .map(|((id, _), label)| (*id, label.as_ref()))
            .collect::<Vec<_>>();
        self.add_responses(&responses);
    }
}
