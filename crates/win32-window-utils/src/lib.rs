use std::ffi::c_void;

use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::{
    EnableMenuItem, GWL_STYLE, GetSystemMenu, GetWindowLongPtrW, IsIconic, IsZoomed,
    MENU_ITEM_FLAGS, MF_BYCOMMAND, MF_ENABLED, MF_GRAYED, PostMessageW, SC_CLOSE, SC_MAXIMIZE,
    SC_MINIMIZE, SC_MOVE, SC_RESTORE, SC_SIZE, SW_RESTORE, SetForegroundWindow, ShowWindowAsync,
    TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TPM_TOPALIGN, TrackPopupMenuEx, WINDOW_STYLE,
    WM_NULL, WM_SYSCOMMAND, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
};

/// Restore a Win32 window from the maximized state.
pub fn restore_window(hwnd: isize) -> bool {
    let hwnd = HWND(hwnd as *mut c_void);
    unsafe { ShowWindowAsync(hwnd, SW_RESTORE).as_bool() }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct SystemMenuState {
    restore: bool,
    move_window: bool,
    size: bool,
    minimize: bool,
    maximize: bool,
    close: bool,
}

fn system_menu_state(
    style: WINDOW_STYLE,
    is_minimized: bool,
    is_maximized: bool,
) -> SystemMenuState {
    let has_system_menu = style.contains(WS_SYSMENU);
    let is_restored = !is_minimized && !is_maximized;
    let can_resize = style.contains(WS_THICKFRAME);
    let has_minimize = style.contains(WS_MINIMIZEBOX);
    let has_maximize = style.contains(WS_MAXIMIZEBOX);

    SystemMenuState {
        restore: has_system_menu && !is_restored,
        move_window: has_system_menu && is_restored,
        size: has_system_menu && can_resize && is_restored,
        minimize: has_system_menu && has_minimize && !is_minimized,
        maximize: has_system_menu && has_maximize && !is_maximized,
        close: has_system_menu,
    }
}

fn enable_menu_item(
    menu: windows::Win32::UI::WindowsAndMessaging::HMENU,
    command: u32,
    enabled: bool,
) {
    let flags: MENU_ITEM_FLAGS = MF_BYCOMMAND | if enabled { MF_ENABLED } else { MF_GRAYED };
    unsafe {
        let _ = EnableMenuItem(menu, command, flags);
    }
}

fn sync_system_menu_state(hwnd: HWND, menu: windows::Win32::UI::WindowsAndMessaging::HMENU) {
    let style = WINDOW_STYLE(unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32);
    let state = system_menu_state(style, unsafe { IsIconic(hwnd).as_bool() }, unsafe {
        IsZoomed(hwnd).as_bool()
    });

    enable_menu_item(menu, SC_RESTORE, state.restore);
    enable_menu_item(menu, SC_MOVE, state.move_window);
    enable_menu_item(menu, SC_SIZE, state.size);
    enable_menu_item(menu, SC_MINIMIZE, state.minimize);
    enable_menu_item(menu, SC_MAXIMIZE, state.maximize);
    enable_menu_item(menu, SC_CLOSE, state.close);
}

/// Show the native Win32 system menu for a window at the given client-area position.
pub fn show_window_system_menu(hwnd: isize, x: i32, y: i32) {
    let hwnd = HWND(hwnd as *mut c_void);
    let mut position = POINT { x, y };

    unsafe {
        if !ClientToScreen(hwnd, &mut position).as_bool() {
            return;
        }

        let menu = GetSystemMenu(hwnd, false);
        if menu.0.is_null() {
            return;
        }

        sync_system_menu_state(hwnd, menu);
        let _ = SetForegroundWindow(hwnd);
        let command = TrackPopupMenuEx(
            menu,
            (TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD).0,
            position.x,
            position.y,
            hwnd,
            None,
        )
        .0 as usize;

        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM::default(), LPARAM::default());
        if command != 0 {
            let _ = PostMessageW(
                Some(hwnd),
                WM_SYSCOMMAND,
                WPARAM(command),
                LPARAM::default(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restored_window_menu_state_enables_move_size_minimize_and_maximize() {
        let style = WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
        let state = system_menu_state(style, false, false);

        assert_eq!(
            state,
            SystemMenuState {
                restore: false,
                move_window: true,
                size: true,
                minimize: true,
                maximize: true,
                close: true,
            }
        );
    }

    #[test]
    fn maximized_window_menu_state_enables_restore_and_minimize_only() {
        let style = WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
        let state = system_menu_state(style, false, true);

        assert_eq!(
            state,
            SystemMenuState {
                restore: true,
                move_window: false,
                size: false,
                minimize: true,
                maximize: false,
                close: true,
            }
        );
    }

    #[test]
    fn minimized_window_menu_state_enables_restore_and_maximize_only() {
        let style = WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
        let state = system_menu_state(style, true, false);

        assert_eq!(
            state,
            SystemMenuState {
                restore: true,
                move_window: false,
                size: false,
                minimize: false,
                maximize: true,
                close: true,
            }
        );
    }
}
