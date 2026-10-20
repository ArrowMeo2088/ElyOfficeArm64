// Release builds open no console on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod add_view;
mod adding;
mod boxes;
mod controls;
mod flow;
mod kinds;
mod letter;
mod mail;
mod menu;
mod panes;
mod reader;
mod reading;
mod removing;
mod side;
mod syncing;
mod view;

#[cfg(test)]
mod tests;

use ely_platform::AppId;

fn main() {
    ely_ui::run(AppId::Mail, mail::open);
}
