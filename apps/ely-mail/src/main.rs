// Release builds open no console on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod add_view;
mod adding;
mod flow;
mod kinds;
mod mail;
mod menu;
mod removing;
mod side;
mod view;

#[cfg(test)]
mod tests;

use ely_platform::AppId;

fn main() {
    ely_ui::run(AppId::Mail, mail::open);
}
