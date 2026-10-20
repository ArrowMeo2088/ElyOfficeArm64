// Release builds open no console on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod add_view;
mod adding;
mod boxes;
mod compose;
mod controls;
mod draft_list;
mod drafts;
mod flow;
mod identity;
mod kinds;
mod letter;
mod mail;
mod menu;
mod panes;
mod pictures;
mod reader;
mod reading;
mod removing;
mod side;
mod syncing;
mod view;
mod writing;

#[cfg(test)]
mod tests;

use ely_platform::AppId;

fn main() {
    ely_ui::run(AppId::Mail, mail::open);
}
