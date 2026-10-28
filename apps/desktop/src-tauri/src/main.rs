// Unter Windows startet die Release-App ohne Konsolenfenster.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    nexus_desktop_lib::run()
}
