// Windows release builds hide the console window; every other target keeps it.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    phonix_desktop_lib::run();
}
