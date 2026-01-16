use crate::dock::{Dock, DockPosition, PanelHandle};
use gpui::{
    Action, AnyElement, AnyView, App, Context, Corner, Entity, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, Styled, Subscription, Window,
};
use settings::SettingsStore;
use std::sync::Arc;
use ui::{
    ContextMenu, Divider, DividerColor, IconButton, IconSize, Tooltip, prelude::*,
    right_click_menu, v_flex,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSide {
    Left,
    Right,
}

pub struct SidebarButtons {
    side: SidebarSide,
    left_dock: Entity<Dock>,
    bottom_dock: Entity<Dock>,
    right_dock: Entity<Dock>,
    bottom_items: Vec<AnyView>,
    _subscriptions: Vec<Subscription>,
}

impl SidebarButtons {
    pub fn new(
        side: SidebarSide,
        left_dock: Entity<Dock>,
        bottom_dock: Entity<Dock>,
        right_dock: Entity<Dock>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.observe(&left_dock, |_, _, cx| cx.notify()),
            cx.observe(&bottom_dock, |_, _, cx| cx.notify()),
            cx.observe(&right_dock, |_, _, cx| cx.notify()),
            cx.observe_global::<SettingsStore>(|_, cx| cx.notify()),
        ];
        Self {
            side,
            left_dock,
            bottom_dock,
            right_dock,
            bottom_items: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn add_bottom_item<V: Render>(&mut self, item: Entity<V>, cx: &mut Context<Self>) {
        self.bottom_items.push(item.into());
        cx.notify();
    }

    fn get_top_panel_names(&self) -> &[&'static str] {
        match self.side {
            SidebarSide::Left => &["Project Panel", "GitPanel", "Outline Panel", "CollabPanel"],
            SidebarSide::Right => &["AgentPanel", "AgentsPanel", "NotificationPanel"],
        }
    }

    fn get_bottom_panel_names(&self) -> &[&'static str] {
        match self.side {
            SidebarSide::Left => &["TerminalPanel", "DebugPanel"],
            SidebarSide::Right => &[],
        }
    }

    fn render_panel_button(
        &self,
        panel: &Arc<dyn PanelHandle>,
        is_active_button: bool,
        dock_position: DockPosition,
        toggle_action: Box<dyn Action>,
        focus_handle: FocusHandle,
        window: &mut Window,
        cx: &App,
    ) -> Option<impl IntoElement> {
        let icon = panel.icon(window, cx)?;
        let icon_tooltip = panel.icon_tooltip(window, cx)?;
        let name = panel.persistent_name();
        let panel_clone = panel.clone();

        let (action, tooltip): (Box<dyn Action>, SharedString) = if is_active_button {
            (
                toggle_action,
                format!("Close {} Dock", dock_position.label()).into(),
            )
        } else {
            (panel.toggle_action(window, cx), icon_tooltip.into())
        };

        let (menu_anchor, menu_attach) = match self.side {
            SidebarSide::Left => (Corner::TopLeft, Corner::TopRight),
            SidebarSide::Right => (Corner::TopRight, Corner::TopLeft),
        };

        Some(
            right_click_menu(name)
                .menu(move |window, cx| {
                    const POSITIONS: [DockPosition; 3] = [
                        DockPosition::Left,
                        DockPosition::Right,
                        DockPosition::Bottom,
                    ];

                    ContextMenu::build(window, cx, |mut menu, _, cx| {
                        for position in POSITIONS {
                            if position != dock_position
                                && panel_clone.position_is_valid(position, cx)
                            {
                                let panel = panel_clone.clone();
                                menu = menu.entry(
                                    format!("Dock {}", position.label()),
                                    None,
                                    move |window, cx| {
                                        panel.set_position(position, window, cx);
                                    },
                                )
                            }
                        }
                        menu
                    })
                })
                .anchor(menu_anchor)
                .attach(menu_attach)
                .trigger(move |is_active, _window, _cx| {
                    IconButton::new((name, is_active_button as u64), icon)
                        .icon_size(IconSize::Medium)
                        .toggle_state(is_active_button)
                        .on_click({
                            let action = action.boxed_clone();
                            let focus_handle = focus_handle.clone();
                            move |_, window, cx| {
                                window.focus(&focus_handle, cx);
                                window.dispatch_action(action.boxed_clone(), cx)
                            }
                        })
                        .when(!is_active, |this| {
                            let tooltip = tooltip.clone();
                            let action = action.boxed_clone();
                            this.tooltip(move |_window, cx| {
                                Tooltip::for_action(tooltip.clone(), &*action, cx)
                            })
                        })
                }),
        )
    }

    fn collect_buttons_from_dock(
        &self,
        dock: &Entity<Dock>,
        top_names: &[&'static str],
        bottom_names: &[&'static str],
        top_buttons: &mut Vec<AnyElement>,
        bottom_buttons: &mut Vec<AnyElement>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dock_read = dock.read(cx);
        let dock_position = dock_read.position();
        let is_open = dock_read.is_open();
        let active_panel_index = dock_read.active_panel_index();
        let toggle_action = dock_read.toggle_action();
        let focus_handle = dock_read.focus_handle(cx);

        for (i, panel) in dock_read.panels().enumerate() {
            let name = panel.persistent_name();
            let is_active_button = Some(i) == active_panel_index && is_open;

            let should_show_in_top = top_names.contains(&name);
            let should_show_in_bottom = bottom_names.contains(&name);

            if !should_show_in_top && !should_show_in_bottom {
                continue;
            }

            if let Some(button) = self.render_panel_button(
                panel,
                is_active_button,
                dock_position,
                toggle_action.boxed_clone(),
                focus_handle.clone(),
                window,
                cx,
            ) {
                if should_show_in_top {
                    top_buttons.push(button.into_any_element());
                } else if should_show_in_bottom {
                    bottom_buttons.push(button.into_any_element());
                }
            }
        }
    }
}

impl Render for SidebarButtons {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let top_names = self.get_top_panel_names();
        let bottom_names = self.get_bottom_panel_names();

        let mut top_buttons: Vec<AnyElement> = Vec::new();
        let mut bottom_buttons: Vec<AnyElement> = Vec::new();

        let left_dock = self.left_dock.clone();
        let bottom_dock = self.bottom_dock.clone();
        let right_dock = self.right_dock.clone();

        self.collect_buttons_from_dock(
            &left_dock,
            top_names,
            bottom_names,
            &mut top_buttons,
            &mut bottom_buttons,
            window,
            cx,
        );
        self.collect_buttons_from_dock(
            &bottom_dock,
            top_names,
            bottom_names,
            &mut top_buttons,
            &mut bottom_buttons,
            window,
            cx,
        );
        self.collect_buttons_from_dock(
            &right_dock,
            top_names,
            bottom_names,
            &mut top_buttons,
            &mut bottom_buttons,
            window,
            cx,
        );

        for item in &self.bottom_items {
            bottom_buttons.push(item.clone().into_any_element());
        }

        let has_top_buttons = !top_buttons.is_empty();
        let has_bottom_buttons = !bottom_buttons.is_empty();

        v_flex()
            .h_full()
            .justify_between()
            .gap_2()
            .py_2()
            .px_0p5()
            .child(v_flex().gap_2().children(top_buttons))
            .when(has_top_buttons && has_bottom_buttons, |this| {
                this.child(Divider::horizontal().color(DividerColor::Border))
            })
            .child(v_flex().gap_2().children(bottom_buttons))
    }
}
