// Unter Windows startet die Release-App ohne Konsolenfenster.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `dispatch-worker <lauf>` kommt vom Server im Prozess der App; dieser Start arbeitet nur den
    // Lauf ab und baut keine Oberfläche auf (kein Dock-Symbol, kein Fenster).
    if nexus_desktop_lib::run_dispatch_worker() {
        return;
    }
    nexus_desktop_lib::run()
}
