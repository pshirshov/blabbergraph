use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{self, Align, Label, Orientation, ScrolledWindow};

use crate::model::state::AppState;
use crate::pw::graph::PortInfo;
use crate::pw::message::{PortDirection, PwCommand};

pub struct MatrixView {
    pub container: gtk::Box,
    grid: gtk::Grid,
    cells: HashMap<(u32, u32), gtk::ToggleButton>,
}

impl MatrixView {
    pub fn new() -> Self {
        let container = gtk::Box::new(Orientation::Vertical, 4);
        container.set_vexpand(true);
        container.set_hexpand(true);
        container.set_margin_start(8);
        container.set_margin_end(8);
        container.set_margin_top(8);
        container.set_margin_bottom(8);

        let info_label = Label::new(Some(
            "Rows: output ports | Columns: input ports | Toggle cells to create/destroy links",
        ));
        info_label.add_css_class("caption");
        info_label.set_halign(Align::Start);
        container.append(&info_label);

        let scroll = ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        scroll.set_vexpand(true);
        scroll.set_hexpand(true);

        let grid = gtk::Grid::new();
        grid.set_column_spacing(1);
        grid.set_row_spacing(1);
        grid.set_column_homogeneous(false);
        grid.set_row_homogeneous(false);

        scroll.set_child(Some(&grid));
        container.append(&scroll);

        Self {
            container,
            grid,
            cells: HashMap::new(),
        }
    }

    pub fn rebuild(
        &mut self,
        pw_sender: &pipewire::channel::Sender<PwCommand>,
        state: &Rc<RefCell<AppState>>,
    ) {
        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }
        self.cells.clear();

        let state_borrow = state.borrow();

        let mut output_ports: Vec<&PortInfo> = Vec::new();
        let mut input_ports: Vec<&PortInfo> = Vec::new();

        for port in state_borrow.graph.ports.values() {
            let is_audio = state_borrow.graph.is_audio_node(port.node_id);
            if !is_audio {
                continue;
            }
            match port.direction {
                PortDirection::Output => output_ports.push(port),
                PortDirection::Input => input_ports.push(port),
            }
        }

        output_ports.sort_by(|a, b| a.node_id.cmp(&b.node_id).then(a.name.cmp(&b.name)));
        input_ports.sort_by(|a, b| a.node_id.cmp(&b.node_id).then(a.name.cmp(&b.name)));

        if output_ports.is_empty() || input_ports.is_empty() {
            let empty_label = Label::new(Some("No audio ports available"));
            self.grid.attach(&empty_label, 0, 0, 1, 1);
            return;
        }

        // Column headers (input ports) - use vertical text via CSS
        for (col, in_port) in input_ports.iter().enumerate() {
            let node_name = state_borrow
                .graph
                .nodes
                .get(&in_port.node_id)
                .map(|n| {
                    if n.description.is_empty() {
                        n.name.as_str()
                    } else {
                        n.description.as_str()
                    }
                })
                .unwrap_or("?");
            let text = format!("{}\n{}", node_name, in_port.name);
            let label = Label::new(Some(&text));
            label.add_css_class("caption");
            label.set_max_width_chars(15);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_halign(Align::Center);
            self.grid.attach(&label, (col + 1) as i32, 0, 1, 1);
        }

        // Row headers and cells
        for (row, out_port) in output_ports.iter().enumerate() {
            let node_name = state_borrow
                .graph
                .nodes
                .get(&out_port.node_id)
                .map(|n| {
                    if n.description.is_empty() {
                        n.name.as_str()
                    } else {
                        n.description.as_str()
                    }
                })
                .unwrap_or("?");
            let text = format!("{} : {}", node_name, out_port.name);
            let label = Label::new(Some(&text));
            label.add_css_class("caption");
            label.set_halign(Align::End);
            label.set_max_width_chars(25);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            self.grid.attach(&label, 0, (row + 1) as i32, 1, 1);

            for (col, in_port) in input_ports.iter().enumerate() {
                let btn = gtk::ToggleButton::new();
                btn.set_size_request(24, 24);

                let link_exists = state_borrow
                    .graph
                    .find_link(out_port.id, in_port.id)
                    .is_some();
                btn.set_active(link_exists);

                if link_exists {
                    btn.add_css_class("suggested-action");
                }

                let sender = pw_sender.clone();
                let state_ref = state.clone();
                let out_id = out_port.id;
                let in_id = in_port.id;
                btn.connect_toggled(move |btn| {
                    let active = btn.is_active();
                    if active {
                        btn.add_css_class("suggested-action");
                        let _ = sender.send(PwCommand::CreateLink {
                            output_port_id: out_id,
                            input_port_id: in_id,
                        });
                    } else {
                        btn.remove_css_class("suggested-action");
                        let state = state_ref.borrow();
                        if let Some(link_id) = state.graph.find_link(out_id, in_id) {
                            let _ = sender.send(PwCommand::DestroyLink { link_id });
                        }
                    }
                });

                self.grid
                    .attach(&btn, (col + 1) as i32, (row + 1) as i32, 1, 1);
                self.cells.insert((out_port.id, in_port.id), btn);
            }
        }
    }

    /// Returns true if the cell existed and was updated.
    pub fn update_link_added(&self, output_port_id: u32, input_port_id: u32) -> bool {
        if let Some(btn) = self.cells.get(&(output_port_id, input_port_id)) {
            if !btn.is_active() {
                btn.set_active(true);
                btn.add_css_class("suggested-action");
            }
            true
        } else {
            false
        }
    }

    /// Returns true if the cell existed and was updated.
    pub fn update_link_removed(
        &self,
        _state: &AppState,
        output_port_id: u32,
        input_port_id: u32,
    ) -> bool {
        if let Some(btn) = self.cells.get(&(output_port_id, input_port_id)) {
            if btn.is_active() {
                btn.set_active(false);
                btn.remove_css_class("suggested-action");
            }
            true
        } else {
            false
        }
    }
}
