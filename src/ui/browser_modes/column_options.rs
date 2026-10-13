// SPDX-License-Identifier: MIT

use std::{cell::RefCell, rc::Rc};

use gtk::prelude::*;

use crate::ui::{
    controls::{check_menu_option, menu_action},
    preferences::{ListColumnId, PreferenceManager},
};

use super::{BoundModeItem, ListColumnLayout};

pub(super) fn create_column_options_popover(
    list_headings: &gtk::Box,
    columns: &ListColumnLayout,
    bound_items: &Rc<RefCell<Vec<BoundModeItem>>>,
) -> gtk::Popover {
    let popover = gtk::Popover::builder()
        .has_arrow(false)
        .halign(gtk::Align::End)
        .position(gtk::PositionType::Bottom)
        .build();
    popover.add_css_class("column-popover");
    crate::ui::scrolling::popover::dismiss_on_outside_scroll(&popover);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 2);
    content.add_css_class("column-menu");

    let title = gtk::Label::new(Some(&crate::i18n::tr("COLUMNS")));
    title.add_css_class("menu-heading");
    title.set_xalign(0.0);
    title.set_hexpand(true);
    content.append(&title);

    let manager = PreferenceManager::shared();
    let hidden = manager.list_hidden_columns();
    let checks: Rc<RefCell<Vec<(ListColumnId, gtk::Image)>>> = Rc::new(RefCell::new(Vec::new()));

    for &col in &ListColumnId::ALL {
        let is_visible = !hidden.contains(&col);
        let (option, check) = check_menu_option(&crate::i18n::tr(col.title()), is_visible);
        if col == ListColumnId::Name {
            option.set_sensitive(false);
            option.set_tooltip_text(Some(&crate::i18n::tr("The Name column cannot be hidden")));
        } else {
            let weak_headings = list_headings.downgrade();
            let columns = columns.clone();
            let bound_items = bound_items.clone();
            let check_for_click = check.clone();
            option.connect_clicked(move |_| {
                let manager = PreferenceManager::shared();
                let next = !manager.is_list_column_visible(col);
                manager.set_list_column_visible(col, next);
                check_for_click.set_visible(next);
                if let Some(headings) = weak_headings.upgrade() {
                    super::apply_columns_layout(&headings, &columns, &bound_items);
                }
            });
        }
        checks.borrow_mut().push((col, check));
        content.append(&option);
    }

    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

    let reset = menu_action(&crate::i18n::tr("Reset to default columns"));
    {
        let weak_headings = list_headings.downgrade();
        let weak_columns = columns.clone();
        let weak_bound_items = bound_items.clone();
        let checks_for_reset = checks.clone();
        reset.connect_clicked(move |_| {
            let manager = PreferenceManager::shared();
            manager.reset_list_columns();
            for (col, check) in checks_for_reset.borrow().iter() {
                check.set_visible(
                    *col == ListColumnId::Name || manager.is_list_column_visible(*col),
                );
            }
            if let Some(headings) = weak_headings.upgrade() {
                super::apply_columns_layout(&headings, &weak_columns, &weak_bound_items);
            }
        });
    }
    content.append(&reset);

    {
        let checks_for_map = checks.clone();
        popover.connect_map(move |_| {
            let manager = PreferenceManager::shared();
            for (col, check) in checks_for_map.borrow().iter() {
                check.set_visible(
                    *col == ListColumnId::Name || manager.is_list_column_visible(*col),
                );
            }
        });
    }

    popover.set_child(Some(&content));
    popover
}

pub(super) fn open_column_options_popover(
    parent: &impl IsA<gtk::Widget>,
    list_headings: &gtk::Box,
    columns: &ListColumnLayout,
    bound_items: &Rc<RefCell<Vec<BoundModeItem>>>,
    point: Option<(f64, f64)>,
) -> gtk::Popover {
    let popover = create_column_options_popover(list_headings, columns, bound_items);
    popover.set_parent(parent);
    if let Some((x, y)) = point {
        popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    }
    popover.connect_closed(|p| p.unparent());
    popover.popup();
    popover
}
