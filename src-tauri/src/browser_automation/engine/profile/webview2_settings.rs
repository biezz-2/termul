//! WebView2 settings and cleanup for agent tabs, on the UI thread inside
//! `with_webview`.

use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2CookieList, ICoreWebView2CookieManager, ICoreWebView2Settings4,
    ICoreWebView2_2,
};
use webview2_com::{take_pwstr, GetCookiesCompletedHandler};
use windows_core::{Interface, PCWSTR, PWSTR};

use super::is_loopback_domain;

/// Turn off the password manager and form autofill for this tab. The
/// runtime's default for password autosave is already off; setting it keeps
/// that true if the default ever changes. Returns false when the runtime
/// predates the settings.
pub fn disable_credential_prompts(core: &ICoreWebView2) -> bool {
    // SAFETY: COM calls on the UI thread that owns `core`.
    unsafe {
        let Ok(settings) = core.Settings() else {
            return false;
        };
        let Ok(settings) = settings.cast::<ICoreWebView2Settings4>() else {
            return false;
        };
        let autosave = settings.SetIsPasswordAutosaveEnabled(false).is_ok();
        let autofill = settings.SetIsGeneralAutofillEnabled(false).is_ok();
        autosave && autofill
    }
}

/// Delete the persisted cookies loopback hosts left in this tab's profile.
/// Session cookies are not persisted by the runtime, so skipping them
/// spares the cookies the tab's own first page is setting right now. The
/// deletion finishes asynchronously on the UI thread; false means it was
/// not started.
pub fn clear_loopback_cookies(core: &ICoreWebView2) -> bool {
    let Ok(core) = core.cast::<ICoreWebView2_2>() else {
        return false;
    };
    // SAFETY: COM calls on the UI thread that owns `core`; the handler runs
    // there too.
    unsafe {
        let Ok(manager) = core.CookieManager() else {
            return false;
        };
        let handler = GetCookiesCompletedHandler::create(Box::new({
            let manager = manager.clone();
            move |result, cookies| {
                result?;
                if let Some(cookies) = cookies {
                    delete_loopback(&manager, &cookies)?;
                }
                Ok(())
            }
        }));
        manager.GetCookies(PCWSTR::null(), &handler).is_ok()
    }
}

/// # Safety
/// UI thread of the webview that owns `manager`.
unsafe fn delete_loopback(
    manager: &ICoreWebView2CookieManager,
    cookies: &ICoreWebView2CookieList,
) -> windows_core::Result<()> {
    let mut count = 0;
    cookies.Count(&mut count)?;
    for index in 0..count {
        let cookie = cookies.GetValueAtIndex(index)?;
        let mut session = windows_core::BOOL::default();
        cookie.IsSession(&mut session)?;
        if session.as_bool() {
            continue;
        }
        let mut domain = PWSTR::null();
        cookie.Domain(&mut domain)?;
        if is_loopback_domain(&take_pwstr(domain)) {
            manager.DeleteCookie(&cookie)?;
        }
    }
    Ok(())
}
